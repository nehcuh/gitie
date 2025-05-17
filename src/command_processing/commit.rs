use crate::{
    ai_module::utils::{ChatMessage, OpenAIChatCompletionResponse, OpenAIChatRequest, clean_ai_output},
    config_management::settings::AppConfig,
    cli_interface::args::CommitArgs,
    core::errors::{AIError, AppError, GitError},
    git_module::map_output_to_git_command_error,
    tree_sitter_analyzer::TreeSitterAnalyzer,
};
use std::{
    process::Command,
    path::PathBuf,
};

/// 为提交消息创建临时文件
/// 
/// 创建一个包含提交消息的临时文件
/// 
/// # 参数
/// 
/// * `message` - 提交消息内容
/// 
/// # 返回值
/// 
/// * `Result<PathBuf, AppError>` - 临时文件路径或错误
fn create_commit_message_file(message: &str) -> Result<PathBuf, AppError> {
    let temp_dir = std::env::temp_dir();
    let temp_file_path = temp_dir.join(format!("gitie_commit_msg_{}.txt", std::process::id()));
    std::fs::write(&temp_file_path, message)
        .map_err(|e| AppError::IO("无法创建临时提交消息文件".into(), e))?;
    Ok(temp_file_path)
}

/// 清理提交消息临时文件
/// 
/// # 参数
/// 
/// * `file_path` - 临时文件路径
fn cleanup_commit_message_file(file_path: &PathBuf) {
    if let Err(e) = std::fs::remove_file(file_path) {
        tracing::warn!("无法删除临时提交消息文件: {}", e);
    }
}

/// 判断是否是Tree-sitter相关标志
fn is_tree_sitter_flag(arg: &str) -> bool {
    arg == "--tree-sitter" || 
    arg == "-t" || 
    arg.starts_with("--tree-sitter=")
}

/// 判断标志是否包含tree-sitter选项
fn contains_tree_sitter_option(arg: &str) -> bool {
    // 检查是否是包含't'的短标志组合（如 -at）
    arg.starts_with('-') && !arg.starts_with("--") && arg.contains('t')
}

/// 判断标志是否包含自动暂存选项
fn contains_auto_stage_option(arg: &str) -> bool {
    // 检查是否是包含'a'的短标志组合（如 -at）
    arg.starts_with('-') && !arg.starts_with("--") && arg.contains('a')
}

/// 判断参数是否是Tree-sitter标志的值
fn is_tree_sitter_value(args: &[String], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    
    // 检查前一个参数是否是tree-sitter标志
    let prev_arg = &args[index - 1];
    if (prev_arg == "--tree-sitter" || prev_arg == "-t") && !args[index].starts_with('-') {
        return true;
    }
    
    false
}

/// 从短标志组合中创建新的参数，移除特定选项
fn create_filtered_short_option(arg: &str, remove_options: &[char]) -> Option<String> {
    if !arg.starts_with('-') || arg.starts_with("--") {
        return None;
    }
    
    // 过滤掉不需要的选项
    let filtered: String = arg.chars()
        .filter(|c| *c == '-' || !remove_options.contains(c))
        .collect();
    
    // 如果只剩下'-'，返回None
    if filtered == "-" {
        None
    } else {
        Some(filtered)
    }
}

/// Handles a standard git commit by passing through to git
///
/// # Arguments
///
/// * `args` - Commit arguments from CLI
/// * `context_msg` - Context message for logging
///
/// # Returns
///
/// * `Result<(), AppError>` - Success or an error
pub async fn handle_commit_passthrough(
    args: CommitArgs,
    context_msg: String,
) -> Result<(), AppError> {
    tracing::info!(
        "提交传递 {}: 消息: {:?}, 参数: {:?}",
        context_msg,
        args.message,
        args.passthrough_args
    );

    // 创建 git commit 命令参数
    let mut git_args = vec!["commit".to_string()];

    // Add -a/--all flag if auto_stage is set
    if args.auto_stage {
        git_args.push("-a".to_string());
    }

    let temp_file_path_opt = if let Some(message) = &args.message {
        // 创建提交消息临时文件
        let temp_file_path = create_commit_message_file(message)?;
        
        git_args.push("-F".to_string());
        git_args.push(temp_file_path.to_string_lossy().to_string());
        Some(temp_file_path)
    } else {
        None
    };

    // Add remaining args, but exclude -a, -all if auto_stage is true, and tree-sitter flags with their values
    for (i, arg) in args.passthrough_args.iter().enumerate() {
        // 判断是否是特定标志
        let is_auto_stage_flag = arg == "-a" || arg == "--all";
        let is_ts_flag = is_tree_sitter_flag(arg);
        let is_ts_value = is_tree_sitter_value(&args.passthrough_args, i);
        
        // 处理组合的短标志
        let contains_auto_stage = contains_auto_stage_option(arg);
        let contains_ts = contains_tree_sitter_option(arg);
        
        if (args.auto_stage && (is_auto_stage_flag || contains_auto_stage)) || 
           is_ts_flag || 
           is_ts_value {
            // 跳过这些标志
            continue;
        }
        
        // 处理包含多个选项的组合短标志
        if contains_auto_stage && contains_ts {
            // 如果同时包含auto-stage和tree-sitter选项，则需要特殊处理
            // 移除 'a' 和 't'
            if let Some(filtered_arg) = create_filtered_short_option(arg, &['a', 't']) {
                if filtered_arg != "-" {
                    git_args.push(filtered_arg.to_string());
                }
            }
        } else if args.auto_stage && contains_auto_stage {
            // 如果启用了auto-stage，移除 'a'
            if let Some(filtered_arg) = create_filtered_short_option(arg, &['a']) {
                if filtered_arg != "-" {
                    git_args.push(filtered_arg.to_string());
                }
            }
        } else if contains_ts {
            // 如果包含tree-sitter选项，移除 't'
            if let Some(filtered_arg) = create_filtered_short_option(arg, &['t']) {
                if filtered_arg != "-" {
                    git_args.push(filtered_arg.to_string());
                }
            }
        } else {
            // 没有特殊处理的情况，直接添加
            git_args.push(arg.clone());
        }
    }

    let cmd_desc = format!(
        "commit (passthrough {}) args: {:?}",
        context_msg, args.passthrough_args
    );
    
    tracing::debug!("执行 Git 命令: git {}", git_args.join(" "));
    
    let result = crate::git_module::passthrough_to_git_with_error_handling(&git_args, true);
    
    // 清理临时文件
    if let Some(temp_file_path) = temp_file_path_opt {
        cleanup_commit_message_file(&temp_file_path);
    }
    
    if let Err(e) = result {
        tracing::error!("传递 git {} 失败", cmd_desc);
        return Err(e);
    }
    
    tracing::info!(
        "传递 git {} 已成功启动/完成。",
        cmd_desc
    );
    Ok(())
}

/// 判断是否应该使用Tree-sitter分析
fn should_use_tree_sitter(args: &CommitArgs, config: &AppConfig) -> bool {
    // 优先使用命令行参数
    if let Some(level) = &args.tree_sitter {
        if !level.is_empty() {
            tracing::info!("通过命令行参数启用Tree-sitter分析，级别: {}", level);
            return true;
        } else {
            tracing::info!("命令行参数 --tree-sitter 值为空，不启用分析");
            return false;
        }
    }

    // 否则使用配置文件中的设置
    if config.tree_sitter.enabled {
        tracing::info!("通过配置文件启用Tree-sitter分析");
        return true;
    }

    tracing::debug!("未启用Tree-sitter分析");
    false
}

/// 获取Tree-sitter分析级别
fn get_analysis_depth(args: &CommitArgs, config: &AppConfig) -> String {
    // 优先使用命令行参数
    if let Some(level) = &args.tree_sitter {
        if !level.is_empty() {
            match level.as_str() {
                "shallow" | "medium" | "deep" => {
                    return level.clone();
                }
                _ => {
                    tracing::warn!("无效的分析级别: {}，使用默认值 'medium'", level);
                }
            }
        }
    }

    // 否则使用配置文件中的设置
    config.tree_sitter.analysis_depth.clone()
}

/// 使用Tree-sitter生成增强提示
async fn generate_enhanced_prompt_with_tree_sitter(
    diff_text: &str, 
    config: &AppConfig,
    args: &CommitArgs
) -> Result<String, AppError> {
    // 获取分析级别
    let analysis_depth = get_analysis_depth(args, config);
    tracing::info!("使用Tree-sitter进行语法分析，级别: {}", analysis_depth);

    // 克隆配置并设置分析级别
    let mut ts_config = config.tree_sitter.clone();
    ts_config.analysis_depth = analysis_depth;

    // 初始化Tree-sitter分析器
    let mut analyzer = match TreeSitterAnalyzer::new(ts_config) {
        Ok(analyzer) => analyzer,
        Err(e) => {
            return Err(AppError::IO(
                format!("Tree-sitter初始化失败: {}", e), 
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            ));
        }
    };

    // 尝试获取项目根目录
    if let Ok(output) = Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output() 
    {
        if output.status.success() {
            let root_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            analyzer.set_project_root(root_path.into());
        }
    }

    // 执行分析
    match analyzer.analyze_diff(diff_text) {
        Ok(_) => {
            // 生成增强提示
            // Call generate_commit_prompt with both required parameters
            let commit_prompt = analyzer.generate_commit_prompt(diff_text, config).await?;
            let enhanced_prompt = format!(
                "Git diff:\n{}\n\n{}\nGenerate commit message.",
                diff_text.trim(),
                commit_prompt
            );
        
            tracing::debug!("生成了增强的提交提示: \n{}", enhanced_prompt);
        
            Ok(enhanced_prompt)
        },
        Err(e) => {
            Err(AppError::IO(
                format!("Tree-sitter分析失败: {}", e),
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            ))
        }
    }
}

/// Handles the enhanced commit functionality with AI message generation
///
/// # Arguments
///
/// * `args` - Commit arguments from CLI
/// * `config` - Application configuration
///
/// # Returns
///
/// * `Result<(), AppError>` - Success or an error
pub async fn handle_commit(args: CommitArgs, config: &AppConfig) -> Result<(), AppError> {
    // Use AI by default unless --noai is specified
    // Note: The --ai flag is kept for backward compatibility
    if !args.noai {
        tracing::info!("AI 提交: 正在尝试生成消息（默认行为）...");
        // Handle auto-staging functionality
        if args.auto_stage {
            tracing::info!("由于使用了 -a/--all 标志，正在自动暂存已跟踪的更改");
            let add_result = Command::new("git")
                .arg("add")
                .arg("-u")
                .output()
                .map_err(|e| AppError::IO("Failed to auto-stage changes".to_string(), e))?;

            if !add_result.status.success() {
                tracing::error!("使用 git add -u 自动暂存更改失败");
                return Err(map_output_to_git_command_error("git add -u", add_result).into());
            }
        }

        let diff_out = Command::new("git")
            .arg("diff")
            .arg("--staged")
            .output()
            .map_err(|e| AppError::Git(GitError::DiffError(e)))?;
        if !diff_out.status.success() {
            tracing::error!("获取 git diff 时出错。是否有任何更改已暂存以供提交？");
            return Err(map_output_to_git_command_error("git diff --staged", diff_out).into());
        }
        let diff = String::from_utf8_lossy(&diff_out.stdout);
        if diff.trim().is_empty() {
            tracing::info!("AI 提交: 没有暂存的更改。检查是否使用了 --allow-empty。");
            if args.passthrough_args.contains(&"--allow-empty".to_string()) {
                let passthrough_commit_args = CommitArgs {
                    ai: false,
                    noai: true,
                    auto_stage: args.auto_stage,
                    tree_sitter: None,
                    message: None,
                    passthrough_args: args.passthrough_args.clone(),
                };
                return handle_commit_passthrough(
                    passthrough_commit_args,
                    "(AI commit with --allow-empty and no diff)".to_string(),
                )
                .await;
            } else {
                return Err(AppError::Git(GitError::NoStagedChanges));
            }
        }
        tracing::debug!("Staged changes for AI: \n{}", diff);

        // 检查是否应该使用Tree-sitter分析
        let use_tree_sitter = should_use_tree_sitter(&args, config);
        
        // 准备提示内容
        let user_prompt = if use_tree_sitter {
            // 使用Tree-sitter增强分析
            match generate_enhanced_prompt_with_tree_sitter(&diff, config, &args).await {
                Ok(enhanced_prompt) => enhanced_prompt,
                Err(e) => {
                    // 如果Tree-sitter分析失败，记录警告并回退到标准分析
                    tracing::warn!("Tree-sitter分析失败，回退到标准分析: {}", e);
                    format!("Git diff:\n{}\nGenerate commit message.", diff.trim())
                }
            }
        } else {
            // 使用标准分析
            format!("Git diff:\n{}\nGenerate commit message.", diff.trim())
        };

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: config.prompts.get("commit").cloned().unwrap_or_else(|| {
                    tracing::warn!("在配置中未找到 Commit Message Generator 提示词，使用空字符串");
                    "".to_string()
                }),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ];
        let req_payload = OpenAIChatRequest {
            model: config.ai.model_name.clone(),
            messages,
            temperature: Some(config.ai.temperature),
            stream: false,
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&req_payload) {
            tracing::debug!("AI req:\n{}", json_str);
        }

        let client = reqwest::Client::new();
        let mut builder = client.post(&config.ai.api_url);
        if let Some(key) = &config.ai.api_key {
            builder = builder.bearer_auth(key);
        }
        let ai_resp = builder
            .json(&req_payload)
            .send()
            .await
            .map_err(AIError::RequestFailed)?;

        if !ai_resp.status().is_success() {
            let code = ai_resp.status();
            let body = ai_resp.text().await.unwrap_or_else(|_| "<no body>".into());
            tracing::error!("AI API 请求失败，状态码 {}: {}", code, body);
            return Err(AppError::AI(AIError::ApiResponseError(code, body)));
        }

        let resp_data = ai_resp
            .json::<OpenAIChatCompletionResponse>()
            .await
            .map_err(AIError::ResponseParseFailed)?;
        let ai_msg = resp_data.choices.get(0).map_or("", |c| &c.message.content);
        let final_msg = clean_ai_output(ai_msg).trim().to_string();

        if final_msg.is_empty() {
            tracing::error!("AI 返回了空消息。");
            return Err(AppError::AI(AIError::EmptyMessage));
        }
        tracing::info!("AI 消息:\n---\n{}\n---", final_msg);

        // 创建提交消息临时文件
        let temp_file_path = create_commit_message_file(&final_msg)?;
        
        // 使用 -F 从文件读取提交消息
        let mut git_args = vec!["commit".to_string(), "-F".to_string(), temp_file_path.to_string_lossy().to_string()];
        
        // Filter out -a, --all from passthrough_args if auto_stage=true, and tree-sitter flags with their values
        for (i, p_arg) in args.passthrough_args.iter().enumerate() {
            // 判断是否是特定标志
            let is_auto_stage_flag = p_arg == "-a" || p_arg == "--all";
            let is_ts_flag = is_tree_sitter_flag(p_arg);
            let is_ts_value = is_tree_sitter_value(&args.passthrough_args, i);
            
            // 处理组合的短标志
            let contains_auto_stage = contains_auto_stage_option(p_arg);
            let contains_ts = contains_tree_sitter_option(p_arg);
            
            if (args.auto_stage && (is_auto_stage_flag || contains_auto_stage)) || 
               is_ts_flag || 
               is_ts_value {
                // 跳过这些标志
                continue;
            }
            
            // 处理包含多个选项的组合短标志
            if contains_auto_stage && contains_ts {
                // 如果同时包含auto-stage和tree-sitter选项，则需要特殊处理
                // 移除 'a' 和 't'
                if let Some(filtered_arg) = create_filtered_short_option(p_arg, &['a', 't']) {
                    if filtered_arg != "-" {
                        git_args.push(filtered_arg.to_string());
                    }
                }
            } else if args.auto_stage && contains_auto_stage {
                // 如果启用了auto-stage，移除 'a'
                if let Some(filtered_arg) = create_filtered_short_option(p_arg, &['a']) {
                    if filtered_arg != "-" {
                        git_args.push(filtered_arg.to_string());
                    }
                }
            } else if contains_ts {
                // 如果包含tree-sitter选项，移除 't'
                if let Some(filtered_arg) = create_filtered_short_option(p_arg, &['t']) {
                    if filtered_arg != "-" {
                        git_args.push(filtered_arg.to_string());
                    }
                }
            } else {
                // 没有特殊处理的情况，直接添加
                git_args.push(p_arg.clone());
            }
        }

        // 使用 git_module 中的函数执行 git commit
        tracing::debug!("执行 Git 命令: git {}", git_args.join(" "));
        
        let result = crate::git_module::passthrough_to_git_with_error_handling(&git_args, true);
        if let Err(e) = result {
            tracing::error!("带有 AI 消息的 Git commit 命令失败。");
            return Err(e);
        }
        // 清理临时文件
        cleanup_commit_message_file(&temp_file_path);
        tracing::info!("使用 AI 消息成功提交。");
    } else {
        return handle_commit_passthrough(args, "(standard commit with --noai)".to_string()).await;
    }
    Ok(())
}