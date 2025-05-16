use crate::{
    ai_utils::{ChatMessage, OpenAIChatCompletionResponse, OpenAIChatRequest, clean_ai_output},
    cli::CommitArgs,
    config::AppConfig,
    errors::{AIError, AppError, GitError},
    git_commands::map_output_to_git_command_error,
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
        "Commit passthrough {}: msg: {:?}, args: {:?}",
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
        tracing::error!("Passthrough git {} failed with status {}", cmd_desc, status);
        return Err(AppError::Git(GitError::PassthroughFailed {
            command: format!("git {}", cmd_desc),
            status_code: status.code(),
        }));
    }
    tracing::info!(
        "Passthrough git {} initiated/completed successfully.",
        cmd_desc
    );
    Ok(())
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
        tracing::info!("AI commit: Attempting to generate message (default behavior)...");
        // Handle auto-staging functionality
        if args.auto_stage {
            tracing::info!("Auto-staging tracked changes due to -a/--all flag");
            let add_result = StdCommand::new("git")
                .arg("add")
                .arg("-u")
                .output()
                .map_err(|e| AppError::IO("Failed to auto-stage changes".to_string(), e))?;

            if !add_result.status.success() {
                tracing::error!("Failed to auto-stage changes with git add -u");
                return Err(map_output_to_git_command_error("git add -u", add_result).into());
            }
        }

        let diff_out = StdCommand::new("git")
            .arg("diff")
            .arg("--staged")
            .output()
            .map_err(|e| AppError::Git(GitError::DiffError(e)))?;
        if !diff_out.status.success() {
            tracing::error!("Error getting git diff. Is anything staged for commit?");
            return Err(map_output_to_git_command_error("git diff --staged", diff_out).into());
        }
        let diff = String::from_utf8_lossy(&diff_out.stdout);
        if diff.trim().is_empty() {
            tracing::info!("AI commit: No staged changes. Checking for --allow-empty.");
            if args.passthrough_args.contains(&"--allow-empty".to_string()) {
                let passthrough_commit_args = CommitArgs {
                    ai: false,
                    noai: true,
                    auto_stage: args.auto_stage,
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
        let user_prompt = format!("Git diff:\n{}\nGenerate commit message.", diff.trim());
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: config.prompts.get("commit").cloned().unwrap_or_else(|| {
                    tracing::warn!("Commit prompt not found in config, using empty string");
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
            tracing::error!("AI API request failed with status {}: {}", code, body);
            return Err(AppError::AI(AIError::ApiResponseError(code, body)));
        }

        let resp_data = ai_resp
            .json::<OpenAIChatCompletionResponse>()
            .await
            .map_err(AIError::ResponseParseFailed)?;
        let ai_msg = resp_data.choices.get(0).map_or("", |c| &c.message.content);
        let final_msg = clean_ai_output(ai_msg).trim().to_string();

        if final_msg.is_empty() {
            tracing::error!("AI returned an empty message.");
            return Err(AppError::AI(AIError::EmptyMessage));
        }
        tracing::info!("AI Message:\n---\n{}\n---", final_msg);

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
            tracing::error!("Git commit command with AI message failed.");
            return Err(map_output_to_git_command_error("git commit -m <AI>", commit_out).into());
        }
        tracing::info!("Successfully committed with AI message.");
    } else {
        return handle_commit_passthrough(args, "(standard commit with --noai)".to_string()).await;
    }
    Ok(())
}
