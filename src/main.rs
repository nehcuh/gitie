mod ai_explainer;
mod ai_utils;
mod cli;
mod commit_commands;
mod config;
mod errors;
mod git_commands;
mod tree_sitter_analyzer;
mod types;

use crate::ai_explainer::{explain_git_command, explain_git_command_output, explain_git_error};
use crate::cli::{GitieArgs, GitieSubCommand, args_contain_help, should_use_ai};
use crate::commit_commands::handle_commit;
use crate::config::AppConfig;
use crate::errors::{AppError, GitError};
use crate::git_commands::{
    execute_git_command_and_capture_output, is_git_available, is_in_git_repository,
    passthrough_to_git, passthrough_to_git_with_error_handling,
};
use clap::Parser;

/// 过滤命令参数，移除tree-sitter相关标志
fn filter_tree_sitter_args(args: &[String]) -> Vec<String> {
    let mut filtered = Vec::new();
    let mut skip_next = false;
    
    // 检查第一个参数是否是tree-sitter，这可能是误用
    if !args.is_empty() && args[0] == "tree-sitter" {
        tracing::info!("检测到误用形式: tree-sitter作为第一个参数");
        // 如果传入的第一个参数是tree-sitter，可能是"gitie tree-sitter commit"这样的形式
        // 在这种情况下，我们忽略tree-sitter并处理其它参数
        if args.len() > 1 {
            return args[1..].to_vec();
        }
    }
    
    for (i, arg) in args.iter().enumerate() {
        // 如果当前参数需要被跳过（因为它是前一个标志的值）
        if skip_next {
            skip_next = false;
            tracing::debug!("跳过tree-sitter参数值: {}", arg);
            continue;
        }
        
        // 检查是否是tree-sitter标志
        if arg == "--tree-sitter" || arg == "-t" {
            // 如果下一个参数存在且不是以'-'开头，它是值参数，需要跳过
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                tracing::debug!("标记跳过下一个tree-sitter值参数: {}", args[i + 1]);
                skip_next = true;
            }
            tracing::debug!("过滤tree-sitter标志: {}", arg);
            continue;
        }
        
        // 检查带有值的tree-sitter标志 (--tree-sitter=value)
        if arg.starts_with("--tree-sitter=") {
            continue;
        }
        
        // 检查是否是"tree-sitter"（无前缀）- 在某些情况下可能被误解
        if arg == "tree-sitter" {
            continue;
        }
        
        // 处理短标志组合 (如 -at)
        if arg.starts_with('-') && !arg.starts_with("--") && arg.contains('t') {
            // 创建不包含't'的新标志
            let new_arg: String = arg.chars()
                .filter(|&c| c != 't')
                .collect();
            
            // 如果过滤后不只是 "-"，则添加
            if new_arg != "-" {
                filtered.push(new_arg);
            }
            continue;
        }
        
        // 正常的参数，添加到过滤后的列表
        filtered.push(arg.clone());
    }
    
    tracing::debug!("过滤前参数: {:?}", args);
    tracing::debug!("过滤后参数: {:?}", filtered);
    
    filtered
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_app());

    if let Err(e) = result {
        tracing::error!("应用程序失败: {}", e);
        let exit_code = match e {
            AppError::Git(GitError::PassthroughFailed { status_code, .. }) => {
                status_code.unwrap_or(128)
            }
            AppError::Git(GitError::CommandFailed { status_code, .. }) => {
                status_code.unwrap_or(128)
            }
            _ => 1,
        };
        std::process::exit(exit_code);
    }
}

/// Execute Git command and handle potential errors
///
/// This function is used when executing regular Git commands and provides AI explanations when commands fail
async fn execute_git_command_with_error_handling(
    config: &AppConfig,
    args: &[String],
    use_ai: bool,
) -> Result<(), AppError> {
    // 特殊处理: 直接检查所有可能的commit命令形式
    // 注意：这是一个跳过正常git命令执行的重要逻辑
    if !args.is_empty() {
        let contains_commit = args.iter().any(|arg| arg == "commit");
        if contains_commit || (args.len() > 1 && args[1] == "commit") {
            tracing::info!("在execute_git_command中检测到commit命令，重定向到handle_commit: {:?}", args);
            
            // 重新构建命令，确保commit在第一位置
            let mut commit_args_vec = vec!["gitie".to_string(), "commit".to_string()];
            for arg in args.iter().filter(|a| *a != "commit") {
                commit_args_vec.push(arg.clone());
            }
            
            tracing::debug!("重构的commit命令: {:?}", commit_args_vec);
            
            if let Ok(parsed_args) = GitieArgs::try_parse_from(&commit_args_vec) {
                match parsed_args.command {
                    GitieSubCommand::Commit(commit_args) => {
                        return handle_commit(commit_args, &config).await;
                    }
                }
            } else {
                tracing::warn!("解析commit命令失败，将创建默认commit命令");
                // 即使解析失败，也应该处理为commit命令
                let default_commit_args = cli::CommitArgs {
                    ai: !args.contains(&"--noai".to_string()),
                    noai: args.contains(&"--noai".to_string()),
                    tree_sitter: None,
                    auto_stage: args.contains(&"-a".to_string()) || args.contains(&"--all".to_string()),
                    message: None,
                    passthrough_args: args.iter().filter(|a| *a != "commit").cloned().collect(),
                };
                return handle_commit(default_commit_args, &config).await;
            }
        }
    }
    
    // 过滤掉tree-sitter相关标志
    let filtered_args = filter_tree_sitter_args(args);
    
    // 再次检查过滤后的参数是否包含commit
    if !filtered_args.is_empty() && filtered_args[0] == "commit" {
        tracing::warn!("过滤参数后仍检测到commit命令，使用默认处理");
        let default_commit_args = cli::CommitArgs {
            ai: !filtered_args.contains(&"--noai".to_string()),
            noai: filtered_args.contains(&"--noai".to_string()),
            tree_sitter: None,
            auto_stage: filtered_args.contains(&"-a".to_string()) || filtered_args.contains(&"--all".to_string()),
            message: None,
            passthrough_args: filtered_args.iter().cloned().collect(),
        };
        return handle_commit(default_commit_args, &config).await;
    }
    
    // Execute Git command and capture output
    let cmd_str_log = filtered_args.join(" ");
    tracing::debug!("执行git命令: git {}", cmd_str_log);
    let output = passthrough_to_git_with_error_handling(&filtered_args, use_ai)?;

    // If AI is enabled and command execution failed, provide AI error explanation
    if use_ai && !output.status.success() {
        tracing::info!("Git 命令执行失败，提供 AI 错误解释");

        // Merge stderr and stdout for analysis
        let mut error_text = String::new();
        if !output.stderr.is_empty() {
            error_text.push_str(&output.stderr);
        }
        if !output.stdout.is_empty() {
            if !error_text.is_empty() {
                error_text.push_str("\n");
            }
            error_text.push_str(&output.stdout);
        }

        // Call error explanation function
        match explain_git_error(config, &error_text, &format!("git {}", cmd_str_log)).await {
            Ok(explanation) => println!("{}", explanation),
            Err(e) => {
                tracing::error!("无法生成 AI 错误解释: {}", e);
                // Original error already output, no need to repeat it here
            }
        }

        // Return appropriate error code
        return Err(AppError::Git(GitError::CommandFailed {
            command: format!("git {}", cmd_str_log),
            status_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        }));
    }

    if !output.status.success() {
        // If not using AI and command failed, return standard error
        return Err(AppError::Git(GitError::CommandFailed {
            command: format!("git {}", cmd_str_log),
            status_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        }));
    }

    Ok(())
}

async fn run_app() -> Result<(), AppError> {
    let config = AppConfig::load()?;

    // First check if git is available
    if !is_git_available()? {
        tracing::error!("错误: Git 在此系统上不可用。");
        return Err(AppError::IO(
            "Git command not found or not executable".to_string(),
            std::io::Error::new(std::io::ErrorKind::NotFound, "Git not available"),
        ));
    }

    // Then check if we're in a git repository
    if !is_in_git_repository()? {
        tracing::error!("错误: 不是一个 git 仓库（或任何父目录）。");
        return Err(GitError::NotARepository.into());
    }

    let raw_cli_args: Vec<String> = std::env::args().skip(1).collect();

    // 1. Check for help flags first
    if args_contain_help(&raw_cli_args) {
        if should_use_ai(&raw_cli_args) {
            tracing::info!("检测到帮助标志，已启用 AI。解释 Git 命令输出...");
            let mut command_to_execute_for_help = raw_cli_args.clone();
            command_to_execute_for_help.retain(|arg| arg != "--ai" && arg != "--noai");
            tracing::debug!(
                "即将执行的命令是: {}",
                &command_to_execute_for_help.join(" ")
            );

            // After removing the --ai flag:
            // - For `gitie --ai --help` -> `--help` remains in the command
            // - For `gitie --ai commit --help` -> `commit --help` remains
            // The help flags (-h/--help) are NOT removed by the retain operation,
            // only the --ai flag is removed
            // Since help flags always remain, we'll never have an empty command
            let cmd_output = execute_git_command_and_capture_output(&command_to_execute_for_help)?;
            let mut text_to_explain = cmd_output.stdout;
            if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
                text_to_explain.push_str("\n--- Stderr ---\n");
                text_to_explain.push_str(&cmd_output.stderr);
            }
            match explain_git_command_output(&config, &text_to_explain).await {
                Ok(explanation) => println!("{}", explanation),
                Err(e) => return Err(AppError::AI(e)),
            }
        } else {
            // --noai is present, just passthrough the help request to git
            tracing::info!("检测到帮助标志和 --noai。传递给 git。");
            let mut filtered_args = raw_cli_args.clone();
            filtered_args.retain(|arg| arg != "--noai");
            
            // 过滤掉tree-sitter标志，防止直接传递给git
            filtered_args = filter_tree_sitter_args(&filtered_args);
            
            passthrough_to_git(&filtered_args)?;
        }
    } else {
        // 2. Not a help request, try parsing as git-enhancer subcommand or global AI explanation
        let mut gitie_parser_args = vec!["gitie-dummy".to_string()]; // Dummy executable name for clap
        gitie_parser_args.extend_from_slice(&raw_cli_args);

        // 首先检查第一个非选项参数是否是commit命令
        let first_non_option_arg = raw_cli_args.iter()
            .filter(|arg| !arg.starts_with('-'))
            .next();
            
        if let Some(arg) = first_non_option_arg {
            if arg == "commit" {
                tracing::info!("检测到commit命令: {:?}", raw_cli_args);
                // 直接构建GitieSubCommand::Commit
                tracing::info!("检测到直接的commit命令，构建commit子命令。");
                let mut commit_args_vec = vec!["gitie".to_string(), "commit".to_string()];
                for arg in &raw_cli_args[1..] {
                    commit_args_vec.push(arg.clone());
                }
                    
                if let Ok(parsed_args) = GitieArgs::try_parse_from(&commit_args_vec) {
                    match parsed_args.command {
                        GitieSubCommand::Commit(commit_args) => {
                            tracing::info!("已解析为 gitie commit 子命令。委托给 handle_commit");
                            return handle_commit(commit_args, &config).await;
                        }
                    }
                }
            }
        }
        
        // 检查组合参数中是否包含commit
        let contains_commit = raw_cli_args.iter().any(|arg| arg == "commit");
        if contains_commit {
            tracing::info!("在参数中检测到commit: {:?}", raw_cli_args);
            // 重新排列参数，将commit放在第一位
            let mut rearranged_args = vec!["gitie".to_string(), "commit".to_string()];
        
            // 添加所有不是"commit"的参数
            for arg in raw_cli_args.iter() {
                if arg != "commit" {
                    rearranged_args.push(arg.clone());
                }
            }
        
            tracing::debug!("重新排列参数: {:?}", rearranged_args);
        
            if let Ok(parsed_args) = GitieArgs::try_parse_from(&rearranged_args) {
                match parsed_args.command {
                    GitieSubCommand::Commit(commit_args) => {
                        tracing::info!("成功重新解析为commit命令");
                        return handle_commit(commit_args, &config).await;
                    }
                }
            } else {
                tracing::warn!("解析commit参数失败，使用默认commit参数");
                // 即使解析失败，也应该处理为commit命令
                let default_commit_args = cli::CommitArgs {
                    ai: !raw_cli_args.contains(&"--noai".to_string()),
                    noai: raw_cli_args.contains(&"--noai".to_string()),
                    tree_sitter: None,
                    auto_stage: raw_cli_args.contains(&"-a".to_string()) || raw_cli_args.contains(&"--all".to_string()),
                    message: None,
                    passthrough_args: raw_cli_args.iter().filter(|a| *a != "commit").cloned().collect(),
                };
                return handle_commit(default_commit_args, &config).await;
            }
        }
            
        // 尝试常规解析
        match GitieArgs::try_parse_from(&gitie_parser_args) {
            Ok(parsed_args) => {
                // Successfully parsed as a gitie specific command
                match parsed_args.command {
                    GitieSubCommand::Commit(commit_args) => {
                        // This handles `gitie commit --ai` as well as `gitie commit -m "message"`
                        // The `handle_commit` function itself checks `commit_args.ai`
                        tracing::info!("已解析为 gitie commit 子命令。委托给 handle_commit");
                        handle_commit(commit_args, &config).await?;
                    } // Future: Add other SubCommand arms here if they are added to cli.rs
                }
            }
            Err(_) => {
                // Failed to parse as a specific gitie subcommand.
                // This could be a generic command that should receive AI explanation by default,
                // or a command to passthrough if --noai is specified
                if should_use_ai(&raw_cli_args) {
                    tracing::info!("不是特定的 gitie 子命令，提供 AI 解释（默认行为）...");

                    let mut command_to_explain = raw_cli_args.clone();
                    command_to_explain.retain(|arg| arg != "--ai"); // Remove all occurrences of --ai for backward compatibility
                    
                    // 检查是否任何位置包含 "commit" 参数，这可能表示用户想要使用commit命令
                    // 但忽略以'-'开头的参数，因为这些可能是选项而不是子命令
                    let commit_index = command_to_explain.iter()
                        .position(|arg| arg == "commit" && !arg.starts_with('-'));
                    
                    tracing::debug!("命令解析: {:?}, commit_index: {:?}", command_to_explain, commit_index);
                    
                    // 如果commit_index存在，重新排列参数并尝试解析
                    if let Some(index) = commit_index {
                        // 创建一个包含 "gitie commit" 和所有其它参数的新命令行，但移动commit到前面
                        let mut gitie_args = vec!["gitie".to_string(), "commit".to_string()];
                        
                        // 添加commit之前的参数（可能包含选项）
                        for arg in &command_to_explain[..index] {
                            gitie_args.push(arg.clone());
                        }
                        
                        // 添加commit之后的参数（跳过commit本身）
                        if index + 1 < command_to_explain.len() {
                            for arg in &command_to_explain[index+1..] {
                                gitie_args.push(arg.clone());
                            }
                        }
                        
                        tracing::info!("尝试重新解析为commit命令: {:?}", gitie_args);
                        
                        if let Ok(parsed_args) = GitieArgs::try_parse_from(&gitie_args) {
                            match parsed_args.command {
                                GitieSubCommand::Commit(commit_args) => {
                                    tracing::info!("成功重新解析为 gitie commit 子命令。委托给 handle_commit");
                                    return handle_commit(commit_args, &config).await;
                                }
                            }
                        } else {
                            tracing::warn!("解析commit命令失败，使用默认commit参数");
                            // 即使解析失败，也应该处理为commit命令 
                            let default_commit_args = cli::CommitArgs {
                                ai: !command_to_explain.contains(&"--noai".to_string()),
                                noai: command_to_explain.contains(&"--noai".to_string()),
                                tree_sitter: None,
                                auto_stage: command_to_explain.contains(&"-a".to_string()) || command_to_explain.contains(&"--all".to_string()),
                                message: None,
                                passthrough_args: command_to_explain.iter().filter(|a| *a != "commit").cloned().collect(),
                            };
                            return handle_commit(default_commit_args, &config).await;
                        }
                    }

                    if command_to_explain.is_empty() {
                        // Handle `gitie` with no actual command
                        // Default to explaining "git --help"
                        tracing::debug!("没有指定命令，默认解释 'git --help'。");
                        command_to_explain.push("--help".to_string());
                        match explain_git_command(&config, &command_to_explain).await {
                            Ok(explanation) => println!("{}", explanation),
                            Err(e) => return Err(AppError::AI(e)),
                        }
                    } else if command_to_explain[0] == "--help" {
                        // 对于帮助命令，使用原有解释逻辑
                        match explain_git_command(&config, &command_to_explain).await {
                            Ok(explanation) => println!("{}", explanation),
                            Err(e) => return Err(AppError::AI(e)),
                        }
                    } else {
                        // 对于非帮助命令，尝试执行并处理可能的错误
                        match execute_git_command_with_error_handling(
                            &config,
                            &command_to_explain,
                            true,
                        )
                        .await
                        {
                            Ok(_) => {
                                // 命令成功执行，不需要额外解释
                            }
                            Err(e) => {
                                // 错误已经在函数内处理，直接返回错误
                                return Err(e);
                            }
                        }
                    }
                } else {
                    // --noai flag is present, pass through to git after removing the flag
                    tracing::info!("检测到 --noai 标志。不使用 AI 功能传递给 git。");
                    let mut filtered_args = raw_cli_args.clone();
                    filtered_args.retain(|arg| arg != "--noai");
                    
                    // 过滤掉tree-sitter标志，防止直接传递给git
                    filtered_args = filter_tree_sitter_args(&filtered_args);
                    
                    execute_git_command_with_error_handling(&config, &filtered_args, false).await?;
                }
            }
        }
    }

    Ok(())
}
