// src/tree_sitter_analyzer/parse_utils.rs
//
// This module provides utilities for parsing Git diff output and other text formats.

use std::path::PathBuf;
use crate::tree_sitter_analyzer::core::{GitDiff, ChangedFile, DiffHunk, HunkRange, ChangeType};

/// Parses Git diff output into a GitDiff structure
///
/// # Arguments
///
/// * `diff_text` - The text output from git diff command
///
/// # Returns
///
/// * `Result<GitDiff, String>` - The parsed GitDiff or an error message
pub fn parse_git_diff_text(diff_text: &str) -> Result<GitDiff, String> {
    let mut git_diff = GitDiff {
        changed_files: Vec::new(),
        metadata: None,
    };
    
    let mut current_file: Option<ChangedFile> = None;
    let mut current_hunks: Vec<DiffHunk> = Vec::new();
    
    for line in diff_text.lines() {
        // Parse file headers
        if line.starts_with("diff --git ") {
            // Add previous file if exists
            if let Some(file) = current_file.take() {
                git_diff.changed_files.push(file);
            }
            
            // Create new file entry
            current_file = Some(ChangedFile {
                path: PathBuf::new(),
                change_type: ChangeType::Modified,
                hunks: Vec::new(),
                file_mode_change: None,
            });
            
            current_hunks.clear();
        }
        // Parse file path
        else if line.starts_with("+++ b/") && line.len() > 6 {
            if let Some(ref mut file) = current_file {
                file.path = PathBuf::from(&line[6..]);
            }
        }
        // Parse file change type
        else if line.starts_with("new file mode ") {
            if let Some(ref mut file) = current_file {
                file.change_type = ChangeType::Added;
                file.file_mode_change = Some(line.trim_start_matches("new file mode ").to_string());
            }
        }
        else if line.starts_with("deleted file mode ") {
            if let Some(ref mut file) = current_file {
                file.change_type = ChangeType::Deleted;
                file.file_mode_change = Some(line.trim_start_matches("deleted file mode ").to_string());
            }
        }
        // Parse hunk header
        else if line.starts_with("@@ ") {
            if let Some(ref mut file) = current_file {
                // Extract hunk ranges from header like "@@ -1,5 +2,6 @@"
                let header_parts: Vec<&str> = line.split("@@").collect();
                if header_parts.len() >= 2 {
                    let range_part = header_parts[1].trim();
                    let range_parts: Vec<&str> = range_part.split_whitespace().collect();
                    
                    if range_parts.len() >= 2 {
                        let old_range_str = range_parts[0].trim_start_matches('-');
                        let new_range_str = range_parts[1].trim_start_matches('+');
                        
                        let old_range = parse_range(old_range_str);
                        let new_range = parse_range(new_range_str);
                        
                        let hunk = DiffHunk {
                            old_range: HunkRange { 
                                start: old_range.0, 
                                count: old_range.1 
                            },
                            new_range: HunkRange { 
                                start: new_range.0, 
                                count: new_range.1 
                            },
                            lines: Vec::new(),
                        };
                        
                        current_hunks.push(hunk);
                    }
                }
            }
        }
        // Add line to current hunk
        else if (line.starts_with('+') || line.starts_with('-') || line.starts_with(' ')) 
                && !current_hunks.is_empty() {
            if let Some(last_hunk) = current_hunks.last_mut() {
                last_hunk.lines.push(line.to_string());
            }
        }
    }
    
    // Add the final file if exists
    if let Some(mut file) = current_file {
        file.hunks = current_hunks;
        git_diff.changed_files.push(file);
    }
    
    Ok(git_diff)
}

/// Parses a range string like "1,5" into (start, count)
fn parse_range(range_str: &str) -> (usize, usize) {
    let parts: Vec<&str> = range_str.split(',').collect();
    
    let start = if parts.is_empty() {
        0
    } else {
        parts[0].parse::<usize>().unwrap_or(0)
    };
    
    let count = if parts.len() > 1 {
        parts[1].parse::<usize>().unwrap_or(0)
    } else {
        1 // Default to 1 if not specified
    };
    
    (start, count)
}

/// Extracts language from file path
pub fn detect_language(path: &PathBuf) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "rs" => "rust".to_string(),
            "py" => "python".to_string(),
            "java" => "java".to_string(),
            "js" | "jsx" => "javascript".to_string(),
            "ts" | "tsx" => "typescript".to_string(),
            "c" => "c".to_string(),
            "cpp" | "cc" | "cxx" | "hpp" | "h" => "cpp".to_string(),
            "go" => "go".to_string(),
            _ => "unknown".to_string(),
        })
}