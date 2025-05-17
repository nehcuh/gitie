// Simple diff parser module for Tree-sitter analyzer
//
// This module provides a simplified implementation for parsing Git diff output,
// focusing on essential information extraction with minimal complexity.

use std::path::PathBuf;
use crate::tree_sitter_analyzer::core::{GitDiff, ChangedFile, DiffHunk, HunkRange, ChangeType};

/// Parse Git diff text into a simplified GitDiff structure
///
/// This function takes Git diff text and extracts basic information to create
/// a GitDiff structure, focusing on files and their paths rather than detailed hunks.
///
/// # Arguments
///
/// * `diff_text` - The Git diff text from git diff command
///
/// # Returns
///
/// * `GitDiff` - A simplified GitDiff structure with basic information
pub fn parse_simple_diff(diff_text: &str) -> GitDiff {
    let mut git_diff = GitDiff {
        changed_files: Vec::new(),
        metadata: None,
    };
    
    let mut current_file: Option<ChangedFile> = None;
    
    for line in diff_text.lines() {
        // Handle new file entries in the diff
        if line.starts_with("diff --git ") {
            // Save previous file if it exists
            if let Some(file) = current_file.take() {
                git_diff.changed_files.push(file);
            }
            
            // Create a new file entry with empty hunks
            current_file = Some(ChangedFile {
                path: PathBuf::new(),
                change_type: ChangeType::Modified, // Default to modified
                hunks: Vec::new(),
                file_mode_change: None,
            });
        }
        // Parse file paths
        else if line.starts_with("+++ b/") && line.len() > 6 {
            if let Some(ref mut file) = current_file {
                file.path = PathBuf::from(&line[6..]);
            }
        }
        // Determine file change type
        else if line.starts_with("new file mode ") {
            if let Some(ref mut file) = current_file {
                file.change_type = ChangeType::Added;
            }
        }
        else if line.starts_with("deleted file mode ") {
            if let Some(ref mut file) = current_file {
                file.change_type = ChangeType::Deleted;
            }
        }
        // We don't parse hunk details in this simplified version
        // Just check for hunk headers to potentially count them
        else if line.starts_with("@@ ") {
            if let Some(ref mut file) = current_file {
                // Create a minimal hunk with empty content
                let hunk = DiffHunk {
                    old_range: HunkRange { start: 0, count: 0 },
                    new_range: HunkRange { start: 0, count: 0 },
                    lines: Vec::new(),
                };
                file.hunks.push(hunk);
            }
        }
    }
    
    // Add the last file if it exists
    if let Some(file) = current_file {
        git_diff.changed_files.push(file);
    }
    
    git_diff
}

/// Extract language from file path based on extension
///
/// # Arguments
///
/// * `path` - PathBuf containing the file path
///
/// # Returns
///
/// * `Option<String>` - Detected language name or None
pub fn detect_language_from_path(path: &PathBuf) -> Option<String> {
    let extension = path.extension()?.to_str()?;
    
    match extension.to_lowercase().as_str() {
        "rs" => Some("rust".to_string()),
        "py" => Some("python".to_string()),
        "java" => Some("java".to_string()),
        "js" | "jsx" => Some("javascript".to_string()),
        "ts" | "tsx" => Some("typescript".to_string()),
        "c" => Some("c".to_string()),
        "cpp" | "cc" | "cxx" | "hpp" | "h" => Some("cpp".to_string()),
        "go" => Some("go".to_string()),
        _ => None,
    }
}

/// Get a summary of languages in a GitDiff
///
/// # Arguments
///
/// * `git_diff` - The GitDiff structure to analyze
///
/// # Returns
///
/// * `Vec<(String, usize)>` - Vector of (language, count) pairs
pub fn summarize_languages(git_diff: &GitDiff) -> Vec<(String, usize)> {
    let mut language_counts = std::collections::HashMap::new();
    
    for file in &git_diff.changed_files {
        if let Some(language) = detect_language_from_path(&file.path) {
            *language_counts.entry(language).or_insert(0) += 1;
        }
    }
    
    language_counts.into_iter().collect()
}