// src/tree_sitter_analyzer/analyzer.rs
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, error, warn};
use tree_sitter::{Language, Parser, Query}; // Added Language here

use super::core::{
    AffectedNode,
    ChangeAnalysis,
    ChangePattern,
    ChangeScope,
    ChangeType,
    ChangedFile,
    DiffAnalysis,
    DiffHunk,
    FileAnalysis,
    // Assuming core.rs will export these
    FileAst,
    GitDiff,
    HunkRange,
    get_tree_sitter_go,
    // calculate_hash, // Assuming this will be in core or a utils.rs
    // parse_git_diff, // Assuming this will be in core or a utils.rs
    get_tree_sitter_java,
    get_tree_sitter_python,
    get_tree_sitter_rust,
};
use super::java::JavaProjectStructure;
use super::java::{
    extract_java_class_name,
    extract_java_class_relations,
    extract_java_imports,
    extract_java_methods,
    // Import functions from java.rs module
    extract_java_package_name,
};
use crate::config_management::settings::TreeSitterConfig;
use crate::core::errors::TreeSitterError; // Updated path
// use super::rust::{analyze_rust_changes}; // Example for rust

// Now using calculate_hash from core.rs
use super::core::calculate_hash;

#[derive(Debug)]
pub struct TreeSitterAnalyzer {
    pub config: TreeSitterConfig,
    pub project_root: PathBuf,
    languages: HashMap<String, Language>,
    file_asts: HashMap<PathBuf, FileAst>, // Cache for parsed file ASTs
    queries: HashMap<String, Query>,      // Cache for compiled queries
                                          // parser_cache: HashMap<String, Parser>, // If we want to reuse parsers
}

impl TreeSitterAnalyzer {
    pub fn new(config: TreeSitterConfig) -> Result<Self, TreeSitterError> {
        let mut analyzer = Self {
            config,
            project_root: PathBuf::new(), // Set later with set_project_root
            languages: HashMap::new(),
            file_asts: HashMap::new(),
            queries: HashMap::new(),
        };
        analyzer.initialize_languages()?;
        analyzer.initialize_queries()?;
        Ok(analyzer)
    }

    pub fn set_project_root(&mut self, root: PathBuf) {
        self.project_root = root;
        // Potentially clear or update caches if project root changes
        self.file_asts.clear();
    }

    /// Create a simple GitDiff from diff text
    ///
    /// This is a simplified version that doesn't attempt advanced parsing
    /// but guarantees the correct return type structure
    pub fn create_simple_git_diff(&self, diff_text: &str) -> GitDiff {
        // Use the parse_utils parser for more robust diff parsing
        match crate::tree_sitter_analyzer::parse_utils::parse_git_diff_text(diff_text) {
            Ok(git_diff) => git_diff,
            Err(_) => {
                // Fallback to the simplest parser if the new one fails
                crate::tree_sitter_analyzer::simple_diff::parse_simple_diff(diff_text)
            }
        }
    }

    /// Parse Git diff text into a structured GitDiff
    ///
    /// This function takes a Git diff text output and parses it into
    /// a structured GitDiff object for further analysis.
    ///
    /// # Arguments
    ///
    /// * `diff_text` - The Git diff text output from git diff command
    ///
    /// # Returns
    ///
    /// * `Result<GitDiff, TreeSitterError>` - The parsed GitDiff or an error
    pub fn parse_git_diff_text(&self, diff_text: &str) -> Result<GitDiff, TreeSitterError> {
        let mut git_diff = GitDiff {
            changed_files: Vec::new(),
            metadata: None,
        };

        if diff_text.trim().is_empty() {
            return Ok(git_diff);
        }

        let mut current_file: Option<ChangedFile> = None;
        let mut current_hunk: Option<DiffHunk> = None;

        for line in diff_text.lines() {
            // Process diff header lines
            if line.starts_with("diff --git ") {
                // Save previous file if exists
                if let Some(file) = current_file.take() {
                    git_diff.changed_files.push(file);
                }

                // Start new file
                current_file = Some(ChangedFile {
                    path: PathBuf::new(),
                    change_type: ChangeType::Modified,
                    hunks: Vec::new(),
                    file_mode_change: None,
                });
            }
            // Process file path lines
            else if line.starts_with("--- ") || line.starts_with("+++ ") {
                if let Some(ref mut file) = current_file {
                    if line.starts_with("+++ b/") && line.len() > 6 {
                        file.path = PathBuf::from(&line[6..]);
                    }
                }
            }
            // Process file mode changes
            else if line.starts_with("new file mode ") {
                if let Some(ref mut file) = current_file {
                    file.change_type = ChangeType::Added;
                }
            } else if line.starts_with("deleted file mode ") {
                if let Some(ref mut file) = current_file {
                    file.change_type = ChangeType::Deleted;
                }
            }
            // Process hunk header
            else if line.starts_with("@@ ") {
                // Save previous hunk if exists
                if let Some(hunk) = current_hunk.take() {
                    if let Some(ref mut file) = current_file {
                        file.hunks.push(hunk);
                    }
                }

                // Parse hunk header: @@ -start,count +start,count @@
                let end_pos = line.find(" @@");
                if let Some(pos) = end_pos {
                    let header = &line[3..pos];
                    let parts: Vec<&str> = header.split(' ').collect();

                    if parts.len() >= 2 {
                        let old_range_str = parts[0].trim_start_matches('-');
                        let new_range_str = parts[1].trim_start_matches('+');

                        let old_range = Self::parse_hunk_range(old_range_str);
                        let new_range = Self::parse_hunk_range(new_range_str);

                        current_hunk = Some(DiffHunk {
                            old_range,
                            new_range,
                            lines: Vec::new(),
                        });
                    }
                }
            }
            // Process hunk content
            else if line.starts_with('+') || line.starts_with('-') || line.starts_with(' ') {
                if let Some(ref mut hunk) = current_hunk {
                    hunk.lines.push(line.to_string());
                }
            }
        }

        // Add final hunk and file
        if let Some(hunk) = current_hunk.take() {
            if let Some(ref mut file) = current_file {
                file.hunks.push(hunk);
            }
        }

        if let Some(file) = current_file.take() {
            git_diff.changed_files.push(file);
        }

        Ok(git_diff)
    }

    /// Parse hunk range string like "1,5" into a HunkRange
    fn parse_hunk_range(range_str: &str) -> HunkRange {
        let parts: Vec<&str> = range_str.split(',').collect();

        let start = parts
            .get(0)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        let count = if parts.len() > 1 {
            parts
                .get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
        } else {
            1 // Default count is 1 if not specified
        };

        HunkRange { start, count }
    }

    fn initialize_languages(&mut self) -> Result<(), TreeSitterError> {
        // Load languages based on config or defaults
        // Example for Rust and Java
        self.languages
            .insert("rust".to_string(), get_tree_sitter_rust());
        self.languages
            .insert("java".to_string(), get_tree_sitter_java());

        // Add Python and Go based on configuration
        if self.config.languages.contains(&"python".to_string()) {
            self.languages
                .insert("python".to_string(), get_tree_sitter_python());
        }
        if self.config.languages.contains(&"go".to_string()) {
            self.languages
                .insert("go".to_string(), get_tree_sitter_go());
        }
        // Potentially load tree_sitter_javascript if configured
        Ok(())
    }

    // Method to get Rust query pattern (moved from the original monolithic file)
    pub(crate) fn get_rust_query_pattern(&self) -> String {
        r#"
        ; 函数定义
        (function_item) @function.declaration
        (function_item name: (identifier) @function.name)

        ; 结构体定义
        (struct_item) @struct.declaration
        (struct_item name: (type_identifier) @struct.name)

        ; 枚举定义
        (enum_item) @enum.declaration
        (enum_item name: (type_identifier) @enum.name)

        ; 特性定义
        (trait_item) @trait.declaration
        (trait_item name: (type_identifier) @trait.name)

        ; 实现块
        (impl_item) @impl.declaration
        ; TODO: Capture impl target type and trait if present

        ; 模块定义
        (mod_item) @module.declaration
        (mod_item name: (identifier) @module.name)

        ; 常量定义
        (const_item) @const.declaration
        (const_item name: (identifier) @const.name)

        ; 静态变量定义
        (static_item) @static.declaration
        (static_item name: (identifier) @static.name)

        ; 类型别名
        (type_item) @type_alias.declaration
        (type_item name: (type_identifier) @type_alias.name)

        ; 宏定义
        (macro_definition) @macro.declaration
        (macro_definition name: (identifier) @macro.name)

        ; 使用声明
        (use_declaration) @use.declaration

        ; 属性标记 (捕获整个属性)
        (attribute_item) @attribute
        "#
        .to_string()
    }

    // Method to get Java query pattern (moved from the original monolithic file)
    pub(crate) fn get_java_query_pattern(&self) -> String {
        r#"
        ; Class declarations
        (class_declaration name: (identifier) @class.name) @class.declaration

        ; Interface declarations
        (interface_declaration name: (identifier) @interface.name) @interface.declaration

        ; Enum declarations
        (enum_declaration name: (identifier) @enum.name) @enum.declaration

        ; Method declarations
        (method_declaration name: (identifier) @method.name) @method.declaration

        ; Constructor declarations
        (constructor_declaration name: (identifier) @constructor.name) @constructor.declaration

        ; Field declarations (instance variables)
        (field_declaration declarator: (variable_declarator name: (identifier) @field.name)) @field.declaration

        ; Static field declarations
        (field_declaration (modifiers (marker_annotation name: (identifier) @annotation.name))? (type_identifier) (variable_declarator name: (identifier) @static.field.name)) @static.field.declaration

        ; Package declaration
        (package_declaration (scoped_identifier) @package.name) @package.declaration

        ; Import statements
        (import_declaration (scoped_identifier) @import.name) @import.declaration
        (import_declaration (asterisk) @import.wildcard) @import.declaration

        ; Annotations
        (marker_annotation name: (identifier) @annotation.name) @annotation.declaration
        (annotation name: (identifier) @annotation.name) @annotation.declaration

        ; Method invocations (useful for call graphs, but can be noisy)
        ; (method_invocation name: (identifier) @method.call)

        ; Object creation (constructors)
        ; (object_creation_expression type: (type_identifier) @constructor.call)
        "#
        .to_string()
    }

    fn initialize_queries(&mut self) -> Result<(), TreeSitterError> {
        if self.languages.contains_key("rust") {
            let rust_query_pattern = self.get_rust_query_pattern();
            let rust_lang = self.languages.get("rust").unwrap(); // Safe due to check
            let query = Query::new(*rust_lang, &rust_query_pattern)
                .map_err(|e| TreeSitterError::QueryError(format!("Rust query error: {}", e)))?;
            self.queries.insert("rust".to_string(), query);
        }
        if self.languages.contains_key("java") {
            let java_query_pattern = self.get_java_query_pattern();
            let java_lang = self.languages.get("java").unwrap(); // Safe due to check
            let query = Query::new(*java_lang, &java_query_pattern)
                .map_err(|e| TreeSitterError::QueryError(format!("Java query error: {}", e)))?;
            self.queries.insert("java".to_string(), query);
        }
        // Initialize queries for other languages...
        Ok(())
    }

    pub fn detect_language(&self, path: &Path) -> Result<Option<String>, TreeSitterError> {
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match extension {
            "rs" => Ok(Some("rust".to_string())),
            "java" => Ok(Some("java".to_string())),
            "py" => Ok(Some("python".to_string())),
            "go" => Ok(Some("go".to_string())),
            "js" | "ts" | "jsx" | "tsx" => Ok(Some("javascript".to_string())), // Group JS/TS
            // 明确标识为非代码文件，不需要 tree-sitter 分析
            "md" | "markdown" | "txt" | "json" | "yml" | "yaml" | "toml" | "xml" | "html"
            | "css" | "svg" | "png" | "jpg" | "jpeg" | "gif" | "ico" => Ok(None),
            // 对于未知扩展名，返回 None 而不是错误，避免错误日志过多
            _ => Ok(None),
        }
    }

    fn is_cache_valid(&self, path: &Path, current_hash: &str) -> bool {
        if !self.config.cache_enabled {
            return false;
        }
        match self.file_asts.get(path) {
            Some(cached_ast) => cached_ast.content_hash == current_hash,
            None => false,
        }
    }

    #[allow(dead_code)]
    fn check_cache(&self, path: &Path) -> Option<FileAst> {
        if !self.config.cache_enabled {
            return None;
        }
        // More sophisticated cache validation could be added here (e.g., timestamp)
        self.file_asts.get(path).cloned()
    }

    pub fn parse_file(&mut self, file_path: &Path) -> Result<FileAst, TreeSitterError> {
        let lang_id_opt = self.detect_language(file_path)?;
        let lang_id = lang_id_opt.ok_or_else(|| {
            TreeSitterError::UnsupportedLanguage(format!("Non-code file: {:?}", file_path))
        })?;
        let language = self.languages.get(&lang_id).ok_or_else(|| {
            TreeSitterError::UnsupportedLanguage(format!("Language '{}' not initialized.", lang_id))
        })?;

        let source_code = fs::read_to_string(file_path).map_err(|e| TreeSitterError::IoError(e))?;

        let current_hash = calculate_hash(&source_code);

        if self.is_cache_valid(file_path, &current_hash) {
            if let Some(cached_ast) = self.file_asts.get(file_path) {
                debug!("Using cached AST for {:?}", file_path);
                return Ok(cached_ast.clone());
            }
        }

        let mut parser = Parser::new();
        parser.set_language(*language).map_err(|e| {
            TreeSitterError::ParseError(format!("Failed to set language for parser: {}", e))
        })?;

        let tree = parser.parse(&source_code, None).ok_or_else(|| {
            TreeSitterError::ParseError(format!("Failed to parse file: {:?}", file_path))
        })?;

        let ast = FileAst {
            path: file_path.to_path_buf(),
            tree,
            source: source_code,
            content_hash: current_hash,
            last_parsed: SystemTime::now(),
            language_id: lang_id,
        };

        if self.config.cache_enabled {
            self.file_asts.insert(file_path.to_path_buf(), ast.clone());
            debug!("Cached AST for {:?}", file_path);
        }

        Ok(ast)
    }

    // Placeholder for is_node_public - this needs the FileAst context
    pub fn is_node_public(&self, node: &tree_sitter::Node, file_ast: &FileAst) -> bool {
        // Implementation depends on language-specific rules
        // This is a simplified placeholder.
        match file_ast.language_id.as_str() {
            "rust" => {
                // Simplified: check for `pub` keyword in the node's S-expression or text
                // A more robust way would be to traverse child nodes for `visibility_modifier`
                // or check the parent `mod_item` or `impl_item` etc.
                let node_sexp = node.to_sexp();
                if node_sexp.starts_with("(visibility_modifier")
                    && node
                        .utf8_text(file_ast.source.as_bytes())
                        .unwrap_or("")
                        .contains("pub")
                {
                    return true;
                }
                // Check if it's a direct child of a `source_file` and doesn't have explicit private/crate visibility
                // This is very simplified and likely incorrect for many Rust visibility rules.
                let mut cursor = node.walk();
                for child_node in node.children(&mut cursor) {
                    if child_node.kind() == "visibility_modifier" {
                        return child_node
                            .utf8_text(file_ast.source.as_bytes())
                            .unwrap_or("")
                            .contains("pub");
                    }
                }
                // If no visibility modifier, default depends on context (e.g. in trait, it's public)
                // This is a complex part of Rust's grammar.
                false // Default to not public if unsure
            }
            "java" => {
                // Check for `public` modifier
                let mut cursor = node.walk();
                for child_node in node.children(&mut cursor) {
                    if child_node.kind() == "modifiers" {
                        let modifiers_text = child_node
                            .utf8_text(file_ast.source.as_bytes())
                            .unwrap_or("");
                        return modifiers_text.contains("public");
                    }
                }
                // Default to package-private if no explicit public modifier (for top-level types)
                // or based on context for members.
                false
            }
            _ => false, // Default for other languages
        }
    }

    // analyze_diff, map_diff_to_ast, analyze_changes, etc. will go here
    // These are complex and will require careful porting from the original file.

    pub fn analyze_diff(&mut self, diff_text: &str) -> Result<DiffAnalysis, TreeSitterError> {
        // Parse the diff text to get structured representation
        let git_diff = super::core::parse_git_diff(diff_text)?;

        let mut file_analyses = Vec::new();
        let mut total_affected_nodes = 0;
        let mut language_counts = HashMap::new();
        let mut total_additions = 0;
        let mut total_deletions = 0;
        let mut total_modifications = 0;

        // 收集变更统计信息
        let mut function_changes = 0;
        let mut type_changes = 0;
        let mut method_changes = 0;
        let mut interface_changes = 0;
        let mut other_changes = 0;

        // 记录语言特定变更
        let mut java_changes = 0;
        let mut rust_changes = 0;
        let mut _python_changes = 0;
        let mut _go_changes = 0;

        for file_diff_info in &git_diff.changed_files {
            match file_diff_info.change_type {
                ChangeType::Added | ChangeType::Modified => {
                    // For added/modified files, parse them
                    // The path in FileDiff should be the new path
                    let file_path = self.project_root.join(&file_diff_info.path);
                    if !file_path.exists() {
                        warn!(
                            "File {:?} mentioned in diff does not exist in project. Skipping.",
                            file_path
                        );
                        continue;
                    }

                    // 首先检查文件类型是否支持 tree-sitter 分析
                    match self.detect_language(&file_path) {
                        Ok(Some(lang_id)) => {
                            // 统计语言分布
                            *language_counts.entry(lang_id.clone()).or_insert(0) += 1;

                            // 记录各语言变更
                            match lang_id.as_str() {
                                "java" => java_changes += 1,
                                "rust" => rust_changes += 1,
                                "python" => _python_changes += 1,
                                "go" => _go_changes += 1,
                                _ => {}
                            }

                            // 支持的编程语言，继续 tree-sitter 分析
                            match self.parse_file(&file_path) {
                                Ok(file_ast) => {
                                    // Analyze changes within this file based on hunks
                                    let affected_nodes = self
                                        .analyze_file_changes(&file_ast, &file_diff_info.hunks)?;
                                    total_affected_nodes += affected_nodes.len();

                                    // 统计不同类型的变更
                                    for node in &affected_nodes {
                                        match node.node_type.as_str() {
                                            "function" | "test_function" => function_changes += 1,
                                            "class" | "struct" | "enum" | "interface" | "type"
                                            | "class_structure" | "debuggable_struct" => {
                                                type_changes += 1
                                            }
                                            "method" | "overridden_method" | "api_endpoint" => {
                                                method_changes += 1
                                            }
                                            "trait" => interface_changes += 1,
                                            _ => other_changes += 1,
                                        }

                                        if let Some(change_type) = &node.change_type {
                                            match change_type.as_str() {
                                                "added" | "added_content" => total_additions += 1,
                                                "deleted" => total_deletions += 1,
                                                "modified" | "modified_with_deletion" => {
                                                    total_modifications += 1
                                                }
                                                _ => {}
                                            }
                                        }
                                    }

                                    // 根据文件语言和变更类型生成更有意义的摘要
                                    let summary = match file_ast.language_id.as_str() {
                                        "java" => self
                                            .generate_java_file_summary(&file_ast, &affected_nodes),
                                        "rust" => self
                                            .generate_rust_file_summary(&file_ast, &affected_nodes),
                                        _ => format!(
                                            "文件 {} 被{}。影响了 {} 个代码结构。",
                                            file_ast.path.display(),
                                            match file_diff_info.change_type {
                                                ChangeType::Added => "新增",
                                                ChangeType::Modified => "修改",
                                                _ => "变更",
                                            },
                                            affected_nodes.len()
                                        ),
                                    };

                                    file_analyses.push(FileAnalysis {
                                        path: file_ast.path.clone(),
                                        language: file_ast.language_id.clone(),
                                        change_type: file_diff_info.change_type.clone(),
                                        affected_nodes: affected_nodes.clone(),
                                        summary: Some(summary),
                                    });

                                    // 输出更详细的变更日志
                                    for node in &affected_nodes {
                                        if let Some(change_type) = &node.change_type {
                                            debug!(
                                                "变更: {} {} - {}",
                                                change_type, node.node_type, node.name
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse file {:?}: {:?}", file_path, e);
                                    file_analyses.push(FileAnalysis {
                                        path: file_path.clone(),
                                        language: lang_id,
                                        change_type: file_diff_info.change_type.clone(),
                                        affected_nodes: Vec::new(),
                                        summary: Some(format!("无法解析文件: {}", e)),
                                    });
                                }
                            }
                        }
                        Ok(None) => {
                            debug!(
                                "File {:?} has unsupported language. Using simple analysis.",
                                file_path
                            );
                            // For unsupported languages, add a placeholder analysis
                            file_analyses.push(FileAnalysis {
                                path: file_path.clone(),
                                language: "unknown".to_string(),
                                change_type: file_diff_info.change_type.clone(),
                                affected_nodes: Vec::new(),
                                summary: Some(format!(
                                    "不支持的文件类型，无法进行 tree-sitter 分析"
                                )),
                            });
                        }
                        Err(e) => {
                            error!("Error detecting language for file {:?}: {:?}", file_path, e);
                            file_analyses.push(FileAnalysis {
                                path: file_path.clone(),
                                language: "error".to_string(),
                                change_type: file_diff_info.change_type.clone(),
                                affected_nodes: Vec::new(),
                                summary: Some(format!("检测文件语言时出错: {}", e)),
                            });
                        }
                    }
                }
                // For other change types like deletions, just add an analysis entry
                _ => {
                    file_analyses.push(FileAnalysis {
                        path: file_diff_info.path.clone(),
                        language: "unknown".to_string(), // We might not be able to determine language for deleted files
                        change_type: file_diff_info.change_type.clone(),
                        affected_nodes: Vec::new(),
                        summary: Some(format!(
                            "文件被{}",
                            match file_diff_info.change_type {
                                ChangeType::Deleted => "删除",
                                ChangeType::Renamed => "重命名",
                                _ => "其他操作",
                            }
                        )),
                    });
                }
            }
        }

        // 确定变更模式
        let change_pattern = self.determine_change_pattern(
            &file_analyses,
            function_changes,
            type_changes,
            method_changes,
            interface_changes,
            java_changes,
            rust_changes,
            total_additions,
            total_deletions,
        );

        // 确定变更范围
        let change_scope = self.determine_change_scope(
            total_affected_nodes,
            total_additions,
            total_deletions,
            total_modifications,
            type_changes,
            method_changes,
            interface_changes,
        );

        // 生成整体摘要
        let languages_summary = if !language_counts.is_empty() {
            let mut lang_parts = Vec::new();
            for (lang, count) in &language_counts {
                lang_parts.push(format!("{} ({}个文件)", lang, count));
            }
            format!("涉及语言: {}", lang_parts.join(", "))
        } else {
            "未检测到支持的编程语言".to_string()
        };

        let changes_summary = format!(
            "变更统计: {}个新增, {}个删除, {}个修改，共影响{}个代码结构",
            total_additions, total_deletions, total_modifications, total_affected_nodes
        );

        let structure_summary = format!(
            "结构变更: {}个函数, {}个类型, {}个方法, {}个接口, {}个其他",
            function_changes, type_changes, method_changes, interface_changes, other_changes
        );

        let pattern_description = match &change_pattern {
            ChangePattern::FeatureImplementation => "新功能实现",
            ChangePattern::BugFix => "bug修复",
            ChangePattern::Refactoring => "代码重构",
            ChangePattern::ModelChange => "模型变更",
            ChangePattern::BehaviorChange => "行为变更",
            ChangePattern::ConfigurationChange => "配置变更",
            ChangePattern::MixedChange => "混合变更",
            ChangePattern::LanguageSpecificChange(lang_change) => {
                if lang_change.starts_with("Java") {
                    "Java特定变更"
                } else if lang_change.starts_with("Rust") {
                    "Rust特定变更"
                } else {
                    "特定语言变更"
                }
            }
        };

        let scope_description = match &change_scope {
            ChangeScope::Minor => "轻微",
            ChangeScope::Moderate => "中等",
            ChangeScope::Major => "重大",
        };

        let overall_summary = format!(
            "变更分析完成。{}\n{}\n{}\n变更类型: {} ({})",
            languages_summary,
            changes_summary,
            structure_summary,
            pattern_description,
            scope_description
        );

        // 构建完整的变更分析结果
        let change_analysis = ChangeAnalysis {
            function_changes,
            type_changes,
            method_changes,
            interface_changes,
            other_changes,
            change_pattern,
            change_scope,
        };

        Ok(DiffAnalysis {
            file_analyses,
            overall_summary,
            change_analysis,
        })
    }

    fn analyze_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        // 根据文件语言调用相应的分析方法
        match file_ast.language_id.as_str() {
            "java" => self.analyze_java_file_changes(file_ast, hunks),
            "rust" => self.analyze_rust_file_changes(file_ast, hunks),
            // 其他语言可以在这里添加
            _ => self.analyze_generic_file_changes(file_ast, hunks),
        }
    }

    fn analyze_java_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

        // Java-specific analysis enhancements
        for node in &mut affected_nodes {
            // Safely get node content for deeper inspection
            let content = node.content.as_ref().map(|c| c.as_str()).unwrap_or("");
            match node.node_type.as_str() {
                // Class structure changes, detect Spring and JPA components
                "class" => {
                    node.node_type = "class_structure".to_string();
                    if content.contains("@Service")
                        || content.contains("@Component")
                        || content.contains("@Controller")
                        || content.contains("@Repository")
                    {
                        node.node_type = "spring_component".to_string();
                    } else if content.contains("@Entity") || content.contains("@Table") {
                        node.node_type = "jpa_entity".to_string();
                    }
                }
                // Interface definition changes
                "interface" => {
                    node.node_type = "interface_structure".to_string();
                }
                // Enum definition changes
                "enum" => {
                    node.node_type = "enum_structure".to_string();
                }
                // Annotation type definitions
                "annotation" => {
                    node.node_type = "annotation_interface".to_string();
                }
                // Constructor changes
                "constructor" => {
                    node.node_type = "constructor".to_string();
                }
                // Field-level changes, detect injection
                "field" => {
                    if content.contains("@Autowired") || content.contains("@Inject") {
                        node.node_type = "injected_field".to_string();
                    } else {
                        node.node_type = "field".to_string();
                    }
                }
                // Method-level changes, detect overrides, endpoints, tests, throws
                "method" => {
                    // Detect overridden methods
                    if content.contains("@Override") {
                        node.node_type = "overridden_method".to_string();
                    }
                    // Detect Spring REST endpoints
                    if content.contains("@GetMapping")
                        || content.contains("@PostMapping")
                        || content.contains("@RequestMapping")
                    {
                        node.node_type = "api_endpoint".to_string();
                    }
                    // Detect JUnit tests
                    if content.contains("@Test") {
                        node.node_type = "test_method".to_string();
                    }
                    // Detect declared exceptions
                    if content.contains("throws ") {
                        node.node_type = "throws_declaration".to_string();
                    }
                }
                // Package declaration
                "package" => {
                    node.node_type = "package_declaration".to_string();
                }
                // Import statements
                "import" => {
                    node.node_type = "import_statement".to_string();
                }
                // Lambda expressions
                "lambda" => {
                    node.node_type = "lambda_expression".to_string();
                }
                // Static initializer blocks
                "static" => {
                    node.node_type = "static_initializer".to_string();
                }
                _ => {
                    // Leave other node types unchanged
                }
            }
        }
        Ok(affected_nodes)
    }

    // fn analyze_java_file_changes(&self, file_ast: &FileAst, hunks: &[DiffHunk]) -> Result<Vec<AffectedNode>, TreeSitterError> {
    //     let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;
    //
    //     // Java 特定分析逻辑
    //     let source_bytes = file_ast.source.as_bytes();
    //
    //     // 分析类结构变更
    //     for node in &mut affected_nodes {
    //         // 对类定义的特殊处理
    //         if node.node_type == "class" {
    //             // 标记为结构变更
    //             node.node_type = "class_structure".to_string();
    //
    //             // 额外分析类中的方法和字段
    //             if let Some(content) = &node.content {
    //                 // 判断是否含有特定注解
    //                 if content.contains("@Service") || content.contains("@Component") ||
    //                    content.contains("@Controller") || content.contains("@Repository") {
    //                     node.node_type = "spring_component".to_string();
    //                 }
    //
    //                 if content.contains("@Entity") || content.contains("@Table") {
    //                     node.node_type = "jpa_entity".to_string();
    //                 }
    //             }
    //         }
    //
    //         // 对方法的特殊处理
    //         if node.node_type == "method" {
    //             // 分析方法签名变更
    //             if let Some(content) = &node.content {
    //                 if content.contains("@Override") {
    //                     node.node_type = "overridden_method".to_string();
    //                 }
    //
    //                 // 检查是否是API端点
    //                 if content.contains("@GetMapping") || content.contains("@PostMapping") ||
    //                    content.contains("@RequestMapping") {
    //                     node.node_type = "api_endpoint".to_string();
    //                 }
    //             }
    //         }
    //     }
    //
    //     Ok(affected_nodes)
    // }
    fn analyze_rust_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

        // —— Rust-specific analysis enhancements ——
        for node in &mut affected_nodes {
            // 准备内容进行匹配
            let content = node.content.as_deref().unwrap_or("");
            // 将基础 node_type 映射为更丰富的 Rust 语义分类
            node.node_type = match node.node_type.as_str() {
                // Structs: check for Debug or other derives
                "struct" => {
                    if content.contains("#[derive(Debug") || content.contains("impl Debug") {
                        "debuggable_struct"
                    } else {
                        "struct_definition"
                    }
                }
                // Enums: similarly detect Debug
                "enum" => {
                    if content.contains("#[derive(Debug") {
                        "debuggable_enum"
                    } else {
                        "enum_definition"
                    }
                }
                // Traits
                "trait" => "trait_definition",
                // Impl blocks: trait impl vs inherent
                "impl" => {
                    if content.contains(" for ") {
                        "trait_impl"
                    } else {
                        "inherent_impl"
                    }
                }
                // Modules
                "module" => {
                    if content.contains("pub mod ") {
                        "public_module"
                    } else {
                        "module_definition"
                    }
                }
                // Constants and statics
                "const" => "constant_definition",
                "static" => "static_definition",
                // Type aliases
                "type_alias" => "type_alias_definition",
                // Macro definitions vs invocations
                "macro" => {
                    if content.contains("macro_rules!") {
                        "macro_definition"
                    } else {
                        "macro_invocation"
                    }
                }
                // Use declarations
                "use" => "use_declaration",
                // Functions: test, async, unsafe, public, or normal
                "function" => {
                    if content.contains("#[test") {
                        "test_function"
                    } else if content.contains("async fn") {
                        "async_function"
                    } else if content.contains("unsafe fn") {
                        "unsafe_function"
                    } else if content.contains("pub fn") {
                        "public_function"
                    } else {
                        "function_definition"
                    }
                }
                // Attributes (e.g. #[derive], #[test])
                "attribute" => "attribute_item",
                // Fallback to leave unchanged
                other => other,
            }
            .to_string();
            // 任何包含 `pub ` 的节点都标记为公开
            if content.contains("pub ") {
                node.is_public = true;
            }
        }

        Ok(affected_nodes)
    }
    // fn analyze_rust_file_changes(
    //     &self,
    //     file_ast: &FileAst,
    //     hunks: &[DiffHunk],
    // ) -> Result<Vec<AffectedNode>, TreeSitterError> {
    //     let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

    //     // Rust 特定分析逻辑
    //     let source_bytes = file_ast.source.as_bytes();

    //     for node in &mut affected_nodes {
    //         // 对结构体的特殊处理
    //         if node.node_type == "struct" {
    //             // 检查是否实现了特定 trait
    //             if let Some(content) = &node.content {
    //                 if content.contains("impl Debug") || content.contains("#[derive(Debug") {
    //                     node.node_type = "debuggable_struct".to_string();
    //                 }

    //                 if content.contains("pub ") {
    //                     node.is_public = true;
    //                 }
    //             }
    //         }

    //         // 对函数的特殊处理
    //         if node.node_type == "function" {
    //             if let Some(content) = &node.content {
    //                 // 检查是否是测试函数
    //                 if content.contains("#[test") {
    //                     node.node_type = "test_function".to_string();
    //                 }

    //                 // 检查是否有宏
    //                 if content.contains("macro_rules!") {
    //                     node.node_type = "macro".to_string();
    //                 }
    //             }
    //         }
    //     }

    //     Ok(affected_nodes)
    // }

    fn generate_java_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        let _source_bytes = file_ast.source.as_bytes(); // Keep but mark as unused
        // 统计各类型节点数量
        let mut class_count = 0;
        let mut method_count = 0;
        let mut field_count = 0;
        let mut api_count = 0;
        let mut spring_component_count = 0;
        let mut jpa_entity_count = 0;

        // 统计变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;

        for node in affected_nodes {
            match node.node_type.as_str() {
                "class" | "class_structure" => class_count += 1,
                "method" | "overridden_method" => method_count += 1,
                "field" => field_count += 1,
                "api_endpoint" => api_count += 1,
                "spring_component" => spring_component_count += 1,
                "jpa_entity" => jpa_entity_count += 1,
                _ => {}
            }

            if let Some(change_type) = &node.change_type {
                match change_type.as_str() {
                    "added" | "added_content" => additions += 1,
                    "deleted" => deletions += 1,
                    "modified" | "modified_with_deletion" => modifications += 1,
                    _ => {}
                }
            }
        }

        // 生成摘要
        let mut summary = format!("Java文件 {} 变更分析: ", file_ast.path.display());

        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 添加结构变更摘要
        let mut structure_summary = Vec::new();
        if class_count > 0 {
            structure_summary.push(format!("{}个类", class_count));
        }
        if method_count > 0 {
            structure_summary.push(format!("{}个方法", method_count));
        }
        if field_count > 0 {
            structure_summary.push(format!("{}个字段", field_count));
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

        // 添加框架特定变更
        if spring_component_count > 0 || jpa_entity_count > 0 || api_count > 0 {
            summary.push_str("。包含");
            let mut framework_summary = Vec::new();

            if spring_component_count > 0 {
                framework_summary.push(format!("{}个Spring组件", spring_component_count));
            }
            if jpa_entity_count > 0 {
                framework_summary.push(format!("{}个JPA实体", jpa_entity_count));
            }
            if api_count > 0 {
                framework_summary.push(format!("{}个API端点", api_count));
            }

            summary.push_str(&framework_summary.join("、"));
        }

        // 添加变更类型摘要
        summary.push_str(&format!(
            "。共有{}个新增、{}个删除、{}个修改",
            additions, deletions, modifications
        ));

        summary
    }

    fn generate_rust_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        let _source_bytes = file_ast.source.as_bytes(); // Keep but mark as unused
        // 统计各类型节点数量
        let mut struct_count = 0;
        let mut enum_count = 0;
        let mut trait_count = 0;
        let mut impl_count = 0;
        let mut function_count = 0;
        let mut macro_count = 0;
        let mut test_count = 0;

        // 统计变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;
        let mut public_items = 0;

        for node in affected_nodes {
            match node.node_type.as_str() {
                "struct" | "debuggable_struct" => struct_count += 1,
                "enum" => enum_count += 1,
                "trait" => trait_count += 1,
                "impl" => impl_count += 1,
                "function" => function_count += 1,
                "macro" => macro_count += 1,
                "test_function" => test_count += 1,
                _ => {}
            }

            if node.is_public {
                public_items += 1;
            }

            if let Some(change_type) = &node.change_type {
                match change_type.as_str() {
                    "added" | "added_content" => additions += 1,
                    "deleted" => deletions += 1,
                    "modified" | "modified_with_deletion" => modifications += 1,
                    _ => {}
                }
            }
        }

        // 生成摘要
        let mut summary = format!("Rust文件 {} 变更分析: ", file_ast.path.display());

        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 添加结构变更摘要
        let mut structure_summary = Vec::new();
        if struct_count > 0 {
            structure_summary.push(format!("{}个结构体", struct_count));
        }
        if enum_count > 0 {
            structure_summary.push(format!("{}个枚举", enum_count));
        }
        if trait_count > 0 {
            structure_summary.push(format!("{}个特征", trait_count));
        }
        if impl_count > 0 {
            structure_summary.push(format!("{}个实现", impl_count));
        }
        if function_count > 0 {
            structure_summary.push(format!("{}个函数", function_count));
        }
        if macro_count > 0 {
            structure_summary.push(format!("{}个宏", macro_count));
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

        // 添加测试相关信息
        if test_count > 0 {
            summary.push_str(&format!("。包含{}个测试函数", test_count));
        }

        // 添加可见性信息
        if public_items > 0 {
            summary.push_str(&format!("。其中{}个为公开项", public_items));
        }

        // 添加变更类型摘要
        summary.push_str(&format!(
            "。共有{}个新增、{}个删除、{}个修改",
            additions, deletions, modifications
        ));

        summary
    }

    // 确定变更模式（FeatureImplementation, BugFix, Refactoring等）
    fn determine_change_pattern(
        &self,
        file_analyses: &[FileAnalysis],
        function_changes: usize,
        type_changes: usize,
        method_changes: usize,
        interface_changes: usize,
        java_changes: usize,
        rust_changes: usize,
        total_additions: usize,
        total_deletions: usize,
    ) -> ChangePattern {
        // 如果变更主要是Java特定的
        if java_changes > 0 && java_changes > rust_changes {
            // 检查是否有SpringBoot或JPA相关的变更
            let has_spring_changes = file_analyses.iter().any(|analysis| {
                analysis.affected_nodes.iter().any(|node| {
                    node.node_type == "spring_component" || node.node_type == "api_endpoint"
                })
            });

            let has_jpa_changes = file_analyses.iter().any(|analysis| {
                analysis
                    .affected_nodes
                    .iter()
                    .any(|node| node.node_type == "jpa_entity")
            });

            // Variables remain in local scope

            let has_visibility_changes = file_analyses.iter().any(|analysis| {
                analysis.affected_nodes.iter().any(|node| {
                    if let Some(content) = &node.content {
                        content.contains("public")
                            || content.contains("private")
                            || content.contains("protected")
                            || content.contains("package")
                    } else {
                        false
                    }
                })
            });

            if has_spring_changes || has_jpa_changes {
                return ChangePattern::LanguageSpecificChange("JavaStructuralChange".to_string());
            } else if has_visibility_changes {
                return ChangePattern::LanguageSpecificChange("JavaVisibilityChange".to_string());
            }
        }

        // 如果变更主要是Rust特定的
        if rust_changes > 0 && rust_changes > java_changes {
            // 检查特定的Rust变更模式
            let has_trait_impl = file_analyses.iter().any(|analysis| {
                analysis.affected_nodes.iter().any(|node| {
                    node.node_type == "trait"
                        || (node.node_type == "impl"
                            && node.content.as_ref().map_or(false, |c| c.contains("impl")))
                })
            });

            let has_macro_changes = file_analyses.iter().any(|analysis| {
                analysis
                    .affected_nodes
                    .iter()
                    .any(|node| node.node_type == "macro")
            });

            if has_trait_impl {
                return ChangePattern::LanguageSpecificChange(
                    "RustTraitImplementation".to_string(),
                );
            } else if has_macro_changes {
                return ChangePattern::LanguageSpecificChange("RustMacroChange".to_string());
            }
        }

        // 通用变更模式判断
        let config_changes = file_analyses.iter().any(|analysis| {
            analysis.path.to_string_lossy().contains("config")
                || analysis.path.to_string_lossy().ends_with(".properties")
                || analysis.path.to_string_lossy().ends_with(".yml")
                || analysis.path.to_string_lossy().ends_with(".toml")
        });

        if config_changes {
            return ChangePattern::ConfigurationChange;
        }

        let test_changes = file_analyses.iter().any(|analysis| {
            analysis.path.to_string_lossy().contains("test")
                || analysis
                    .affected_nodes
                    .iter()
                    .any(|node| node.node_type == "test_function")
        });

        if test_changes {
            return ChangePattern::BugFix; // 测试变更通常与bug修复相关
        }

        // 检查是否为bug修复
        let has_bug_fix_keywords = file_analyses.iter().any(|analysis| {
            analysis.affected_nodes.iter().any(|node| {
                if let Some(content) = &node.content {
                    content.to_lowercase().contains("fix")
                        || content.to_lowercase().contains("bug")
                        || content.to_lowercase().contains("issue")
                        || content.to_lowercase().contains("error")
                        || content.to_lowercase().contains("crash")
                        || content.to_lowercase().contains("exception")
                } else {
                    false
                }
            })
        });

        if has_bug_fix_keywords {
            return ChangePattern::BugFix;
        }

        // 检查是否为重构
        let has_refactor_keywords = file_analyses.iter().any(|analysis| {
            analysis.affected_nodes.iter().any(|node| {
                if let Some(content) = &node.content {
                    content.to_lowercase().contains("refactor")
                        || content.to_lowercase().contains("clean")
                        || content.to_lowercase().contains("improve")
                        || content.to_lowercase().contains("simplify")
                        || content.to_lowercase().contains("restructure")
                } else {
                    false
                }
            })
        });

        if has_refactor_keywords {
            return ChangePattern::Refactoring;
        }

        // 检查是否为模型/接口变更
        if type_changes > 0 || interface_changes > 0 {
            if method_changes > 0 {
                return ChangePattern::BehaviorChange; // 行为变更
            } else {
                return ChangePattern::ModelChange; // 模型变更
            }
        }

        // 检查是否为新功能实现
        if function_changes > 0
            && total_additions > 0
            && total_deletions > 0
            && total_additions > total_deletions * 2
        {
            return ChangePattern::FeatureImplementation;
        }

        // 默认为混合变更
        ChangePattern::MixedChange
    }

    // 确定变更范围(Minor, Moderate, Major)
    fn determine_change_scope(
        &self,
        total_affected_nodes: usize,
        total_additions: usize,
        total_deletions: usize,
        _total_modifications: usize,
        type_changes: usize,
        method_changes: usize,
        interface_changes: usize,
    ) -> ChangeScope {
        // 接口和类型变更是最严重的
        if interface_changes > 2 || type_changes > 5 {
            return ChangeScope::Major;
        }

        // 中等数量的变更
        if total_affected_nodes > 20 || total_additions + total_deletions > 50 || method_changes > 3
        {
            return ChangeScope::Moderate;
        }

        // 默认为轻微变更
        ChangeScope::Minor
    }

    fn analyze_generic_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = Vec::new();
        let query = self.queries.get(&file_ast.language_id).ok_or_else(|| {
            TreeSitterError::QueryError(format!(
                "No query found for language {}",
                file_ast.language_id
            ))
        })?;

        let source_bytes = file_ast.source.as_bytes();
        let tree_root = file_ast.tree.root_node();

        for hunk in hunks {
            // Determine the byte range of the hunk in the new file content
            // This requires careful mapping from line numbers to byte offsets.
            // For simplicity, let's assume new_range.start is 1-based line number.
            let hunk_start_line = hunk.new_range.start.saturating_sub(1); // 0-indexed
            let hunk_end_line = hunk_start_line + hunk.new_range.count;

            let mut hunk_start_byte = 0;
            let mut current_line = 0;
            for (i, byte) in source_bytes.iter().enumerate() {
                if current_line == hunk_start_line {
                    hunk_start_byte = i;
                    break;
                }
                if *byte == b'\n' {
                    current_line += 1;
                }
            }
            if current_line < hunk_start_line && hunk_start_line > 0 {
                // If hunk_start_line is 0, hunk_start_byte remains 0
                // Reached EOF before hunk start line, means hunk is likely beyond file end (should not happen in valid diff)
                warn!(
                    "Hunk start line {} is beyond file end for {:?}",
                    hunk_start_line + 1,
                    file_ast.path
                );
                continue;
            }

            let mut hunk_end_byte = source_bytes.len(); // Default to end of file
            current_line = 0; // Reset for end_byte calculation
            for (i, byte) in source_bytes.iter().enumerate() {
                if *byte == b'\n' {
                    current_line += 1;
                }
                if current_line == hunk_end_line {
                    // After processing the last line of the hunk
                    hunk_end_byte = i + 1; // Include the newline or the char itself
                    break;
                }
            }
            if current_line < hunk_end_line && hunk_end_line > 0 {
                // If hunk_end_line is 0, it means 0 lines, hunk_end_byte should be hunk_start_byte
                if hunk_end_line == 0 {
                    hunk_end_byte = hunk_start_byte;
                } else {
                    // otherwise, it means hunk extends to EOF
                    hunk_end_byte = source_bytes.len();
                }
            }

            // 分析每个 hunk 中的添加和删除操作
            let mut additions = Vec::new();
            let mut deletions = Vec::new();

            for line in &hunk.lines {
                if line.starts_with('+') {
                    additions.push(line.trim_start_matches('+').to_string());
                } else if line.starts_with('-') {
                    deletions.push(line.trim_start_matches('-').to_string());
                }
            }

            // 计算变更类型
            let change_operation = if !additions.is_empty() && !deletions.is_empty() {
                "modified"
            } else if !additions.is_empty() {
                "added"
            } else if !deletions.is_empty() {
                "deleted"
            } else {
                "unchanged"
            };

            let mut cursor = tree_sitter::QueryCursor::new();
            let matches = cursor.matches(query, tree_root, source_bytes);

            for m in matches {
                for capture in m.captures {
                    let node = capture.node;
                    let node_range = node.byte_range();

                    // Check if the node overlaps with the hunk's byte range
                    if node_range.start < hunk_end_byte && node_range.end > hunk_start_byte {
                        let node_name_capture = m
                            .captures
                            .iter()
                            .find(|c| query.capture_names()[c.index as usize].ends_with(".name"));

                        let name: String = node_name_capture
                            .map(|c| c.node.utf8_text(source_bytes).unwrap_or("").to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        let kind_capture_index = query.capture_names()[capture.index as usize]
                            .split('.')
                            .next()
                            .unwrap_or("unknown_type");

                        let start_line = node.start_position().row;
                        let end_line = node.end_position().row;

                        // 获取节点内容
                        let content = node.utf8_text(source_bytes).unwrap_or("").to_string();

                        // 确定变更的详细类型
                        let change_details = if content.contains(&additions.join("\n")) {
                            "added_content"
                        } else if !deletions.is_empty()
                            && deletions.iter().any(|d| content.contains(d))
                        {
                            "modified_with_deletion"
                        } else {
                            change_operation
                        };

                        affected_nodes.push(AffectedNode {
                            node_type: kind_capture_index.to_string(),
                            name,
                            range: (node_range.start, node_range.end),
                            is_public: self.is_node_public(&node, file_ast),
                            content: Some(content),
                            line_range: (start_line, end_line),
                            change_type: Some(change_details.to_string()),
                            additions: if !additions.is_empty() {
                                Some(additions.clone())
                            } else {
                                None
                            },
                            deletions: if !deletions.is_empty() {
                                Some(deletions.clone())
                            } else {
                                None
                            },
                        });
                    }
                }
            }
        }
        // Deduplicate affected_nodes if necessary (e.g. by node range and type)
        affected_nodes.sort_by_key(|n| (n.range.0, n.range.1, n.node_type.clone(), n.name.clone()));
        affected_nodes
            .dedup_by_key(|n| (n.range.0, n.range.1, n.node_type.clone(), n.name.clone()));
        Ok(affected_nodes)
    }

    #[allow(dead_code)]
    pub fn analyze_java_project_structure(
        &mut self,
        file_paths: &[PathBuf],
    ) -> Result<JavaProjectStructure, TreeSitterError> {
        let mut project_structure = JavaProjectStructure::new();
        for file_path in file_paths {
            match self.detect_language(file_path)? {
                Some(lang) if lang == "java" => {}
                _ => continue, // Skip non-Java files
            }
            let ast = self.parse_file(file_path)?;

            let package_name = match extract_java_package_name(&ast) {
                Ok(name) => name,
                Err(_) => "".to_string(),
            };

            let class_name = match extract_java_class_name(&ast) {
                Ok(name) => name,
                Err(_) => continue, // Skip if we can't extract class name
            };

            project_structure.add_class(&package_name, &class_name, file_path);

            match extract_java_imports(&ast) {
                Ok(imports) => {
                    for imp in imports {
                        project_structure.add_import(&package_name, &class_name, &imp);
                    }
                }
                Err(_) => {} // Continue even if imports can't be extracted
            }

            match extract_java_class_relations(&ast) {
                Ok(relations) => {
                    for rel in relations {
                        project_structure.add_relation(&package_name, &class_name, &rel);
                    }
                }
                Err(_) => {} // Continue even if relations can't be extracted
            }

            match extract_java_methods(&ast, self) {
                Ok(methods) => {
                    for method in methods {
                        project_structure.add_method(&package_name, &class_name, &method);
                    }
                }
                Err(_) => {} // Continue even if methods can't be extracted
            }

            // Simplified Spring Bean / JPA Entity detection
            // A real implementation would check for specific annotations on the class
            let source_code = &ast.source;
            if source_code.contains("@Entity") {
                // Very basic check
                project_structure.mark_as_jpa_entity(&package_name, &class_name);
            }
            if source_code.contains("@Component")
                || source_code.contains("@Service")
                || source_code.contains("@Repository")
            {
                // Basic check
                project_structure.mark_as_spring_bean(&package_name, &class_name);
            }
        }
        Ok(project_structure)
    }

    // generate_commit_prompt would also go here, likely calling analyze_diff first.
    pub async fn generate_commit_prompt(
        &mut self,
        diff_text: &str,
        config: &crate::config_management::settings::AppConfig,
    ) -> Result<String, AppError> {
        // 1. Analyze the diff
        let diff_analysis = self
            .analyze_diff(diff_text)
            .map_err(|e| AppError::TreeSitter(e))?;

        // 2. Construct a prompt based on the analysis
        // This is a simplified example. You'd want to be more sophisticated.
        let mut prompt_parts = Vec::new();
        prompt_parts.push(
            "Generate a concise and informative commit message for the following changes:"
                .to_string(),
        );
        prompt_parts.push(format!(
            "\nOverall summary: {}",
            diff_analysis.overall_summary
        ));

        if !diff_analysis.file_analyses.is_empty() {
            prompt_parts.push("\nKey changes per file:".to_string());
            for file_analysis in diff_analysis.file_analyses.iter().take(5) {
                // Limit for brevity
                prompt_parts.push(format!("- File: {}", file_analysis.path.display()));
                if let Some(summary) = &file_analysis.summary {
                    prompt_parts.push(format!("  Summary: {}", summary));
                }
                if !file_analysis.affected_nodes.is_empty() {
                    prompt_parts.push("  Affected elements:".to_string());
                    for node in file_analysis.affected_nodes.iter().take(3) {
                        // Limit for brevity
                        prompt_parts.push(format!(
                            "    - {} {} ({}public)",
                            node.node_type,
                            node.name,
                            if node.is_public { "" } else { "non-" }
                        ));
                    }
                }
            }
        }

        // Add context from AppConfig if needed (e.g., commit message conventions)
        if let Some(syntax_prompt) = config.prompts.get("commit-syntax") {
            prompt_parts.push(
                "\nAdhere to the following commit message syntax and conventions:".to_string(),
            );
            prompt_parts.push(syntax_prompt.clone());
        }

        // TODO: Integrate with AI model to generate the message
        // For now, just return the constructed prompt text itself for debugging or manual use.
        // In a real scenario, this prompt would be sent to an LLM.
        // Example:
        // let ai_response = ai_module::utils::call_ai_model(prompt_parts.join("\n"), &config.ai).await?;
        // Ok(ai_response.message_content)

        Ok(prompt_parts.join("\n"))
    }
}

// Errors specific to TreeSitterAnalyzer operations, distinct from general AppError
// Moved to error_handling module, but if some are specific to analyzer, they could be here.
// pub enum AnalyzerError {
//     InitializationError(String),
//     LanguageNotSupported(String),
//     ParseFailed(String),
//     QueryFailed(String),
// }
use crate::core::errors::AppError; // Ensure AppError is accessible
