use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Helper struct to manage a temporary git repository for testing
struct GitTestRepo {
    root_dir: TempDir,
}

impl GitTestRepo {
    /// Create a new temporary git repository
    fn new() -> Self {
        let root_dir = TempDir::new().expect("Failed to create temp directory");
        
        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&root_dir)
            .output()
            .expect("Failed to initialize git repository");
        
        // Configure git user
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&root_dir)
            .output()
            .expect("Failed to configure git user name");
        
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&root_dir)
            .output()
            .expect("Failed to configure git user email");
        
        Self { root_dir }
    }
    
    /// Get the path to the root directory
    fn path(&self) -> &Path {
        self.root_dir.path()
    }
    
    /// Create a file with given content in the repository
    fn create_file(&self, filename: &str, content: &str) {
        let path = self.path().join(filename);
        let mut file = File::create(path).expect("Failed to create file");
        write!(file, "{}", content).expect("Failed to write to file");
    }
    
    /// Stage all changes
    fn stage_all(&self) -> Output {
        Command::new("git")
            .args(["add", "."])
            .current_dir(self.path())
            .output()
            .expect("Failed to stage changes")
    }
    
    /// Run gitie command and return the output
    fn run_gitie(&self, args: &[&str]) -> Output {
        let gitie_path = env!("CARGO_BIN_EXE_gitie");
        Command::new(gitie_path)
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("Failed to execute gitie command")
    }
    
    /// Get the last commit message
    fn get_last_commit_message(&self) -> String {
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=%B"])
            .current_dir(self.path())
            .output()
            .expect("Failed to get last commit message");
        
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

/// Test that `gitie commit` now uses AI by default
#[test]
fn test_default_commit_uses_ai() {
    // Set up test environment
    let repo = GitTestRepo::new();
    
    // Create a sample file
    repo.create_file("test.txt", "This is a test file\n");
    repo.stage_all();
    
    // Run gitie commit (without --ai flag)
    let output = repo.run_gitie(&["commit"]);
    
    // Check if the commit was created successfully
    assert!(output.status.success(), "Commit command failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // Get the commit message
    let commit_msg = repo.get_last_commit_message();
    
    // The message should not be empty (AI generated)
    assert!(!commit_msg.is_empty(), "Commit message should not be empty");
    
    // The message should not contain the default editor template text
    assert!(!commit_msg.contains("Please enter the commit message"), 
            "Commit message should be AI-generated, not from editor template");
}

/// Test that `gitie commit --noai` disables AI
#[test]
fn test_noai_flag_disables_ai() {
    // Set up test environment
    let repo = GitTestRepo::new();
    
    // Create a sample file
    repo.create_file("test.txt", "This is a test file\n");
    repo.stage_all();
    
    // Run gitie commit with --noai and a manual message
    let output = repo.run_gitie(&["commit", "--noai", "-m", "Manual commit message"]);
    
    // Check if the commit was created successfully
    assert!(output.status.success(), "Commit command failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // Get the commit message
    let commit_msg = repo.get_last_commit_message();
    
    // The message should match our manual message
    assert_eq!(commit_msg, "Manual commit message", 
               "Commit message should match the manually provided message");
}

/// Test that `gitie commit --ai` still works for backward compatibility
#[test]
fn test_ai_flag_backward_compatibility() {
    // Set up test environment
    let repo = GitTestRepo::new();
    
    // Create a sample file
    repo.create_file("test.txt", "This is a test file\n");
    repo.stage_all();
    
    // Run gitie commit with explicit --ai flag
    let output = repo.run_gitie(&["commit", "--ai"]);
    
    // Check if the commit was created successfully
    assert!(output.status.success(), "Commit command failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // Get the commit message
    let commit_msg = repo.get_last_commit_message();
    
    // The message should not be empty (AI generated)
    assert!(!commit_msg.is_empty(), "Commit message should not be empty");
    
    // The message should not contain the default editor template text
    assert!(!commit_msg.contains("Please enter the commit message"), 
            "Commit message should be AI-generated, not from editor template");
}

/// Test the precedence when both --ai and --noai flags are provided
#[test]
fn test_flag_precedence() {
    // Set up test environment
    let repo = GitTestRepo::new();
    
    // Create a sample file
    repo.create_file("test.txt", "This is a test file\n");
    repo.stage_all();
    
    // Run gitie commit with both flags and a manual message
    // --noai should take precedence
    let output = repo.run_gitie(&["commit", "--ai", "--noai", "-m", "Manual precedence test"]);
    
    // Check if the commit was created successfully
    assert!(output.status.success(), "Commit command failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // Get the commit message
    let commit_msg = repo.get_last_commit_message();
    
    // The message should match our manual message (--noai takes precedence)
    assert_eq!(commit_msg, "Manual precedence test", 
               "--noai should take precedence over --ai");
}

/// Test that help commands use AI by default
#[test]
fn test_help_uses_ai_by_default() {
    // Set up test environment
    let repo = GitTestRepo::new();
    
    // Run gitie commit --help
    let output = repo.run_gitie(&["commit", "--help"]);
    
    // Check if the command was successful
    assert!(output.status.success(), "Help command failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // The output should contain AI explanation markers
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Look for some indication of AI explanation (this might need adjustment based on actual output)
    assert!(stdout.contains("AI Explanation") || 
            stdout.contains("Here's an explanation") ||
            stdout.contains("In simple terms"),
            "Help output should include AI explanations");
}

/// Test that help commands with --noai disable AI
#[test]
fn test_help_with_noai_disables_ai() {
    // Set up test environment
    let repo = GitTestRepo::new();
    
    // Run gitie commit --help with --noai
    let output = repo.run_gitie(&["commit", "--help", "--noai"]);
    
    // Check if the command was successful
    assert!(output.status.success(), "Help command failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // The output should be standard git help without AI explanation
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Print the actual output for debugging
    println!("HELP OUTPUT WITH --NOAI:\n{}", stdout);
    
    // Git help can be displayed in different formats (man page or usage)
    // For man page format, it typically contains "GIT-COMMIT(1)" header
    // For usage format, it typically starts with "usage: git commit"
    assert!(stdout.contains("GIT-COMMIT(1)") || 
            stdout.contains("usage: git commit"), 
            "Help output should be standard git help without AI explanations");
    
    // The exact format of Git help output can vary, but it typically 
    // contains git command syntax and doesn't have AI-specific language
    // Just check that it contains standard git help elements
    assert!(stdout.contains("Record changes to the repository") || 
            stdout.contains("OPTIONS") || 
            stdout.contains("DESCRIPTION"),
            "Help output should contain standard git help elements");
}