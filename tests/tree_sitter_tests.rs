// Integration tests for Tree-sitter functionality

#[cfg(test)]
mod tree_sitter_integration_tests {
    use gitie::config_management::settings::TreeSitterConfig;
    // Updated import to reflect new module structure
    use gitie::tree_sitter_analyzer::TreeSitterAnalyzer;
    use gitie::tree_sitter_analyzer::core::ChangeType;
    use std::path::PathBuf;

    #[test]
    fn test_tree_sitter_analyzer_creation() {
        // Create with default configuration
        let config = TreeSitterConfig::default();
        let analyzer = TreeSitterAnalyzer::new(config);
        
        assert!(analyzer.is_ok(), "Should be able to create analyzer with default config");
    }
    
    #[test]
    fn test_language_detection() {
        // Create analyzer
        let config = TreeSitterConfig::default();
        let analyzer = TreeSitterAnalyzer::new(config).unwrap();
        
        // Test common languages
        assert_eq!(analyzer.detect_language(&PathBuf::from("file.rs")).unwrap(), "rust");
        assert_eq!(analyzer.detect_language(&PathBuf::from("script.js")).unwrap(), "javascript");
        assert_eq!(analyzer.detect_language(&PathBuf::from("module.py")).unwrap(), "python");
        assert_eq!(analyzer.detect_language(&PathBuf::from("Main.java")).unwrap(), "java");
        
        // Test TypeScript extension (should be detected as JavaScript)
        assert_eq!(analyzer.detect_language(&PathBuf::from("component.tsx")).unwrap(), "javascript");
        
        // Test unsupported language
        let result = analyzer.detect_language(&PathBuf::from("unknown.xyz"));
        assert!(result.is_err(), "Should return error for unsupported language");
    }
    
    #[test]
    fn test_diff_parsing() {
        let diff_text = r#"diff --git a/src/main.rs b/src/main.rs
index 83db48f..2c6f1f0 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,5 @@
 fn main() {
-    println!("Hello, world!");
+    println!("Hello, Tree-sitter!");
 }
"#;
        
        // Create analyzer
        let config = TreeSitterConfig::default();
        let mut analyzer = TreeSitterAnalyzer::new(config).unwrap();
        
        // Parse diff
        let result = analyzer.analyze_diff(diff_text);
        assert!(result.is_ok(), "Should be able to parse simple diff");
        
        let analysis = result.unwrap();
        assert!(!analysis.file_analyses.is_empty(), "Should contain file analyses");
        
        // Verify the first file analysis
        let file_analysis = &analysis.file_analyses[0];
        assert_eq!(file_analysis.path, PathBuf::from("src/main.rs"));
        assert_eq!(file_analysis.change_type, ChangeType::Modified);
    }
    
    #[test]
    fn test_analysis_with_multiple_files() {
        let diff_text = r#"diff --git a/src/main.rs b/src/main.rs
index 83db48f..2c6f1f0 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,5 @@
 fn main() {
-    println!("Hello, world!");
+    println!("Hello, Tree-sitter!");
 }
diff --git a/src/lib.rs b/src/lib.rs
new file mode 100644
index 0000000..f0eacc8
--- /dev/null
+++ b/src/lib.rs
@@ -0,0 +1,3 @@
+pub fn hello() -> &'static str {
+    "Hello from lib"
+}
diff --git a/src/old.rs b/src/old.rs
deleted file mode 100644
index 83db48f..0000000
--- a/src/old.rs
+++ /dev/null
@@ -1,3 +0,0 @@
-fn old_function() {
-    println!("This is old");
-}
"#;
        
        // Create analyzer
        let config = TreeSitterConfig::default();
        let mut analyzer = TreeSitterAnalyzer::new(config).unwrap();
        
        // Analyze diff
        let result = analyzer.analyze_diff(diff_text);
        assert!(result.is_ok(), "Should be able to analyze complex diff");
        
        let analysis = result.unwrap();
        
        // Should have 3 files: modified, added, and deleted
        assert_eq!(analysis.file_analyses.len(), 3, "Should have 3 file analyses");
        
        // Each file should have the correct change type
        let change_types: Vec<ChangeType> = analysis.file_analyses
            .iter()
            .map(|a| a.change_type.clone())
            .collect();
            
        assert!(change_types.contains(&ChangeType::Modified));
        assert!(change_types.contains(&ChangeType::Added));
        assert!(change_types.contains(&ChangeType::Deleted));
    }
}