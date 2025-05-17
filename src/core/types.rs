use colored::*;
use std::collections::HashMap;
use std::process::ExitStatus;

/// Represents the output of a command execution
///
/// This structure captures the stdout, stderr, and exit status
/// of a command that has been executed.
#[derive(Debug)]
pub struct CommandOutput {
    /// Standard output from the command
    pub stdout: String,

    /// Standard error output from the command
    pub stderr: String,

    /// Exit status of the command
    pub status: ExitStatus,
}

impl CommandOutput {
    /// Returns true if the command executed successfully
    pub fn is_success(&self) -> bool {
        self.status.success()
    }

    /// Returns the exit code of the command, if available
    #[allow(unused)]
    pub fn exit_code(&self) -> Option<i32> {
        self.status.code()
    }

    /// Returns the combined output (stdout + stderr) with stderr
    #[allow(unused)]
    pub fn combined_output(&self) -> String {
        let mut output = self.stdout.clone();

        if !self.stderr.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(
                &self
                    .stderr
                    .lines()
                    .map(|line| format!("{}: {}", "ERROR\n".red(), line))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }

        output
    }

    /// Return true if both stdout and stderr are empty
    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.stdout.is_empty() && self.stderr.is_empty()
    }

    /// Returns stdout as a vector of lines
    #[allow(unused)]
    pub fn stdout_lines(&self) -> Vec<String> {
        self.stdout.lines().map(String::from).collect()
    }

    /// Returns a formatted display of the command output for user interaction
    #[allow(unused)]
    pub fn formatted_display(&self) -> String {
        let mut display = String::new();

        if !self.stdout.is_empty() {
            display.push_str("Output:\n");
            display.push_str(&self.stdout);
            display.push('\n');
        }

        if !self.stderr.is_empty() {
            display.push_str("Errors:\n");
            display.push_str(&self.stderr);
            display.push('\n');
        }

        if !self.is_success() {
            display.push_str(&format!("Exit code: {}\n", self.exit_code().unwrap_or(-1)));
        }

        display
    }
}

/// Represents a Git commit
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct GitCommit {
    /// The commit hash
    pub hash: String,

    //// The commit message
    pub message: String,

    /// The commit author name
    pub author: String,

    /// The commit author email
    pub email: String,

    /// The commit date (ISO 8601 format YYYY-MM-DD HH:MM:SS)
    pub date: String,
}

/// Represents the states of files in a Git repository
#[allow(unused)]
#[derive(Debug, Default)]
pub struct GitStatus {
    /// Files that are staged for commit
    pub staged: Vec<GitFileStatus>,

    /// Files that are modified but not staged
    pub modified: Vec<GitFileStatus>,

    /// Files that are untracked
    pub untracked: Vec<String>,

    /// Current branch name
    pub current_branch: Option<String>,
}

/// Represents the status of a specific file in Git
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct GitFileStatus {
    /// Path to the file
    pub path: String,

    /// Status code (A: added, M: Modified, D: deleted, etc.)
    pub status_code: String,
}

/// Represents different Git operations
#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
pub enum GitOperation {
    /// Git commit operation
    Commit,

    /// Git push operation
    Push,

    /// Git pull operation
    Pull,

    /// Git fetch operation
    Get,

    /// Git merge operation
    Merge,

    /// Git rebase operation
    Rebase,

    /// Git checkout operation
    Checkout,

    /// Git branch operation
    Branch,

    /// Other Git operations
    Other(String),
}

/// Represents an entry in the Git config
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct GitConfigEntry {
    /// The section of the config (e.g. "user", "core")
    pub section: String,

    /// The key name
    pub key: String,

    /// The value
    pub value: String,
}

/// Represents parsed Git configuration
#[allow(unused)]
#[derive(Debug, Default)]
pub struct GitConfig {
    /// Map of section.key to value
    pub entries: HashMap<String, String>,

    /// Structured access to common sections
    pub section: HashMap<String, HashMap<String, String>>,
}
