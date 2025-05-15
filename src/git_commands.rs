use std::process::{Command, Output as ProcessOutput};

use crate::{
    errors::{AppError, GitError},
    types::CommandOutput,
};

/// Execute a git command and captures its output
///
/// This function runs a git command with the provided argument and return
/// the command's output, include stdout, stderr, and exit status.
///
/// # Arguments
///
/// * `args` - A slice of String containing the arguments to pass to git
///
/// # Returns
///
/// * `Result<CommandOutput, AppError>` - The command output or an error
///
/// # Example
///
/// ```
/// use crate::git_commands::execute_git_command_and_capture_output;
///
/// let args = vec!["status".to_string(), "--short".to_string()];
/// match execute_git_command_and_capture_outputs(&args) {
///     Ok(output) => println!("Git status: {}", output.stdout),
///     Err(err) => eprintln!("Error: {}", err)
/// }
pub fn execute_git_command_and_capture_output(args: &[String]) -> Result<CommandOutput, AppError> {
    let cmd_to_run = if args.is_empty() {
        vec!["--help".to_string()]
    } else {
        args.to_vec()
    };

    tracing::debug!("Capturing output: git {}", cmd_to_run.join(" "));

    let output = Command::new("git")
        .args(&cmd_to_run)
        .output()
        .map_err(|e| {
            AppError::IO(
                format!("Failed to execute: git {}", cmd_to_run.join(" ")),
                e,
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        tracing::warn!(
            "Git cmd 'git {}' non-success {}. Stdout: [{}], Stderr: [{}]",
            cmd_to_run.join(" "),
            output.status,
            stdout,
            stderr
        );
    }

    Ok(CommandOutput {
        stdout,
        stderr,
        status: output.status,
    })
}

/// Checks if Git is installed and available
///
/// # Returns
///
/// * `Result<bool, AppError>` - True if git is available, or an error
pub fn is_git_available() -> Result<bool, AppError> {
    match Command::new("git").arg("--version").output() {
        Ok(output) => Ok(output.status.success()),
        Err(e) => Err(AppError::IO(
            "Failed to check if git is available".to_string(),
            e,
        )),
    }
}

/// Checks if the current directory is within a Git repository
///
/// # Returns
///
/// * `Result<bool, AppError>` - True if in a git repo, or an error
pub fn is_in_git_repository() -> Result<bool, AppError> {
    let result = execute_git_command_and_capture_output(&[
        "rev-parse".to_string(),
        "--is-inside-work-tree".to_string(),
    ]);

    match result {
        Ok(output) => Ok(output.is_success() && output.stdout.trim() == "true"),
        Err(e) => Err(e),
    }
}

/// Parses arguments directly to the system's git command
///
/// This function is used when the gitie needs to delegate to the
/// underlying git command without modification
///
/// # Arguments
///
/// * `args` - A slice of String containing the arguments to pass to git
///
/// # Returns
///
/// * `Result<(), AppError>` - Success or an error
pub fn passthrough_to_git(args: &[String]) -> Result<(), AppError> {
    let command_to_run = if args.is_empty() {
        vec!["--help".to_string()]
    } else {
        args.to_vec()
    };
    let cmd_str_log = command_to_run.join(" ");
    tracing::debug!("Passing to system git: git {}", cmd_str_log);

    let status = Command::new("git")
        .args(&command_to_run)
        .status()
        .map_err(|e| {
            AppError::IO(
                format!("Failed to execute system git: git {}", cmd_str_log),
                e,
            )
        })?;

    if !status.success() {
        tracing::warn!("Git passthrough 'git {}' failed: {}", cmd_str_log, status);
        return Err(AppError::Git(GitError::PassthroughFailed {
            command: format!("git: {}", cmd_str_log),
            status_code: status.code(),
        }));
    }

    Ok(())
}

/// Maps command output to a GitError
///
/// # Arguments
///
/// * `cmd_str` - The command string for error reporting
/// * `output` - The process output
///
/// # Returns
///
/// * `GitError` - The mapped error
pub fn map_output_to_git_command_error(cmd_str: &str, output: ProcessOutput) -> GitError {
    GitError::CommandFailed {
        command: cmd_str.to_string(),
        status_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}
