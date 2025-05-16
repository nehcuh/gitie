use crate::{
    ai_utils::{ChatMessage, OpenAIChatCompletionResponse, OpenAIChatRequest, clean_ai_output},
    cli::CommitArgs,
    config::AppConfig,
    errors::{AIError, AppError, GitError},
    git_commands::map_output_to_git_command_error,
    tree_sitter_analyzer::TreeSitterAnalyzer,
};
use std::process::Command as StdCommand;

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

    let mut cmd_builder = StdCommand::new("git");
    cmd_builder.arg("commit");

    // Add -a/--all flag if auto_stage is set
    if args.auto_stage {
        cmd_builder.arg("-a");
    }

    if let Some(message) = &args.message {
        cmd_builder.arg("-m").arg(message);
    }

    // Add remaining args, but exclude -a and -all if auto_stage is true
    for arg in &args.passthrough_args {
        if !(args.auto_stage
            && (arg == "-a"
                || arg == "--all"
                || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('a'))))
        {
            cmd_builder.arg(arg);
        }
    }

    let cmd_desc = format!(
        "commit (passthrough {}) args: {:?}",
        context_msg, args.passthrough_args
    );
    let status = cmd_builder
        .status()
        .map_err(|e| AppError::IO(format!("Failed git {}", cmd_desc), e))?;
    if !status.success() {
        tracing::error!("传递 git {} 失败，状态码 {}", cmd_desc, status);
        return Err(AppError::Git(GitError::PassthroughFailed {
            command: format!("git {}", cmd_desc),
            status_code: status.code(),
        }));
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
    if args.tree_sitter.is_some() {
        tracing::info!("通过命令行参数启用Tree-sitter分析");
        return true;
    }

    // 否则使用配置文件中的设置
    if config.tree_sitter.enabled {
        tracing::info!("通过配置文件启用Tree-sitter分析");
        return true;
    }

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
fn generate_enhanced_prompt_with_tree_sitter(
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
    if let Ok(output) = StdCommand::new("git")
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
        Ok(analysis) => {
            // 生成增强提示
            let enhanced_prompt = format!(
                "Git diff:\n{}\n\n{}\nGenerate commit message.",
                diff_text.trim(),
                analyzer.generate_commit_prompt(&analysis)
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
            let add_result = StdCommand::new("git")
                .arg("add")
                .arg("-u")
                .output()
                .map_err(|e| AppError::IO("Failed to auto-stage changes".to_string(), e))?;

            if !add_result.status.success() {
                tracing::error!("使用 git add -u 自动暂存更改失败");
                return Err(map_output_to_git_command_error("git add -u", add_result).into());
            }
        }

        let diff_out = StdCommand::new("git")
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
            match generate_enhanced_prompt_with_tree_sitter(&diff, config, &args) {
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
                    tracing::warn!("在配置中未找到提交提示词，使用空字符串");
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

        let mut cmd_builder = StdCommand::new("git");
        cmd_builder.arg("commit").arg("-m").arg(&final_msg);

        // Filter out -a and --all from passthrough_args if auto_stage=true
        for p_arg in &args.passthrough_args {
            if p_arg != "-a"
                && p_arg != "--all"
                && !(p_arg.starts_with('-') && !p_arg.starts_with("--") && p_arg.contains('a'))
            {
                cmd_builder.arg(p_arg);
            }
        }

        let commit_out = cmd_builder
            .output()
            .map_err(|e| AppError::IO("AI commit failed".into(), e))?;
        if !commit_out.status.success() {
            tracing::error!("带有 AI 消息的 Git commit 命令失败。");
            return Err(map_output_to_git_command_error("git commit -m <AI>", commit_out).into());
        }
        tracing::info!("使用 AI 消息成功提交。");
    } else {
        return handle_commit_passthrough(args, "(standard commit with --noai)".to_string()).await;
    }
    Ok(())
}