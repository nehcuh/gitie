use crate::ai_module::prompt_handler::{send_prompt_and_get_response, load_prompt_file, get_prompt_directories};
use crate::cli_interface::args::ReviewArgs;
use crate::config_management::settings::AppConfig;
use crate::core::errors::AppError;
use crate::git_module::execute_git_command_and_capture_output;
use crate::tree_sitter_analyzer::analyzer::TreeSitterAnalyzer;
use crate::tree_sitter_analyzer::analyzer::TreeSitterConfig;
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

/// Map diff to language-appropriate prompts
async fn analyze_diff_with_tree_sitter(
    diff_text: &str,
    depth: AnalysisDepth,
) -> Result<(GitDiff, String), AppError> {
    // Initialize Tree-sitter analyzer with config
    let config = TreeSitterConfig::default();
    let mut analyzer = TreeSitterAnalyzer::new(config)?;
    
    // Parse the diff to get structured representation
    let git_diff = analyzer.parse_git_diff_text(diff_text)?;
    
    // Generate analysis summary based on the diff
    let analysis = analyzer.analyze_diff(diff_text)?;
    
    // Format analysis results
    let mut analysis_text = String::new();
    analysis_text.push_str("## 代码变更分析\n\n");
    
    // Add files summary
    analysis_text.push_str("### 变更文件摘要\n\n");
    for file in &git_diff.changed_files {
        analysis_text.push_str(&format!("- **{}** ({})\n", file.path, file.change_type));
    }
    analysis_text.push_str("\n");
    
    // Add structure changes from file analyses
    analysis_text.push_str("### 结构变更\n\n");
    
    // Extract functions from file analyses
    let mut functions_found = false;
    for file_analysis in &analysis.file_analyses {
        if let Some(file_structure) = &file_analysis.structure {
            if !file_structure.functions.is_empty() {
                if !functions_found {
                    analysis_text.push_str("#### 函数变更\n\n");
                    functions_found = true;
                }
                
                for func in &file_structure.functions {
                    analysis_text.push_str(&format!("- **{}**\n", func.name));
                    if let Some(desc) = &func.description {
                        analysis_text.push_str(&format!("  - {}\n", desc));
                    }
                }
            }
        }
    }
    
    if functions_found {
        analysis_text.push_str("\n");
    }
    
    // Extract classes from file analyses
    let mut classes_found = false;
    for file_analysis in &analysis.file_analyses {
        if let Some(file_structure) = &file_analysis.structure {
            if !file_structure.classes.is_empty() {
                if !classes_found {
                    analysis_text.push_str("#### 类型/类变更\n\n");
                    classes_found = true;
                }
                
                for class in &file_structure.classes {
                    analysis_text.push_str(&format!("- **{}**\n", class.name));
                    if let Some(desc) = &class.description {
                        analysis_text.push_str(&format!("  - {}\n", desc));
                    }
                }
            }
        }
    }
    
    if classes_found {
        analysis_text.push_str("\n");
    }
    
    // Add language detection
    let mut language_counts = std::collections::HashMap::new();
    for file_analysis in &analysis.file_analyses {
        if let Some(lang) = &file_analysis.language {
            *language_counts.entry(lang.clone()).or_insert(0) += 1;
        }
    }
    
    if !language_counts.is_empty() {
        analysis_text.push_str("### 检测到的语言\n\n");
        for (lang, count) in language_counts {
            analysis_text.push_str(&format!("- {}: {} 文件\n", lang, count));
        }
        analysis_text.push_str("\n");
    }
    
    Ok((git_diff, analysis_text))
}

/// Generate the prompt for AI review
async fn generate_ai_review_prompt(
    config: &AppConfig,
    diff_text: &str,
    analysis: &str,
    args: &ReviewArgs,
    git_diff: &GitDiff,
) -> Result<String, AppError> {
    // Get prompt directories and load the base prompt
    let prompt_dirs = vec!["assets".to_string()];
    let base_prompt = load_prompt_file("expert-prompt.md", &prompt_dirs)?;
    
    // Determine languages for specialized prompts
    let mut detected_languages = Vec::new();
    
    // If user specified a language, use that
    if let Some(lang) = &args.lang {
        detected_languages.push(lang.to_lowercase());
    } else {
        // Otherwise use languages from the diff
        for file in &git_diff.changed_files {
            if let Some(lang) = get_language_from_file_path(&file.path.to_string_lossy()) {
                if !detected_languages.contains(&lang) {
                    detected_languages.push(lang);
                }
            }
        }
    }
    
    // Load language-specific prompts and combine them
    let mut language_prompts = String::new();
    for lang in &detected_languages {
        let prompt_file = format!("review-{}-prompt.md", lang);
        tracing::info!("加载语言特定提示: {}", prompt_file);
        
        match load_prompt_file(&prompt_file, &prompt_dirs) {
            Ok(content) => {
                language_prompts.push_str(&content);
                language_prompts.push_str("\n\n");
            }
            Err(e) => {
                tracing::warn!("无法加载语言提示 {}: {}", prompt_file, e);
                // Continue even if we can't load a specific language prompt
            }
        }
    }
    
    // Determine focus areas
    let focus_instruction = if let Some(focus) = &args.focus {
        format!("请特别关注以下方面: {}", focus)
    } else {
        "请全面评审代码，关注安全性、性能、可读性和最佳实践".to_string()
    };
    
    // Combine everything into the final prompt
    let prompt = format!(
        "{}\n\n{}\n\n## 代码评审请求\n\n{}\n\n## Tree-sitter分析\n\n{}\n\n## 代码变更\n\n```diff\n{}\n```",
        base_prompt, language_prompts, focus_instruction, analysis, diff_text
    );
    
    Ok(prompt)
}

/// Attempt to determine the programming language from a file path
fn get_language_from_file_path(path: &str) -> Option<String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())?;
    
    match extension.to_lowercase().as_str() {
        "rs" => Some("rust".to_string()),
        "py" => Some("python".to_string()),
        "java" => Some("java".to_string()),
        "js" | "jsx" | "ts" | "tsx" => Some("js".to_string()),
        "c" | "h" => Some("c".to_string()),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "h" => Some("cpp".to_string()),
        "go" => Some("go".to_string()),
        _ => None,
    }
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
            let now = chrono::Utc::now();
            serde_json::json!({
                "review": review_text,
                "timestamp": now.to_rfc3339(),
                "format_version": "1.0"
            }).to_string()
        },
        "html" => {
            // Convert to HTML
            let mut html = String::new();
            html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
            html.push_str("<meta charset=\"UTF-8\">\n");
            html.push_str("<title>Gitie Code Review</title>\n");
            html.push_str("<style>\n");
            html.push_str("body { font-family: Arial, sans-serif; margin: 20px; }\n");
            html.push_str("h1, h2, h3 { color: #333; }\n");
            html.push_str("pre { background-color: #f5f5f5; padding: 10px; border-radius: 5px; }\n");
            html.push_str(".security { color: #d73a49; }\n");
            html.push_str(".performance { color: #e36209; }\n");
            html.push_str(".style { color: #6f42c1; }\n");
            html.push_str(".suggestion { color: #005cc5; }\n");
            html.push_str("</style>\n");
            html.push_str("</head>\n<body>\n");
            html.push_str("<h1>Gitie 代码评审报告</h1>\n");
            
            // Simple markdown-to-html conversion
            let markdown = review_text.replace("\n\n", "\n<p>\n")
                .replace("# ", "<h1>").replace("\n## ", "\n<h2>").replace("\n### ", "\n<h3>")
                .replace("```diff", "<pre>").replace("```", "</pre>");
            
            html.push_str(&markdown);
            let now = chrono::Local::now();
            html.push_str("\n<p><em>由 Gitie 生成于 ");
            html.push_str(&now.to_rfc3339());
            html.push_str("</em></p>\n</body>\n</html>");
            
            html
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
    
    // Analyze with Tree-sitter if enabled
    let (git_diff, analysis_text) = if should_use_tree_sitter(&args) {
        tracing::info!("使用 Tree-sitter 进行代码分析");
        analyze_diff_with_tree_sitter(&diff_text, depth).await?
    } else {
        // Simple fallback without Tree-sitter
        tracing::info!("不使用 Tree-sitter 进行代码分析");
        let config = TreeSitterConfig::default();
        let analyzer = TreeSitterAnalyzer::new(config)?;
        let git_diff = analyzer.parse_git_diff_text(&diff_text)?;
        (git_diff, "未使用 Tree-sitter 进行深度分析。".to_string())
    };
    
    // Generate AI prompt
    let prompt = generate_ai_review_prompt(config, &diff_text, &analysis_text, &args, &git_diff).await?;
    
    // Send to AI and get review
    tracing::info!("发送至 AI 进行代码评审");
    let ai_response = send_prompt_and_get_response(
        config, 
        &prompt,
        "您是一位经验丰富的代码评审专家，精通多种编程语言和软件开发最佳实践。"
    ).await?;
    
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
    
    /// Determine analysis depth
    let depth = if let Some(ts_value) = &args.tree_sitter {
        match ts_value.to_lowercase().as_str() {
            "shallow" | "basic" => AnalysisDepth::Basic,
            "deep" => AnalysisDepth::Deep,
            _ => AnalysisDepth::Normal,
        }
    } else {
        AnalysisDepth::Normal
    };
    
    // Analyze with Tree-sitter - using await since the function is async
    let (git_diff, analysis_text) = analyze_diff_with_tree_sitter(&diff_text, depth).await?;
    
    // Generate AI prompt
    let prompt = generate_ai_review_prompt(config, &diff_text, &analysis_text, &review_args, &git_diff).await?;
    
    // Send to AI and get review
    tracing::info!("发送至 AI 进行代码评审");
    let ai_response = send_prompt_and_get_response(
        config, 
        &prompt,
        "您是一位经验丰富的代码评审专家，精通多种编程语言和软件开发最佳实践。"
    ).await?;
    
    // Display the review
    println!("{}", "提交前代码评审结果".bold().green());
    println!("{}", "===================".green());
    println!("\n{}", ai_response);
    
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