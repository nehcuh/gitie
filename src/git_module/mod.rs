use std::process::{Command, Output as ProcessOutput};

use crate::{
    core::{
        errors::{AppError, GitError},
        types::CommandOutput,
    },
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
/// use gitie::git_module::execute_git_command_and_capture_output;
///
/// let args = vec!["status".to_string(), "--short".to_string()];
/// match execute_git_command_and_capture_output(&args) {
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

/// Execute Git command and optionally handle errors
///
/// Executes Git command, captures output, and based on execution status decides 
/// whether to output the result directly or handle errors
///
/// # Arguments
///
/// * `args` - Arguments to pass to Git
/// * `handle_error` - Whether to handle errors (if false, behavior is the same as the original function)
///
/// # Returns
///
/// * `Result<CommandOutput, AppError>` - Command output or error
pub fn passthrough_to_git_with_error_handling(
    args: &[String],
    handle_error: bool,
) -> Result<CommandOutput, AppError> {
    let command_to_run = if args.is_empty() {
        vec!["--help".to_string()]
    } else {
        args.to_vec()
    };
    let cmd_str_log = command_to_run.join(" ");
    tracing::debug!("执行系统 git 命令: git {}", cmd_str_log);

    // 直接执行并获取输出，而不是只获取状态
    let output = Command::new("git")
        .args(&command_to_run)
        .output()
        .map_err(|e| {
            AppError::IO(
                format!("执行系统 git 命令失败: git {}", cmd_str_log),
                e,
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // 如果不需要错误处理或命令成功执行，则直接打印输出
    if !handle_error || output.status.success() {
        // 打印标准输出和错误输出，模拟原始命令的行为
        if !stdout.is_empty() {
            print!("{}", stdout);
        }
        if !stderr.is_empty() {
            eprint!("{}", stderr);
        }
    }

    if !output.status.success() {
        tracing::warn!("Git 命令 'git {}' 执行失败: {}", cmd_str_log, output.status);
        
        if !handle_error {
            // 如果不处理错误，则直接返回原始错误
            return Err(AppError::Git(GitError::PassthroughFailed {
                command: format!("git: {}", cmd_str_log),
                status_code: output.status.code(),
            }));
        }
    }

    Ok(CommandOutput {
        stdout,
        stderr,
        status: output.status,
    })
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
    // 调用新函数，指定不处理错误
    let result = passthrough_to_git_with_error_handling(args, false)?;
    
    // 检查状态以保持原始行为一致
    if !result.status.success() {
        return Err(AppError::Git(GitError::PassthroughFailed {
            command: format!("git: {}", args.join(" ")),
            status_code: result.status.code(),
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

#[cfg(test)]
mod tests {
    use super::*;
    
    // Test the Git command execution function with error handling (success case)
    #[test]
    fn test_passthrough_with_error_handling_success() {
        // Use git --version command, which typically executes successfully
        let args = vec!["--version".to_string()];
        
        // Test with error handling enabled
        let result = passthrough_to_git_with_error_handling(&args, true);
        assert!(result.is_ok(), "命令应该成功执行");
        
        if let Ok(output) = result {
            assert!(output.status.success(), "Git 命令应该成功");
            assert!(!output.stdout.is_empty(), "输出不应为空");
            assert!(output.stdout.contains("git version"), "输出应包含版本信息");
        }
        
        // Test with error handling disabled
        let result = passthrough_to_git_with_error_handling(&args, false);
        assert!(result.is_ok(), "Command should execute successfully");
        
        if let Ok(output) = result {
            assert!(output.status.success(), "Git 命令应该成功");
        }
    }
    
    // Test the Git command execution function with error handling (failure case)
    #[test]
    fn test_passthrough_with_error_handling_failure() {
        // Use a non-existent Git command
        let args = vec!["invalid-command".to_string()];
        
        // Test with error handling enabled
        let result = passthrough_to_git_with_error_handling(&args, true);
        
        // Command will fail, but function should successfully return error output
        assert!(result.is_ok(), "函数应该成功捕获错误");
        
        if let Ok(output) = result {
            assert!(!output.status.success(), "Git 命令应该失败");
            assert!(!output.stderr.is_empty(), "错误输出不应为空");
            assert!(
                output.stderr.contains("'invalid-command' is not a git command") || 
                output.stderr.contains("git: 'invalid-command' is not a git command"),
                "错误输出应该包含无效命令信息"
            );
        }
        
        // Test with error handling disabled
        let result = passthrough_to_git_with_error_handling(&args, false);
        assert!(result.is_err(), "函数应该返回错误");
        
        if let Err(err) = result {
            match err {
                AppError::Git(GitError::PassthroughFailed { command, status_code: _ }) => {
                    assert!(command.contains("invalid-command"), "错误信息应包含无效命令");
                },
                _ => panic!("错误类型不匹配"),
            }
        }
    }
    
    // Test empty arguments case
    #[test]
    fn test_passthrough_with_error_handling_empty_args() {
        // Use empty arguments
        let args: Vec<String> = vec![];
        
        // Test with error handling enabled
        let result = passthrough_to_git_with_error_handling(&args, true);
        assert!(result.is_ok(), "命令应该成功执行");
        
        if let Ok(output) = result {
            // Empty arguments should execute --help
            assert!(output.status.success(), "Git 命令应该成功");
            assert!(!output.stdout.is_empty(), "输出不应为空");
            assert!(output.stdout.contains("usage:") || output.stdout.contains("Usage:"), "输出应包含使用帮助");
        }
    }
}
