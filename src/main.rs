mod ai_explainer;
mod ai_utils;
mod cli;
mod commit_commands;
mod config;
mod errors;
mod git_commands;
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
    // Execute Git command and capture output
    let cmd_str_log = args.join(" ");
    let output = passthrough_to_git_with_error_handling(args, use_ai)?;

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
            passthrough_to_git(&filtered_args)?;
        }
    } else {
        // 2. Not a help request, try parsing as git-enhancer subcommand or global AI explanation
        let mut gitie_parser_args = vec!["gitie-dummy".to_string()]; // Dummy executable name for clap
        gitie_parser_args.extend_from_slice(&raw_cli_args);

        match GitieArgs::try_parse_from(&gitie_parser_args) {
            Ok(parsed_args) => {
                // Successfully parsed as a gitie specific command
                match parsed_args.command {
                    GitieSubCommand::Commit(commit_args) => {
                        // This handles `gitie commit --ai` as well as `gitie commit -m "message"`
                        // The `handle_commit` function itself checks `commit_args.ai`:w
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
                    execute_git_command_with_error_handling(&config, &filtered_args, false).await?;
                }
            }
        }
    }

    Ok(())
}
