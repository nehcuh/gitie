use crate::ai_module::prompt_handler::send_prompt_and_get_response;
use crate::cli_interface::args::ReviewArgs;
use crate::config_management::settings::{AppConfig, TreeSitterConfig};
use crate::core::errors::AppError;
use crate::git_module::execute_git_command_and_capture_output;
use crate::tree_sitter_analyzer::simple_diff::{parse_simple_diff, detect_language_from_path};
use crate::tree_sitter_analyzer::core::{GitDiff, ChangePattern, ChangeScope, DiffAnalysis};
use crate::tree_sitter_analyzer::analyzer::TreeSitterAnalyzer;
use crate::review_engine::AnalysisDepth;
use std::fs;
use std::io::Write;
use colored::Colorize;
use std::env;

/// Extract diff information for review
///
/// This function gets the diff between specified commits or the current staged changes
async fn extract_diff_for_review(args: &ReviewArgs) -> Result<String, AppError> {
    match (&args.commit1, &args.commit2) {
        (Some(commit1), Some(commit2)) => {
            // Compare two specific commits
            tracing::info!("比较两个指定的提交: {} 和 {}", commit1, commit2);
            let diff_args = vec![
                "diff".to_string(),
                format!("{}..{}", commit1, commit2),
                "--".to_string(),
            ];
            let result = execute_git_command_and_capture_output(&diff_args)?;
            Ok(result.stdout)
        }
        (Some(commit), None) => {
            // Compare one commit with HEAD
            tracing::info!("比较指定的提交与HEAD: {}", commit);
            let diff_args = vec![
                "diff".to_string(),
                format!("{}..HEAD", commit),
                "--".to_string(),
            ];
            let result = execute_git_command_and_capture_output(&diff_args)?;
            Ok(result.stdout)
        }
        (None, None) => {
            // Check if there are staged changes
            let status_result = execute_git_command_and_capture_output(&["status".to_string(), "--porcelain".to_string()])?;
            
            if status_result.stdout.trim().is_empty() {
                return Err(AppError::Generic("没有检测到变更，无法执行代码评审。请先暂存(git add)或提交一些变更。".to_string()));
            }
            
            // If no commit specified, use staged changes or unstaged changes
            let has_staged = status_result.stdout.lines().any(|line| line.starts_with(|c| c == 'M' || c == 'A' || c == 'D' || c == 'R'));
            
            let diff_args = if has_staged {
                tracing::info!("评审已暂存的变更");
                vec!["diff".to_string(), "--staged".to_string()]
            } else {
                tracing::info!("评审工作区的变更");
                vec!["diff".to_string()]
            };
            
            let result = execute_git_command_and_capture_output(&diff_args)?;
            Ok(result.stdout)
        }
        (None, Some(_)) => {
            // This should not happen with the CLI parser, but handle it just in case
            Err(AppError::Generic("如果指定了第二个提交，则必须同时指定第一个提交。".to_string()))
        }
    }
}

/// Determine analysis depth from args
fn get_analysis_depth(args: &ReviewArgs) -> AnalysisDepth {
    match args.depth.to_lowercase().as_str() {
        "shallow" | "basic" => AnalysisDepth::Basic,
        "deep" => AnalysisDepth::Deep,
        _ => AnalysisDepth::Normal, // Default to normal if not recognized
    }
}

/// Determine if TreeSitter should be used
fn should_use_tree_sitter(args: &ReviewArgs) -> bool {
    args.tree_sitter || args.review_ts || (!args.no_tree_sitter)
}

/// Advanced diff analysis using TreeSitter for language-aware parsing
/// Creates a detailed GitDiff structure with structural code analysis
async fn analyze_diff_with_tree_sitter(
    diff_text: &str,
    depth: AnalysisDepth,
) -> Result<(GitDiff, String), AppError> {
    // Initialize Tree-sitter analyzer with config
    let mut config = TreeSitterConfig::default();
    config.analysis_depth = match depth {
        AnalysisDepth::Basic => "shallow".to_string(),
        AnalysisDepth::Normal => "medium".to_string(),
        AnalysisDepth::Deep => "deep".to_string(),
    };
    
    let mut analyzer = TreeSitterAnalyzer::new(config)
        .map_err(|e| AppError::TreeSitter(e))?;
    
    // Parse the diff to get structured representation
    let git_diff = analyzer.parse_git_diff_text(diff_text)
        .map_err(|e| AppError::TreeSitter(e))?;
    
    // Generate analysis summary based on the diff
    let analysis = analyzer.analyze_diff(diff_text)
        .map_err(|e| AppError::TreeSitter(e))?;
    
    // Create a more detailed analysis text from the TreeSitter analysis
    let mut analysis_text = String::new();
    analysis_text.push_str("## 代码变更分析\n\n");
    
    // Add file summary
    analysis_text.push_str("### 变更文件摘要\n\n");
    if git_diff.changed_files.is_empty() {
        analysis_text.push_str("- 未检测到代码变更\n");
    } else {
        for file in &git_diff.changed_files {
            analysis_text.push_str(&format!("- **{}**\n", file.path.display()));
        }
    }
    analysis_text.push_str("\n");
    
    // Add detailed analysis from TreeSitter
    analysis_text.push_str("### 代码结构分析\n\n");
    analysis_text.push_str(&format!("- {}\n\n", analysis.overall_summary));
    
    // 显示变更统计
    let change_analysis = &analysis.change_analysis;
    analysis_text.push_str("#### 变更统计\n\n");
    analysis_text.push_str(&format!("- 函数/方法变更: **{}**\n", change_analysis.function_changes + change_analysis.method_changes));
    analysis_text.push_str(&format!("- 类型/结构变更: **{}**\n", change_analysis.type_changes));
    analysis_text.push_str(&format!("- 接口/特征变更: **{}**\n", change_analysis.interface_changes));
    analysis_text.push_str(&format!("- 其他结构变更: **{}**\n\n", change_analysis.other_changes));
    
    // 按语言分类显示文件
    let mut java_files = Vec::new();
    let mut rust_files = Vec::new();
    let mut other_files = Vec::new();
    
    for file_analysis in &analysis.file_analyses {
        match file_analysis.language.as_str() {
            "java" => java_files.push(file_analysis),
            "rust" => rust_files.push(file_analysis),
            _ => other_files.push(file_analysis),
        }
    }
    
    // 显示 Java 文件变更
    if !java_files.is_empty() {
        analysis_text.push_str("#### Java 文件变更\n\n");
        for file_analysis in &java_files {
            analysis_text.push_str(&format!("- **{}**\n", file_analysis.path.display()));
            
            if let Some(summary) = &file_analysis.summary {
                analysis_text.push_str(&format!("  - {}\n", summary));
            }
            
            if !file_analysis.affected_nodes.is_empty() {
                analysis_text.push_str("  - 受影响的代码结构:\n");
                for node in &file_analysis.affected_nodes {
                    let visibility = if node.is_public { "公开" } else { "私有" };
                    let change_type = match &node.change_type {
                        Some(change) => match change.as_str() {
                            "added" | "added_content" => "➕ ",
                            "deleted" => "❌ ",
                            "modified" | "modified_with_deletion" => "🔄 ",
                            _ => "",
                        },
                        None => "",
                    };
                    
                    // 特殊处理特定节点类型
                    let node_type_display = match node.node_type.as_str() {
                        "spring_component" => "Spring组件",
                        "api_endpoint" => "API端点",
                        "jpa_entity" => "JPA实体",
                        "class_structure" => "类",
                        "overridden_method" => "重写方法",
                        _ => &node.node_type,
                    };
                    
                    analysis_text.push_str(&format!("    - {}**{}** `{}` ({})\n", 
                        change_type, node_type_display, node.name, visibility));
                }
            }
        }
        analysis_text.push_str("\n");
    }
    
    // 显示 Rust 文件变更
    if !rust_files.is_empty() {
        analysis_text.push_str("#### Rust 文件变更\n\n");
        for file_analysis in &rust_files {
            analysis_text.push_str(&format!("- **{}**\n", file_analysis.path.display()));
            
            if let Some(summary) = &file_analysis.summary {
                analysis_text.push_str(&format!("  - {}\n", summary));
            }
            
            if !file_analysis.affected_nodes.is_empty() {
                analysis_text.push_str("  - 受影响的代码结构:\n");
                for node in &file_analysis.affected_nodes {
                    let visibility = if node.is_public { "公开" } else { "私有" };
                    let change_type = match &node.change_type {
                        Some(change) => match change.as_str() {
                            "added" | "added_content" => "➕ ",
                            "deleted" => "❌ ",
                            "modified" | "modified_with_deletion" => "🔄 ",
                            _ => "",
                        },
                        None => "",
                    };
                    
                    // 特殊处理特定节点类型
                    let node_type_display = match node.node_type.as_str() {
                        "debuggable_struct" => "可调试结构体",
                        "test_function" => "测试函数",
                        "macro" => "宏",
                        _ => &node.node_type,
                    };
                    
                    analysis_text.push_str(&format!("    - {}**{}** `{}` ({})\n", 
                        change_type, node_type_display, node.name, visibility));
                }
            }
        }
        analysis_text.push_str("\n");
    }
    
    // 显示其他文件变更
    if !other_files.is_empty() {
        analysis_text.push_str("#### 其他文件变更\n\n");
        for file_analysis in &other_files {
            analysis_text.push_str(&format!("- **{}**\n", file_analysis.path.display()));
            
            if let Some(summary) = &file_analysis.summary {
                analysis_text.push_str(&format!("  - {}\n", summary));
            }
            
            if !file_analysis.affected_nodes.is_empty() {
                analysis_text.push_str("  - 受影响的代码结构:\n");
                for node in &file_analysis.affected_nodes {
                    let visibility = if node.is_public { "公开" } else { "私有" };
                    analysis_text.push_str(&format!("    - {} `{}` ({})\n", 
                        node.node_type, node.name, visibility));
                }
            }
        }
        analysis_text.push_str("\n");
    }
    
    analysis_text.push_str("### 评审重点及建议\n\n");
    
    // 根据变更类型给出评审建议
    match &analysis.change_analysis.change_pattern {
        ChangePattern::FeatureImplementation => {
            analysis_text.push_str("- 🆕 **新功能实现**\n");
            analysis_text.push_str("  - 建议关注功能完整性和边界情况处理\n");
            analysis_text.push_str("  - 确认是否有足够的测试覆盖新功能\n");
            analysis_text.push_str("  - 评估与现有系统的集成是否顺畅\n");
        },
        ChangePattern::BugFix => {
            analysis_text.push_str("- 🐛 **Bug修复**\n");
            analysis_text.push_str("  - 确认修复是否解决了根本问题\n");
            analysis_text.push_str("  - 检查是否有回归测试防止问题再次出现\n");
            analysis_text.push_str("  - 评估是否可能引入新的问题\n");
        },
        ChangePattern::Refactoring => {
            analysis_text.push_str("- ♻️ **代码重构**\n");
            analysis_text.push_str("  - 关注功能等价性，确保重构不改变行为\n");
            analysis_text.push_str("  - 检查性能影响，尤其是循环和算法改变\n");
            analysis_text.push_str("  - 评估可维护性和可读性的提升\n");
        },
        ChangePattern::ModelChange => {
            analysis_text.push_str("- 🏗️ **模型变更**\n");
            analysis_text.push_str("  - 关注数据结构变化对系统的影响\n");
            analysis_text.push_str("  - 检查是否需要数据迁移或兼容处理\n");
            analysis_text.push_str("  - 评估模型变更的文档是否更新\n");
        },
        ChangePattern::BehaviorChange => {
            analysis_text.push_str("- 🔄 **行为变更**\n");
            analysis_text.push_str("  - 关注API合约是否发生变化\n");
            analysis_text.push_str("  - 检查依赖方是否需要适配\n");
            analysis_text.push_str("  - 评估行为变更是否有充分的测试验证\n");
        },
        ChangePattern::ConfigurationChange => {
            analysis_text.push_str("- ⚙️ **配置变更**\n");
            analysis_text.push_str("  - 关注配置变更对不同环境的影响\n");
            analysis_text.push_str("  - 检查默认值和边界值处理\n");
            analysis_text.push_str("  - 评估文档是否同步更新\n");
        },
        ChangePattern::LanguageSpecificChange(lang_change) => {
            if lang_change.starts_with("Java") {
                analysis_text.push_str("- ☕ **Java特定变更**\n");
                if lang_change.contains("Structural") {
                    analysis_text.push_str("  - 关注类结构变化和继承关系\n");
                } else if lang_change.contains("Visibility") {
                    analysis_text.push_str("  - 关注访问权限变更对客户端代码的影响\n");
                } else {
                    analysis_text.push_str("  - 关注Java特定语言特性使用是否合理\n");
                }
            } else if lang_change.starts_with("Rust") {
                analysis_text.push_str("- 🦀 **Rust特定变更**\n");
                if lang_change.contains("Trait") {
                    analysis_text.push_str("  - 关注trait实现和泛型约束\n");
                } else if lang_change.contains("Macro") {
                    analysis_text.push_str("  - 关注宏定义的正确性和安全性\n");
                } else {
                    analysis_text.push_str("  - 关注所有权和生命周期管理\n");
                }
            } else {
                analysis_text.push_str("- 🔧 **特定语言变更**\n");
                analysis_text.push_str("  - 关注语言特定惯用法和最佳实践\n");
            }
        },
        _ => {
            analysis_text.push_str("- ℹ️ **代码评审**\n");
            analysis_text.push_str("  - 使用 AI 进行深度评审，提供详细反馈\n");
        }
    }
    
    // 根据变更范围提供额外建议
    match &analysis.change_analysis.change_scope {
        ChangeScope::Minor => {
            analysis_text.push_str("\n- 🔍 **轻微变更**\n");
            analysis_text.push_str("  - 可以进行快速评审\n");
            analysis_text.push_str("  - 重点关注变更的准确性\n");
        },
        ChangeScope::Moderate => {
            analysis_text.push_str("\n- 🔎 **中等变更**\n");
            analysis_text.push_str("  - 建议进行完整评审\n");
            analysis_text.push_str("  - 关注变更的完整性和一致性\n");
        },
        ChangeScope::Major => {
            analysis_text.push_str("\n- 🔬 **重大变更**\n");
            analysis_text.push_str("  - 建议安排多人详细评审\n");
            analysis_text.push_str("  - 考虑分阶段合并或更多测试\n");
            analysis_text.push_str("  - 特别关注向后兼容性和稳定性\n");
        }
    }
    
    Ok((git_diff, analysis_text))
}

/// Generate the prompt for AI review
async fn generate_ai_review_prompt(
    _config: &AppConfig,
    diff_text: &str,
    analysis: &str,
    args: &ReviewArgs,
    _git_diff: &GitDiff,
    languages: &str,
) -> Result<String, AppError> {
    // 更丰富的基础提示，强调结构化分析
    let base_prompt = format!(
        "你是一位经验丰富的代码评审专家，精通多种编程语言，特别是{}。\
        你擅长识别代码中的潜在问题、安全隐患和性能瓶颈，并提供具体的改进建议。\
        请根据TreeSitter提供的结构化分析，对以下代码变更进行全面评审。",
        if languages.is_empty() { "各种编程语言".to_string() } else { languages.to_string() }
    );
    
    // 更具体的关注点指示
    let focus_instruction = if let Some(focus) = &args.focus {
        format!("请特别关注以下方面: {}", focus)
    } else {
        "请全面评审代码，特别关注以下方面：\n\
        1. 代码质量和最佳实践\n\
        2. 可能的安全隐患或漏洞\n\
        3. 性能优化机会\n\
        4. 可读性和可维护性\n\
        5. 与现有代码的集成和兼容性".to_string()
    };
    
    // 添加结构化评审指南
    let review_guide = "请提供结构化的评审，包括：\n\
        1. 总体评价：变更的整体质量和目的\n\
        2. 问题列表：发现的具体问题，每个问题包含：\n\
           - 问题位置和描述\n\
           - 问题严重程度\n\
           - 改进建议\n\
        3. 改进建议：如何提升代码质量\n\
        4. 总结：最重要的1-3个需要关注的点";
    
    // Combine everything into the final prompt
    let prompt = format!(
        "{}\n\n## 代码评审请求\n\n{}\n\n## 评审指南\n\n{}\n\n## TreeSitter结构分析\n\n{}\n\n## 代码变更\n\n```diff\n{}\n```",
        base_prompt, focus_instruction, review_guide, analysis, diff_text
    );
    
    Ok(prompt)
}


/// 展开路径中的波浪号(~)为用户主目录
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Ok(home) = env::var("HOME") {
            return path.replacen("~", &home, 1);
        }
        
        // 尝试通过其他方式获取主目录
        if let Some(home_dir) = dirs_next::home_dir() {
            if let Some(home_str) = home_dir.to_str() {
                return path.replacen("~", home_str, 1);
            }
        }
    }
    
    // 如果无法展开或路径不包含波浪号，返回原始路径
    path.to_string()
}

/// Format and save or display review results
async fn format_and_output_review(
    review_text: &str, 
    args: &ReviewArgs
) -> Result<(), AppError> {
    // Process based on requested format
    let formatted_output = match args.format.to_lowercase().as_str() {
        "json" => {
            // Convert to JSON format
            serde_json::json!({
                "review": review_text,
                "timestamp": "2023-01-01T00:00:00Z",
                "format_version": "1.0"
            }).to_string()
        },
        "html" => {
            // Convert to simple HTML
            format!(
                "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"UTF-8\">\n\
                <title>Gitie Code Review</title>\n\
                <style>body {{ font-family: Arial, sans-serif; margin: 20px; }}</style>\n\
                </head>\n<body>\n\
                <h1>Gitie 代码评审报告</h1>\n\
                <div>{}</div>\n\
                <p><em>由 Gitie 生成</em></p>\n\
                </body>\n</html>",
                review_text.replace("\n", "<br>")
            )
        },
        _ => {
            // Default to text format (markdown)
            review_text.to_string()
        }
    };
    
    // Output to file or stdout
    if let Some(output_file) = &args.output {
        // 展开波浪号为用户主目录
        let expanded_path = expand_tilde(output_file);
        tracing::debug!("输出路径从 {} 展开为 {}", output_file, expanded_path);
        
        let mut file = fs::File::create(&expanded_path)
            .map_err(|e| AppError::IO(format!("无法创建输出文件: {}", expanded_path), e))?;
            
        file.write_all(formatted_output.as_bytes())
            .map_err(|e| AppError::IO(format!("写入输出文件时发生错误: {}", expanded_path), e))?;
            
        println!("评审结果已保存到: {}", expanded_path);
    } else {
        // Print to console with some formatting
        println!("{}", "代码评审结果".bold().green());
        println!("{}", "=============".green());
        println!("\n{}", formatted_output);
    }
    
    Ok(())
}

/// Main handler for the review command
pub async fn handle_review(args: ReviewArgs, config: &AppConfig) -> Result<(), AppError> {
    tracing::info!("执行代码评审");
    
    // Extract the Git diff
    let diff_text = extract_diff_for_review(&args).await?;
    
    if diff_text.trim().is_empty() {
        return Err(AppError::Generic("没有检测到代码变更，无法执行评审。".to_string()));
    }
    
    // Determine analysis depth
    let depth = get_analysis_depth(&args);
    tracing::info!("使用分析深度: {:?}", depth);
    
    // Determine if TreeSitter should be used
    let use_tree_sitter = should_use_tree_sitter(&args);
    
    // Analyze the diff with appropriate analyzer
    let (git_diff, analysis_text, analysis_results) = if use_tree_sitter {
        tracing::info!("使用TreeSitter进行深度代码分析");
        let (diff, text) = analyze_diff_with_tree_sitter(&diff_text, depth).await?;
        // 获取额外的分析结果用于语言信息
        let mut analyzer = TreeSitterAnalyzer::new(TreeSitterConfig::default())
            .map_err(|e| AppError::TreeSitter(e))?;
        let analysis_obj = analyzer.analyze_diff(&diff_text)
            .map_err(|e| AppError::TreeSitter(e))?;
        (diff, text, Some(analysis_obj))
    } else {
        tracing::info!("使用简化的代码分析");
        // Fallback to simple diff parser
        let git_diff = parse_simple_diff(&diff_text);
        
        // Create a basic analysis
        let mut simple_analysis = String::new();
        simple_analysis.push_str("## 代码变更分析\n\n");
        simple_analysis.push_str("### 变更文件摘要\n\n");
        
        if git_diff.changed_files.is_empty() {
            simple_analysis.push_str("- 未检测到代码变更\n");
        } else {
            for file in &git_diff.changed_files {
                simple_analysis.push_str(&format!("- **{}**\n", file.path.display()));
            }
        }
        
        simple_analysis.push_str("\n### 初步分析结果\n\n");
        simple_analysis.push_str("- ℹ️ **简化分析模式**\n");
        simple_analysis.push_str("  - 未启用TreeSitter进行深度分析\n");
        
        (git_diff, simple_analysis, None)
    };
    
    // 为AI审查增加更多有用的上下文
    let language_info = if let Some(ref analysis) = analysis_results {
        // 从TreeSitter分析中获取详细语言信息
        analysis.file_analyses.iter()
            .filter(|f| !f.language.is_empty() && f.language != "unknown" && f.language != "error" && f.language != "text")
            .map(|f| f.language.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        // 从文件扩展名猜测语言
        git_diff.changed_files.iter()
            .filter_map(|f| detect_language_from_path(&f.path))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    };
    
    // Generate AI prompt with enhanced context
    let prompt = generate_ai_review_prompt(config, &diff_text, &analysis_text, &args, &git_diff, &language_info).await?;
    
    // Try to send to AI
    tracing::info!("发送至 AI 进行代码评审");
    let ai_response = match send_prompt_and_get_response(
        config, 
        &prompt,
        "您是一位经验丰富的代码评审专家，精通多种编程语言和软件开发最佳实践。"
    ).await {
        Ok(response) => response,
        Err(e) => {
            // 如果AI请求失败，使用简单结果
            tracing::warn!("AI请求失败: {}，使用简单评审结果", e);
            let mut simple_response = String::new();
            simple_response.push_str("# 代码评审结果\n\n");
            simple_response.push_str("无法连接到 AI 服务，请检查网络连接和 API 配置。\n\n");
            simple_response.push_str("## 基本代码检查\n\n");
            simple_response.push_str("- 检测到代码变更\n");
            simple_response.push_str("- 建议手动检查代码质量和安全性\n");
            simple_response
        }
    };
    
    // Format and output the review
    format_and_output_review(&ai_response, &args).await?;
    
    Ok(())
}

/// Handler for the commit command with review option
pub async fn handle_commit_with_review(
    args: &crate::cli_interface::args::CommitArgs, 
    config: &AppConfig
) -> Result<bool, AppError> {
    if !args.review {
        return Ok(false); // Review not requested
    }
    
    tracing::info!("执行提交前代码评审");
    
    // Extract staged changes for review
    let diff_args = vec!["diff".to_string(), "--staged".to_string()];
    let result = execute_git_command_and_capture_output(&diff_args)?;
    let diff_text = result.stdout;
    
    if diff_text.trim().is_empty() {
        println!("{}", "没有已暂存的变更，跳过代码评审。".yellow());
        return Ok(false);
    }
    
    // Create ReviewArgs from CommitArgs
    let review_args = ReviewArgs {
        depth: "normal".to_string(),
        focus: None,
        lang: None,
        format: "text".to_string(),
        output: None,
        tree_sitter: args.tree_sitter.is_some(),
        no_tree_sitter: false,
        review_ts: false,
        passthrough_args: vec![],
        commit1: None,
        commit2: None,
    };
    
    // Parse the diff to create GitDiff and generate a basic analysis
    let git_diff = parse_simple_diff(&diff_text);
    
    // Create a basic analysis text
    let mut analysis_text = String::from("## 初步代码检查\n\n提交前评审，检查代码质量和潜在问题。\n");
    
    // Add file information if available
    if !git_diff.changed_files.is_empty() {
        analysis_text.push_str("\n### 检测到的文件：\n");
        for file in &git_diff.changed_files {
            analysis_text.push_str(&format!("- {}\n", file.path.display()));
        }
    }
    
    // Detect languages from file extensions
    let language_info = git_diff.changed_files.iter()
        .filter_map(|f| detect_language_from_path(&f.path))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    
    // Try to get AI review
    let review_text = match generate_ai_review_prompt(config, &diff_text, &analysis_text, &review_args, &git_diff, &language_info).await {
        Ok(prompt) => {
            match send_prompt_and_get_response(
                config, 
                &prompt,
                "您是一位经验丰富的代码评审专家，精通多种编程语言和软件开发最佳实践。"
            ).await {
                Ok(response) => response,
                Err(_) => {
                    // Fall back to simple message
                    "# 代码评审结果\n\n无法连接到 AI 服务，请检查网络连接和 API 配置。\n\n建议手动检查代码质量后再提交。".to_string()
                }
            }
        },
        Err(_) => {
            // Fall back to simple message
            "# 代码评审结果\n\n无法生成代码评审提示，请检查配置。\n\n建议手动检查代码质量后再提交。".to_string()
        }
    };
    
    // Display the review
    println!("{}", "提交前代码评审结果".bold().green());
    println!("{}", "===================".green());
    println!("\n{}", review_text);
    
    // Ask user if they want to continue with the commit
    print!("\n{} (y/n): ", "是否继续提交？".bold().yellow());
    std::io::stdout().flush().unwrap();
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    if input.trim().to_lowercase() == "y" {
        println!("继续提交...");
        Ok(false) // Continue with commit
    } else {
        println!("取消提交。");
        Ok(true) // Cancel commit
    }
}