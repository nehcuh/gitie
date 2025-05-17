use crate::ai_module::prompt_handler::send_prompt_and_get_response;
use crate::cli_interface::args::ReviewArgs;
use crate::config_management::settings::AppConfig;
use crate::core::errors::AppError;
use crate::git_module::execute_git_command_and_capture_output;
use crate::tree_sitter_analyzer::simple_diff::{parse_simple_diff, detect_language_from_path};
use crate::tree_sitter_analyzer::core::GitDiff;
use crate::review_engine::AnalysisDepth;
use std::path::Path;
use std::fs;
use std::io::Write;
use colored::Colorize;

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

/// Simplified diff analysis using basic parsing
/// Create a simple GitDiff structure and basic analysis
async fn analyze_diff_with_tree_sitter(
    diff_text: &str,
    _depth: AnalysisDepth,
) -> Result<(GitDiff, String), AppError> {
    // Use the simplified diff parser instead of TreeSitterAnalyzer
    let git_diff = parse_simple_diff(diff_text);
    
    // Create a simplified analysis result
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
    
    // Add simplified analysis
    analysis_text.push_str("### 初步分析结果\n\n");
    analysis_text.push_str("- ℹ️ **代码评审**\n");
    analysis_text.push_str("  - 使用 AI 进行深度评审，提供详细反馈\n");
    
    Ok((git_diff, analysis_text))
}

/// Generate the prompt for AI review
async fn generate_ai_review_prompt(
    _config: &AppConfig,
    diff_text: &str,
    analysis: &str,
    args: &ReviewArgs,
    _git_diff: &GitDiff,
) -> Result<String, AppError> {
    // Simplified base prompt
    let base_prompt = "你是一位经验丰富的代码评审专家，精通多种编程语言。请对以下代码变更进行评审。";
    
    // Simplified focus instruction
    let focus_instruction = if let Some(focus) = &args.focus {
        format!("请特别关注以下方面: {}", focus)
    } else {
        "请全面评审代码，关注安全性、性能、可读性和最佳实践".to_string()
    };
    
    // Combine everything into the final prompt
    let prompt = format!(
        "{}\n\n## 代码评审请求\n\n{}\n\n## 初步分析\n\n{}\n\n## 代码变更\n\n```diff\n{}\n```",
        base_prompt, focus_instruction, analysis, diff_text
    );
    
    Ok(prompt)
}

/// Wrapper for the detect_language_from_path function
fn get_language_from_file_path(path: &str) -> Option<String> {
    let path_buf = std::path::PathBuf::from(path);
    detect_language_from_path(&path_buf)
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
        let mut file = fs::File::create(output_file)
            .map_err(|e| AppError::IO(format!("无法创建输出文件: {}", output_file), e))?;
            
        file.write_all(formatted_output.as_bytes())
            .map_err(|e| AppError::IO(format!("写入输出文件时发生错误: {}", output_file), e))?;
            
        println!("评审结果已保存到: {}", output_file);
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
    
    // Use simplified analysis
    tracing::info!("使用简化的代码分析");
    let (git_diff, analysis_text) = analyze_diff_with_tree_sitter(&diff_text, depth).await?;
    
    // Generate AI prompt
    let prompt = generate_ai_review_prompt(config, &diff_text, &analysis_text, &args, &git_diff).await?;
    
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
    
    // Try to get AI review
    let review_text = match generate_ai_review_prompt(config, &diff_text, &analysis_text, &review_args, &git_diff).await {
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