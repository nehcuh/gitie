mod ai_module;
mod cli_interface;
mod command_processing;
mod config_management;
mod core;
mod git_module;
mod review_engine;
mod tree_sitter_analyzer;

use crate::ai_module::explainer::{explain_git_command_output, explain_git_error};
use crate::cli_interface::args::{
    CommitArgs, GitieArgs, GitieSubCommand, ReviewArgs, args_contain_help, generate_gitie_help,
    should_use_ai,
};
use crate::command_processing::commit::handle_commit;
use crate::command_processing::review::{handle_commit_with_review, handle_review};
use crate::config_management::settings::AppConfig;
use crate::core::errors::{AppError, GitError};
use crate::git_module::{
    execute_git_command_and_capture_output, is_git_available, is_in_git_repository,
    passthrough_to_git, passthrough_to_git_with_error_handling,
};
use clap::Parser;
use std::env;

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
            tracing::debug!("跳过tree-sitter标志: {}", arg);
            continue;
        }

        // 保留所有其他参数
        filtered.push(arg.clone());
    }

    filtered
}

/// 使用错误处理执行git命令
async fn execute_git_command_with_error_handling(
    config: &AppConfig,
    args: &[String],
    original_args: &[String],
) -> Result<(), AppError> {
    // 检查git是否可用
    if let Ok(available) = is_git_available() {
        if !available {
            return Err(AppError::Git(GitError::CommandFailed {
                command: "git --version".to_string(),
                status_code: None,
                stdout: String::new(),
                stderr: "Git command not found".to_string(),
            }));
        }
    } else {
        return Err(AppError::Generic(
            "Failed to check if git is available".to_string(),
        ));
    }

    // 检查当前目录是否是git仓库
    if let Ok(is_repo) = is_in_git_repository() {
        if !is_repo {
            return Err(AppError::Git(GitError::NotARepository));
        }
    } else {
        return Err(AppError::Generic(
            "Failed to check if directory is a git repository".to_string(),
        ));
    }

    // 尝试执行git命令
    let result = execute_git_command_and_capture_output(args);

    // 处理结果
    match result {
        Ok(output) => {
            // 处理成功的输出
            if !output.stdout.trim().is_empty() {
                println!("{}", output.stdout);
            }
            if !output.stderr.trim().is_empty() {
                eprintln!("{}", output.stderr);
            }
            Ok(())
        }
        Err(app_error) => {
            // 从AppError提取GitError
            if let AppError::Git(git_error) = &app_error {
                // 如果需要解释错误，提供AI解释
                if should_use_ai(original_args) {
                    tracing::info!("使用AI解释错误");
                    if let GitError::CommandFailed {
                        command, stderr, ..
                    } = git_error
                    {
                        // 尝试获取AI错误解释
                        let explanation_result = explain_git_error(config, stderr, command).await;
                        if let Ok(explanation) = explanation_result {
                            eprintln!("{}", stderr);
                            eprintln!("\n错误解释:\n{}", explanation);
                        } else if let Err(ai_err) = explanation_result {
                            tracing::warn!("无法获取AI错误解释: {}", ai_err);
                        }
                    }
                }
            }
            // 返回原始错误
            Err(app_error)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    // 加载配置
    let config = match AppConfig::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("错误：配置加载失败: {}", e);
            return Err(AppError::Config(e));
        }
    };

    // 获取命令行参数
    let args: Vec<String> = env::args().collect();

    // 如果没有参数，直接执行git，显示git帮助
    if args.len() <= 1 || args.iter().any(|arg| arg == "--noai") {
        passthrough_to_git(&[])?;
        return Ok(());
    }

    // 过滤掉tree-sitter相关的参数，确保干净的git命令
    let filtered_args = filter_tree_sitter_args(&args[1..]);

    // 检查是否为 review 命令
    if filtered_args.contains(&"review".to_string())
        && filtered_args.iter().all(|a| a != "--help" && a != "-h")
    {
        tracing::info!("检测到review命令");

        // 重构review命令参数以便使用clap解析
        let mut review_args_vec = vec!["gitie".to_string(), "review".to_string()];

        // 获取review之后的所有其他参数
        let review_index = filtered_args
            .iter()
            .position(|a| a == "review")
            .unwrap_or(0);
        if review_index + 1 < filtered_args.len() {
            review_args_vec.extend_from_slice(&filtered_args[review_index + 1..]);
        }

        tracing::debug!("重构的review命令: {:?}", review_args_vec);

        if let Ok(parsed_args) = GitieArgs::try_parse_from(&review_args_vec) {
            match parsed_args.command {
                GitieSubCommand::Review(review_args) => {
                    return handle_review(review_args, &config).await;
                }
                _ => {}
            }
        } else {
            tracing::warn!("解析review命令失败");
            // 创建默认的ReviewArgs
            let default_review_args = ReviewArgs {
                depth: "normal".to_string(),
                focus: None,
                lang: None,
                format: "text".to_string(),
                output: None,
                tree_sitter: false,
                no_tree_sitter: false,
                review_ts: false,
                passthrough_args: vec![],
                commit1: None,
                commit2: None,
            };
            return handle_review(default_review_args, &config).await;
        }
    }

    // 如果是commit命令，使用AI辅助生成提交信息
    if filtered_args.contains(&"commit".to_string())
        && filtered_args.iter().all(|a| a != "--help" && a != "-h")
    {
        tracing::info!("检测到commit命令");

        // 重构commit命令参数以便使用clap解析
        let mut commit_args_vec = vec!["gitie".to_string(), "commit".to_string()];

        // 获取commit之后的所有其他参数
        let commit_index = filtered_args
            .iter()
            .position(|a| a == "commit")
            .unwrap_or(0);
        if commit_index + 1 < filtered_args.len() {
            commit_args_vec.extend_from_slice(&filtered_args[commit_index + 1..]);
        }

        tracing::debug!("重构的commit命令: {:?}", commit_args_vec);

        if let Ok(parsed_args) = GitieArgs::try_parse_from(&commit_args_vec) {
            match parsed_args.command {
                GitieSubCommand::Commit(commit_args) => {
                    // 检查是否需要进行提交前代码评审
                    if commit_args.review {
                        if let Ok(should_cancel) =
                            handle_commit_with_review(&commit_args, &config).await
                        {
                            if should_cancel {
                                return Ok(());
                            }
                        }
                    }
                    return handle_commit(commit_args, &config).await;
                }
                _ => {}
            }
        } else {
            tracing::info!("无法解析标准参数格式，将使用默认commit命令");
            // 获取commit之后的所有其他参数作为passthrough
            let mut passthrough_args = Vec::new();
            let commit_index = filtered_args
                .iter()
                .position(|a| a == "commit")
                .unwrap_or(0);
            if commit_index + 1 < filtered_args.len() {
                passthrough_args = filtered_args[commit_index + 1..].to_vec();
            }

            let default_commit_args = CommitArgs {
                ai: !filtered_args.contains(&"--noai".to_string()),
                noai: filtered_args.contains(&"--noai".to_string()),
                tree_sitter: if filtered_args.contains(&"--tree-sitter".to_string())
                    || filtered_args.contains(&"-t".to_string())
                {
                    Some("medium".to_string())
                } else {
                    None
                },
                auto_stage: filtered_args.contains(&"--all".to_string())
                    || filtered_args.contains(&"-a".to_string()),
                message: None,
                review: filtered_args.contains(&"--review".to_string()),
                passthrough_args,
            };

            // 检查是否需要进行提交前代码评审
            if default_commit_args.review {
                if let Ok(should_cancel) =
                    handle_commit_with_review(&default_commit_args, &config).await
                {
                    if should_cancel {
                        return Ok(());
                    }
                }
            }

            return handle_commit(default_commit_args, &config).await;
        }
    }

    // 检查是否包含help标志和AI标志
    let need_help = args_contain_help(&filtered_args);
    let use_ai = should_use_ai(&filtered_args);

    if need_help {
        tracing::info!("检测到help标志");

        // 获取gitie自定义帮助
        let gitie_help = generate_gitie_help();

        // 获取完整的git帮助文本
        let git_help = match execute_git_command_and_capture_output(&["--help".to_string()]) {
            Ok(output) => output.stdout,
            Err(e) => {
                eprintln!("获取git帮助信息失败: {}", e);
                return Err(e);
            }
        };

        // 合并帮助信息
        let combined_help = format!("{}\n\n== 标准 Git 帮助 ==\n\n{}", gitie_help, git_help);

        if use_ai {
            // 使用AI解释帮助内容
            match ai_module::explainer::explain_git_command_output(&config, &combined_help).await {
                Ok(explanation) => {
                    // 输出AI解释和原始帮助
                    println!("{}", explanation);
                }
                Err(e) => {
                    tracing::warn!("无法获取AI帮助解释: {}", e);
                    // 如果AI解释失败，仍然显示原始帮助
                    println!("{}", combined_help);
                }
            }
        } else {
            // 不使用AI，直接显示组合帮助
            println!("{}", combined_help);
        }

        return Ok(());
    }

    // 对于所有其他情况，直接传递给git
    // 使用包含错误处理的函数来处理
    if use_ai {
        execute_git_command_with_error_handling(&config, &filtered_args, &args[1..]).await?;
    } else {
        // 对于禁用AI的情况，直接传递给git，不包含额外处理
        passthrough_to_git_with_error_handling(&filtered_args, false)?;
    }

    Ok(())
}
