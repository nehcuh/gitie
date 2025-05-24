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
        let mut python_changes = 0;
        let mut go_changes = 0;
        let mut c_changes = 0;
        let mut cpp_changes = 0;
        let mut js_changes = 0;

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
                                "python" => python_changes += 1,
                                "go" => go_changes += 1,
                                "c" => c_changes += 1,
                                "cpp" => cpp_changes += 1,
                                "js" => js_changes += 1,
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
                                        "python" => self.generate_python_file_summary(
                                            &file_ast,
                                            &affected_nodes,
                                        ),
                                        "go" => self
                                            .generate_go_file_summary(&file_ast, &affected_nodes),
                                        "js" => self
                                            .generate_js_file_summary(&file_ast, &affected_nodes),
                                        "c" => {
                                            self.generate_c_file_summary(&file_ast, &affected_nodes)
                                        }
                                        "cpp" => self
                                            .generate_cpp_file_summary(&file_ast, &affected_nodes),
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

    fn analyze_c_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

        for node in &mut affected_nodes {
            // 结构体
            if node.node_type == "struct" {
                node.node_type = "struct_definition".to_string();
                if let Some(content) = &node.content {
                    if content.contains("typedef struct") {
                        node.node_type = "typedef_struct".to_string();
                    }
                }
            }
            // 联合体
            if node.node_type == "union" {
                node.node_type = "union_definition".to_string();
                if let Some(content) = &node.content {
                    if content.contains("typedef union") {
                        node.node_type = "typedef_union".to_string();
                    }
                }
            }
            // 枚举
            if node.node_type == "enum" {
                node.node_type = "enum_definition".to_string();
                if let Some(content) = &node.content {
                    if content.contains("typedef enum") {
                        node.node_type = "typedef_enum".to_string();
                    }
                }
            }
            // Typedef
            if node.node_type == "typedef" {
                node.node_type = "typedef_declaration".to_string();
            }
            // 宏定义
            if node.node_type == "macro" {
                if let Some(content) = &node.content {
                    if content.contains("#ifdef")
                        || content.contains("#ifndef")
                        || content.contains("#if")
                    {
                        node.node_type = "conditional_macro".to_string();
                    } else if content.contains("#define") {
                        node.node_type = "define_macro".to_string();
                    }
                }
            }
            // 条件编译块
            if node.node_type == "preproc_if"
                || node.node_type == "preproc_elif"
                || node.node_type == "preproc_else"
            {
                node.node_type = "conditional_block".to_string();
            }
            // 函数定义
            if node.node_type == "function" {
                let mut tags = vec!["function".to_string()];
                if let Some(content) = &node.content {
                    if content.contains("inline") {
                        tags.push("inline".to_string());
                    }
                    if content.contains("static") {
                        tags.push("static".to_string());
                    }
                    if content.contains("extern") {
                        tags.push("extern".to_string());
                    }
                    // 检查主函数
                    if content.contains("main(") {
                        tags.push("main_function".to_string());
                    }
                    // 常见标准库API检测
                    let std_apis = [
                        "printf", "scanf", "malloc", "free", "memcpy", "strcpy", "strlen", "fopen",
                        "fclose", "fread", "fwrite", "fprintf", "fscanf", "exit", "system",
                        "getchar", "putchar", "puts", "gets", "perror", "abort",
                    ];
                    for api in &std_apis {
                        if content.contains(api) {
                            tags.push(format!("std_api:{}", api));
                        }
                    }
                }
                node.node_type = tags.join("|");
            }
            // 函数声明
            if node.node_type == "declaration" {
                if let Some(content) = &node.content {
                    // 检测是否为函数声明
                    if content.contains("(") && content.contains(");") {
                        node.node_type = "function_declaration".to_string();
                    }
                }
            }
            // 全局变量
            if node.node_type == "global_var" || node.node_type == "variable" {
                if let Some(content) = &node.content {
                    // 简单判断是否为全局变量
                    if !content.contains("static")
                        && !content.contains("const")
                        && !content.contains("(")
                    {
                        node.node_type = "global_variable".to_string();
                    } else if content.contains("static") {
                        node.node_type = "static_global_variable".to_string();
                    }
                }
            }
            // 指针类型声明
            if node.node_type == "pointer_declaration" {
                node.node_type = "pointer_type".to_string();
            }
            // 常量声明
            if node.node_type == "const_declaration" {
                node.node_type = "const_variable".to_string();
            }
            // 文件包含
            if node.node_type == "include" {
                node.node_type = "include_directive".to_string();
                if let Some(content) = &node.content {
                    if content.contains("<stdio.h>") {
                        node.node_type = "include_stdio".to_string();
                    } else if content.contains("<stdlib.h>") {
                        node.node_type = "include_stdlib".to_string();
                    } else if content.contains("<string.h>") {
                        node.node_type = "include_string".to_string();
                    }
                    // ...可根据需要继续添加常见头文件
                }
            }
            // 注释块
            if node.node_type == "comment" {
                node.node_type = "comment_block".to_string();
            }
            // goto语句
            if node.node_type == "goto_statement" {
                node.node_type = "goto_statement".to_string();
            }
            // switch语句
            if node.node_type == "switch_statement" {
                node.node_type = "switch_statement".to_string();
            }
            // case语句
            if node.node_type == "case_statement" {
                node.node_type = "case_statement".to_string();
            }
            // for, while, do-while
            if node.node_type == "for_statement" {
                node.node_type = "for_loop".to_string();
            }
            if node.node_type == "while_statement" {
                node.node_type = "while_loop".to_string();
            }
            if node.node_type == "do_statement" {
                node.node_type = "do_while_loop".to_string();
            }
            // break/continue
            if node.node_type == "break_statement" {
                node.node_type = "break_statement".to_string();
            }
            if node.node_type == "continue_statement" {
                node.node_type = "continue_statement".to_string();
            }
            // return语句
            if node.node_type == "return_statement" {
                node.node_type = "return_statement".to_string();
            }
        }
        Ok(affected_nodes)
    }

    fn analyze_python_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

        for node in &mut affected_nodes {
            // 类定义
            if node.node_type == "class" {
                node.node_type = "class_definition".to_string();
                if let Some(content) = &node.content {
                    // 判断是否继承
                    if content.contains("(") && content.contains("):") {
                        node.node_type = "derived_class".to_string();
                    }
                    // Django Model
                    if content.contains("(models.Model)") {
                        node.node_type = "django_model".to_string();
                    }
                }
            }
            // 函数/方法定义
            if node.node_type == "function" {
                let mut tags = vec!["function".to_string()];
                if let Some(content) = &node.content {
                    if content.contains("@staticmethod") {
                        tags.push("staticmethod".to_string());
                    }
                    if content.contains("@classmethod") {
                        tags.push("classmethod".to_string());
                    }
                    if content.contains("@property") {
                        tags.push("property".to_string());
                    }
                    if content.contains("def __init__") {
                        tags.push("constructor".to_string());
                    }
                    // 常用标准库API
                    let std_apis = [
                        "os.",
                        "sys.",
                        "re.",
                        "json.",
                        "logging.",
                        "subprocess.",
                        "threading.",
                        "asyncio.",
                        "datetime.",
                        "open(",
                        "print(",
                        "len(",
                        "range(",
                        "map(",
                        "zip(",
                        "enumerate(",
                        "filter(",
                        "reduce(",
                        "list(",
                        "dict(",
                        "set(",
                        "tuple(",
                    ];
                    for api in &std_apis {
                        if content.contains(api) {
                            tags.push(format!("std_api:{}", api));
                        }
                    }
                }
                node.node_type = tags.join("|");
            }
            // 装饰器
            if node.node_type == "decorator" {
                node.node_type = "decorator".to_string();
            }
            // import 导入
            if node.node_type == "import_statement" {
                node.node_type = "import_statement".to_string();
                if let Some(content) = &node.content {
                    // 常见三方库
                    let libs = [
                        "numpy",
                        "pandas",
                        "torch",
                        "tensorflow",
                        "sklearn",
                        "requests",
                        "flask",
                        "django",
                        "matplotlib",
                    ];
                    for lib in &libs {
                        if content.contains(lib) {
                            node.node_type = format!("import_{}", lib);
                        }
                    }
                }
            }
            // from ... import ...
            if node.node_type == "import_from_statement" {
                node.node_type = "import_from_statement".to_string();
            }
            // 变量
            if node.node_type == "assignment" {
                node.node_type = "assignment".to_string();
            }
            // lambda
            if node.node_type == "lambda" {
                node.node_type = "lambda_expression".to_string();
            }
            // 注释
            if node.node_type == "comment" {
                node.node_type = "comment_block".to_string();
            }
            // if/elif/else/for/while/try/except/finally/with
            if node.node_type == "if_statement" {
                node.node_type = "if_statement".to_string();
            }
            if node.node_type == "elif_clause" {
                node.node_type = "elif_clause".to_string();
            }
            if node.node_type == "else_clause" {
                node.node_type = "else_clause".to_string();
            }
            if node.node_type == "for_statement" {
                node.node_type = "for_loop".to_string();
            }
            if node.node_type == "while_statement" {
                node.node_type = "while_loop".to_string();
            }
            if node.node_type == "try_statement" {
                node.node_type = "try_statement".to_string();
            }
            if node.node_type == "except_clause" {
                node.node_type = "except_clause".to_string();
            }
            if node.node_type == "finally_clause" {
                node.node_type = "finally_clause".to_string();
            }
            if node.node_type == "with_statement" {
                node.node_type = "with_statement".to_string();
            }
            // return/yield
            if node.node_type == "return_statement" {
                node.node_type = "return_statement".to_string();
            }
            if node.node_type == "yield" {
                node.node_type = "yield_statement".to_string();
            }
            // assert
            if node.node_type == "assert_statement" {
                node.node_type = "assert_statement".to_string();
            }
            // 异步
            if node.node_type == "async_function" {
                node.node_type = "async_function".to_string();
            }
            if node.node_type == "await" {
                node.node_type = "await_expression".to_string();
            }
        }
        Ok(affected_nodes)
    }

    fn analyze_go_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

        for node in &mut affected_nodes {
            // 包声明
            if node.node_type == "package_clause" {
                node.node_type = "package_declaration".to_string();
            }
            // import
            if node.node_type == "import_declaration" {
                node.node_type = "import_declaration".to_string();
                if let Some(content) = &node.content {
                    // 常用包
                    let pkgs = [
                        "\"fmt\"",
                        "\"os\"",
                        "\"io\"",
                        "\"bufio\"",
                        "\"net\"",
                        "\"http\"",
                        "\"json\"",
                        "\"time\"",
                        "\"context\"",
                        "\"sync\"",
                    ];
                    for pkg in &pkgs {
                        if content.contains(pkg) {
                            node.node_type = format!("import_{}", pkg.replace("\"", ""));
                        }
                    }
                }
            }
            // 结构体
            if node.node_type == "type_spec" {
                if let Some(content) = &node.content {
                    if content.contains("struct {") {
                        node.node_type = "struct_definition".to_string();
                    } else if content.contains("interface {") {
                        node.node_type = "interface_definition".to_string();
                    } else if content.contains("type ") && content.contains("func(") {
                        node.node_type = "type_function_alias".to_string();
                    } else {
                        node.node_type = "type_alias".to_string();
                    }
                }
            }
            // 函数定义
            if node.node_type == "function_declaration" {
                let mut tags = vec!["function".to_string()];
                if let Some(content) = &node.content {
                    if content.contains("func (") {
                        tags.push("method".to_string());
                    }
                    if content.contains("go ") {
                        tags.push("goroutine".to_string());
                    }
                    if content.contains("defer ") {
                        tags.push("defer".to_string());
                    }
                    // 常用标准库API
                    let std_apis = [
                        "fmt.", "os.", "io.", "bufio.", "net.", "http.", "json.", "time.",
                        "context.", "sync.",
                    ];
                    for api in &std_apis {
                        if content.contains(api) {
                            tags.push(format!("std_api:{}", api));
                        }
                    }
                }
                node.node_type = tags.join("|");
            }
            // 变量声明
            if node.node_type == "var_declaration" {
                node.node_type = "var_declaration".to_string();
                if let Some(content) = &node.content {
                    if content.contains(":=") {
                        node.node_type = "short_var_declaration".to_string();
                    }
                }
            }
            if node.node_type == "const_declaration" {
                node.node_type = "const_declaration".to_string();
            }
            // 接口
            if node.node_type == "interface_type" {
                node.node_type = "interface_definition".to_string();
            }
            // 方法接收者
            if node.node_type == "method_declaration" {
                node.node_type = "method_declaration".to_string();
            }
            // 类型断言
            if node.node_type == "type_assertion_expression" {
                node.node_type = "type_assertion".to_string();
            }
            // select/goroutine
            if node.node_type == "go_statement" {
                node.node_type = "go_statement".to_string();
            }
            if node.node_type == "select_statement" {
                node.node_type = "select_statement".to_string();
            }
            if node.node_type == "defer_statement" {
                node.node_type = "defer_statement".to_string();
            }
            if node.node_type == "channel_type" {
                node.node_type = "channel_type".to_string();
            }
            // for/range/if/switch/case
            if node.node_type == "for_statement" {
                node.node_type = "for_loop".to_string();
            }
            if node.node_type == "range_clause" {
                node.node_type = "range_clause".to_string();
            }
            if node.node_type == "if_statement" {
                node.node_type = "if_statement".to_string();
            }
            if node.node_type == "switch_statement" {
                node.node_type = "switch_statement".to_string();
            }
            if node.node_type == "case_clause" {
                node.node_type = "case_clause".to_string();
            }
            // return/break/continue
            if node.node_type == "return_statement" {
                node.node_type = "return_statement".to_string();
            }
            if node.node_type == "break_statement" {
                node.node_type = "break_statement".to_string();
            }
            if node.node_type == "continue_statement" {
                node.node_type = "continue_statement".to_string();
            }
            // 注释
            if node.node_type == "comment" {
                node.node_type = "comment_block".to_string();
            }
        }
        Ok(affected_nodes)
    }

    fn analyze_javascript_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

        for node in &mut affected_nodes {
            // 函数声明/表达式/箭头函数
            if node.node_type == "function_declaration" {
                node.node_type = "function_declaration".to_string();
            }
            if node.node_type == "function_expression" {
                node.node_type = "function_expression".to_string();
            }
            if node.node_type == "arrow_function" {
                node.node_type = "arrow_function".to_string();
            }
            // 类
            if node.node_type == "class_declaration" {
                node.node_type = "class_definition".to_string();
                if let Some(content) = &node.content {
                    // 判断是否继承
                    if content.contains("extends ") {
                        node.node_type = "derived_class".to_string();
                    }
                }
            }
            // 方法
            if node.node_type == "method_definition" {
                let mut tags = vec!["method".to_string()];
                if let Some(content) = &node.content {
                    if content.contains("static ") {
                        tags.push("static".to_string());
                    }
                    if content.contains("async ") {
                        tags.push("async".to_string());
                    }
                }
                node.node_type = tags.join("|");
            }
            // 变量声明
            if node.node_type == "variable_declaration" {
                if let Some(content) = &node.content {
                    if content.contains("const ") {
                        node.node_type = "const_variable".to_string();
                    } else if content.contains("let ") {
                        node.node_type = "let_variable".to_string();
                    } else if content.contains("var ") {
                        node.node_type = "var_variable".to_string();
                    }
                }
            }
            // import/export
            if node.node_type == "import_declaration" {
                node.node_type = "import_statement".to_string();
                if let Some(content) = &node.content {
                    // 常见库
                    let libs = [
                        "react", "vue", "angular", "lodash", "moment", "axios", "express", "next",
                        "redux",
                    ];
                    for lib in &libs {
                        if content.contains(lib) {
                            node.node_type = format!("import_{}", lib);
                        }
                    }
                }
            }
            if node.node_type == "export_statement" {
                node.node_type = "export_statement".to_string();
            }
            // require
            if node.node_type == "call_expression" {
                if let Some(content) = &node.content {
                    if content.contains("require(") {
                        node.node_type = "require_call".to_string();
                    }
                    // 常用标准库API或全局对象
                    let std_apis = [
                        "console.",
                        "setTimeout",
                        "setInterval",
                        "clearTimeout",
                        "clearInterval",
                        "process.",
                        "Buffer",
                        "fs.",
                        "path.",
                        "http.",
                        "fetch",
                        "document.",
                        "window.",
                    ];
                    for api in &std_apis {
                        if content.contains(api) {
                            node.node_type = format!("std_api:{}", api);
                        }
                    }
                }
            }
            // 异步特性
            if node.node_type == "await_expression" {
                node.node_type = "await_expression".to_string();
            }
            if node.node_type == "async_function" {
                node.node_type = "async_function".to_string();
            }
            // Promise
            if node.node_type == "new_expression" {
                if let Some(content) = &node.content {
                    if content.contains("Promise(") {
                        node.node_type = "promise_creation".to_string();
                    }
                }
            }
            // try/catch/finally
            if node.node_type == "try_statement" {
                node.node_type = "try_statement".to_string();
            }
            if node.node_type == "catch_clause" {
                node.node_type = "catch_clause".to_string();
            }
            if node.node_type == "finally_clause" {
                node.node_type = "finally_clause".to_string();
            }
            // if/else/for/while/switch/case/break/continue/return
            if node.node_type == "if_statement" {
                node.node_type = "if_statement".to_string();
            }
            if node.node_type == "else_clause" {
                node.node_type = "else_clause".to_string();
            }
            if node.node_type == "for_statement" {
                node.node_type = "for_loop".to_string();
            }
            if node.node_type == "while_statement" {
                node.node_type = "while_loop".to_string();
            }
            if node.node_type == "do_statement" {
                node.node_type = "do_while_loop".to_string();
            }
            if node.node_type == "switch_statement" {
                node.node_type = "switch_statement".to_string();
            }
            if node.node_type == "case_clause" {
                node.node_type = "case_clause".to_string();
            }
            if node.node_type == "break_statement" {
                node.node_type = "break_statement".to_string();
            }
            if node.node_type == "continue_statement" {
                node.node_type = "continue_statement".to_string();
            }
            if node.node_type == "return_statement" {
                node.node_type = "return_statement".to_string();
            }
            // 注释
            if node.node_type == "comment" {
                node.node_type = "comment_block".to_string();
            }
            // Lambda/匿名函数
            if node.node_type == "arrow_function" {
                node.node_type = "arrow_function".to_string();
            }
            // 对象/数组解构
            if node.node_type == "object_pattern" {
                node.node_type = "object_destructuring".to_string();
            }
            if node.node_type == "array_pattern" {
                node.node_type = "array_destructuring".to_string();
            }
            // 模板字符串
            if node.node_type == "template_string" {
                node.node_type = "template_literal".to_string();
            }
            // 其它可根据需求扩展
        }
        Ok(affected_nodes)
    }

    fn analyze_cpp_file_changes(
        &self,
        file_ast: &FileAst,
        hunks: &[DiffHunk],
    ) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = self.analyze_generic_file_changes(file_ast, hunks)?;

        for node in &mut affected_nodes {
            // 类与结构体
            if node.node_type == "class" {
                node.node_type = "class_definition".to_string();
                if let Some(content) = &node.content {
                    // 判断是否模板类
                    if content.contains("template") {
                        node.node_type = "template_class".to_string();
                    }
                    // 判断是否继承
                    if content.contains(": public")
                        || content.contains(": protected")
                        || content.contains(": private")
                    {
                        node.node_type = "derived_class".to_string();
                    }
                }
            }
            if node.node_type == "struct" {
                node.node_type = "struct_definition".to_string();
                if let Some(content) = &node.content {
                    if content.contains("template") {
                        node.node_type = "template_struct".to_string();
                    }
                }
            }
            // 联合体
            if node.node_type == "union" {
                node.node_type = "union_definition".to_string();
            }
            // 枚举
            if node.node_type == "enum" {
                node.node_type = "enum_definition".to_string();
                if let Some(content) = &node.content {
                    if content.contains("class") {
                        node.node_type = "enum_class".to_string();
                    }
                }
            }
            // 命名空间
            if node.node_type == "namespace" {
                node.node_type = "namespace_block".to_string();
            }
            // 模板函数/声明
            if node.node_type == "function" {
                let mut tags = vec!["function".to_string()];
                if let Some(content) = &node.content {
                    if content.contains("template") {
                        tags.push("template".to_string());
                    }
                    if content.contains("constexpr") {
                        tags.push("constexpr".to_string());
                    }
                    if content.contains("virtual") {
                        tags.push("virtual".to_string());
                    }
                    if content.contains("override") {
                        tags.push("override".to_string());
                    }
                    if content.contains("static") {
                        tags.push("static".to_string());
                    }
                    if content.contains("inline") {
                        tags.push("inline".to_string());
                    }
                    if content.contains("explicit") {
                        tags.push("explicit".to_string());
                    }
                    if content.contains("operator") {
                        tags.push("operator_overload".to_string());
                    }
                    // Lambda 检测
                    if content.contains("[](")
                        || content.contains("[&](")
                        || content.contains("[=](")
                    {
                        tags.push("lambda".to_string());
                    }
                    // 构造/析构函数
                    if content.contains("~") {
                        tags.push("destructor".to_string());
                    } else {
                        // 构造函数名称和类名一致
                        if content.contains(&node.name) {
                            tags.push("constructor".to_string());
                        }
                    }

                    // 常用C++标准库API检测
                    let std_apis = [
                        "std::vector",
                        "std::string",
                        "std::map",
                        "std::unordered_map",
                        "std::set",
                        "std::unique_ptr",
                        "std::shared_ptr",
                        "std::make_shared",
                        "std::thread",
                        "std::async",
                        "std::function",
                        "std::move",
                        "std::swap",
                        "std::sort",
                        "std::cout",
                        "std::cin",
                        "std::cerr",
                        "std::endl",
                    ];
                    for api in &std_apis {
                        if content.contains(api) {
                            tags.push(format!("std_api:{}", api));
                        }
                    }
                }
                node.node_type = tags.join("|");
            }
            // Lambda 表达式（有的 AST 可能单独标记）
            if node.node_type == "lambda_expression" {
                node.node_type = "lambda_expression".to_string();
            }
            // 操作符重载
            if node.node_type == "operator_function" {
                node.node_type = "operator_overload".to_string();
            }
            // 友元
            if node.node_type == "friend" {
                node.node_type = "friend_declaration".to_string();
            }
            // Typedef/Using
            if node.node_type == "typedef" {
                node.node_type = "typedef_declaration".to_string();
            }
            if node.node_type == "using_declaration" {
                node.node_type = "using_declaration".to_string();
            }
            // 宏
            if node.node_type == "macro" {
                if let Some(content) = &node.content {
                    if content.contains("#define") {
                        node.node_type = "define_macro".to_string();
                    } else if content.contains("#ifdef")
                        || content.contains("#ifndef")
                        || content.contains("#if")
                    {
                        node.node_type = "conditional_macro".to_string();
                    }
                }
            }
            // 条件编译块
            if node.node_type == "preproc_if"
                || node.node_type == "preproc_elif"
                || node.node_type == "preproc_else"
            {
                node.node_type = "conditional_block".to_string();
            }
            // include
            if node.node_type == "include" {
                node.node_type = "include_directive".to_string();
                if let Some(content) = &node.content {
                    if content.contains("<vector>") {
                        node.node_type = "include_vector".to_string();
                    } else if content.contains("<string>") {
                        node.node_type = "include_string".to_string();
                    } else if content.contains("<map>") {
                        node.node_type = "include_map".to_string();
                    } else if content.contains("<memory>") {
                        node.node_type = "include_memory".to_string();
                    } else if content.contains("<thread>") {
                        node.node_type = "include_thread".to_string();
                    }
                    // 可继续添加常用头文件
                }
            }
            // 注释
            if node.node_type == "comment" {
                node.node_type = "comment_block".to_string();
            }
            // 全局变量
            if node.node_type == "global_var" || node.node_type == "variable" {
                if let Some(content) = &node.content {
                    if !content.contains("static")
                        && !content.contains("const")
                        && !content.contains("(")
                    {
                        node.node_type = "global_variable".to_string();
                    } else if content.contains("static") {
                        node.node_type = "static_global_variable".to_string();
                    }
                }
            }
            // 常量声明
            if node.node_type == "const_declaration" {
                node.node_type = "const_variable".to_string();
            }
            // Goto, switch, case, loop, break, continue, return
            if node.node_type == "goto_statement" {
                node.node_type = "goto_statement".to_string();
            }
            if node.node_type == "switch_statement" {
                node.node_type = "switch_statement".to_string();
            }
            if node.node_type == "case_statement" {
                node.node_type = "case_statement".to_string();
            }
            if node.node_type == "for_statement" {
                node.node_type = "for_loop".to_string();
            }
            if node.node_type == "while_statement" {
                node.node_type = "while_loop".to_string();
            }
            if node.node_type == "do_statement" {
                node.node_type = "do_while_loop".to_string();
            }
            if node.node_type == "break_statement" {
                node.node_type = "break_statement".to_string();
            }
            if node.node_type == "continue_statement" {
                node.node_type = "continue_statement".to_string();
            }
            if node.node_type == "return_statement" {
                node.node_type = "return_statement".to_string();
            }
            // 其它可以根据需求继续扩展
        }

        Ok(affected_nodes)
    }

    fn generate_java_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        // 统计各类型节点数量
        let mut class_count = 0;
        let mut method_count = 0;
        let mut field_count = 0;
        let mut api_count = 0;
        let mut spring_component_count = 0;
        let mut jpa_entity_count = 0;
        let mut annotation_interface_count = 0;
        let mut test_method_count = 0;
        let mut injected_field_count = 0;
        let mut constructor_count = 0;

        // 统计变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;

        // 其他未知类型
        let mut other_types: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for node in affected_nodes {
            match node.node_type.as_str() {
                "class_structure" => class_count += 1,
                "spring_component" => spring_component_count += 1,
                "jpa_entity" => jpa_entity_count += 1,
                "api_endpoint" => api_count += 1,
                "annotation_interface" => annotation_interface_count += 1,
                "test_method" => test_method_count += 1,
                "method" | "overridden_method" => method_count += 1,
                "constructor" => constructor_count += 1,
                "field" => field_count += 1,
                "injected_field" => injected_field_count += 1,
                // 统计其他类型
                other => *other_types.entry(other.to_string()).or_insert(0) += 1,
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

        let mut summary = format!("Java文件 {} 变更分析：", file_ast.path.display());
        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 添加结构变更摘要
        let mut structure_summary = Vec::new();
        if class_count > 0 {
            structure_summary.push(format!("{}个类", class_count));
        }
        if spring_component_count > 0 {
            structure_summary.push(format!("{}个Spring组件", spring_component_count));
        }
        if jpa_entity_count > 0 {
            structure_summary.push(format!("{}个JPA实体", jpa_entity_count));
        }
        if annotation_interface_count > 0 {
            structure_summary.push(format!("{}个注解接口", annotation_interface_count));
        }
        if api_count > 0 {
            structure_summary.push(format!("{}个API端点", api_count));
        }
        if test_method_count > 0 {
            structure_summary.push(format!("{}个测试方法", test_method_count));
        }
        if constructor_count > 0 {
            structure_summary.push(format!("{}个构造函数", constructor_count));
        }
        if injected_field_count > 0 {
            structure_summary.push(format!("{}个依赖注入字段", injected_field_count));
        }
        if method_count > 0 {
            structure_summary.push(format!("{}个方法", method_count));
        }
        if field_count > 0 {
            structure_summary.push(format!("{}个字段", field_count));
        }
        // 汇总其他类型
        for (ty, cnt) in other_types {
            if cnt > 0 && ty != "unknown" {
                structure_summary.push(format!("{}个{}", cnt, ty));
            }
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
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
        // 各类型节点数量
        let mut struct_count = 0;
        let mut debuggable_struct_count = 0;
        let mut enum_count = 0;
        let mut debuggable_enum_count = 0;
        let mut trait_count = 0;
        let mut trait_impl_count = 0;
        let mut inherent_impl_count = 0;
        let mut function_count = 0;
        let mut public_function_count = 0;
        let mut async_function_count = 0;
        let mut unsafe_function_count = 0;
        let mut test_function_count = 0;
        let mut macro_definition_count = 0;
        let mut macro_invocation_count = 0;
        let mut constant_count = 0;
        let mut static_count = 0;
        let mut type_alias_count = 0;
        let mut attribute_item_count = 0;
        let mut public_items = 0;

        // 变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;

        let mut other_types: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for node in affected_nodes {
            match node.node_type.as_str() {
                "struct_definition" => struct_count += 1,
                "debuggable_struct" => debuggable_struct_count += 1,
                "enum_definition" => enum_count += 1,
                "debuggable_enum" => debuggable_enum_count += 1,
                "trait_definition" => trait_count += 1,
                "trait_impl" => trait_impl_count += 1,
                "inherent_impl" => inherent_impl_count += 1,
                "function_definition" => function_count += 1,
                "public_function" => public_function_count += 1,
                "async_function" => async_function_count += 1,
                "unsafe_function" => unsafe_function_count += 1,
                "test_function" => test_function_count += 1,
                "macro_definition" => macro_definition_count += 1,
                "macro_invocation" => macro_invocation_count += 1,
                "constant_definition" => constant_count += 1,
                "static_definition" => static_count += 1,
                "type_alias_definition" => type_alias_count += 1,
                "attribute_item" => attribute_item_count += 1,
                _ => *other_types.entry(node.node_type.clone()).or_insert(0) += 1,
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

        let mut summary = format!("Rust文件 {} 变更分析：", file_ast.path.display());
        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 结构变更摘要
        let mut structure_summary = Vec::new();
        if struct_count > 0 {
            structure_summary.push(format!("{}个结构体", struct_count));
        }
        if debuggable_struct_count > 0 {
            structure_summary.push(format!("{}个可调试结构体", debuggable_struct_count));
        }
        if enum_count > 0 {
            structure_summary.push(format!("{}个枚举", enum_count));
        }
        if debuggable_enum_count > 0 {
            structure_summary.push(format!("{}个可调试枚举", debuggable_enum_count));
        }
        if trait_count > 0 {
            structure_summary.push(format!("{}个Trait", trait_count));
        }
        if trait_impl_count > 0 {
            structure_summary.push(format!("{}个Trait实现", trait_impl_count));
        }
        if inherent_impl_count > 0 {
            structure_summary.push(format!("{}个固有impl实现", inherent_impl_count));
        }
        if function_count > 0 {
            structure_summary.push(format!("{}个普通函数", function_count));
        }
        if public_function_count > 0 {
            structure_summary.push(format!("{}个公开函数", public_function_count));
        }
        if async_function_count > 0 {
            structure_summary.push(format!("{}个异步函数", async_function_count));
        }
        if unsafe_function_count > 0 {
            structure_summary.push(format!("{}个unsafe函数", unsafe_function_count));
        }
        if macro_definition_count > 0 {
            structure_summary.push(format!("{}个宏定义", macro_definition_count));
        }
        if macro_invocation_count > 0 {
            structure_summary.push(format!("{}个宏调用", macro_invocation_count));
        }
        if constant_count > 0 {
            structure_summary.push(format!("{}个常量", constant_count));
        }
        if static_count > 0 {
            structure_summary.push(format!("{}个静态变量", static_count));
        }
        if type_alias_count > 0 {
            structure_summary.push(format!("{}个类型别名", type_alias_count));
        }
        if attribute_item_count > 0 {
            structure_summary.push(format!("{}个属性标记", attribute_item_count));
        }
        for (ty, cnt) in other_types {
            if cnt > 0 && ty != "unknown" {
                structure_summary.push(format!("{}个{}", cnt, ty));
            }
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

        // 添加测试相关
        if test_function_count > 0 {
            summary.push_str(&format!("。包含{}个测试函数", test_function_count));
        }

        // 可见性
        if public_items > 0 {
            summary.push_str(&format!("。其中{}个为公开项", public_items));
        }

        // 变更类型摘要
        summary.push_str(&format!(
            "。共有{}个新增、{}个删除、{}个修改",
            additions, deletions, modifications
        ));

        summary
    }

    fn generate_c_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        // 各类型节点统计
        let mut struct_count = 0;
        let mut typedef_struct_count = 0;
        let mut union_count = 0;
        let mut typedef_union_count = 0;
        let mut enum_count = 0;
        let mut typedef_enum_count = 0;
        let mut function_count = 0;
        let mut function_declaration_count = 0;
        let mut main_function_count = 0;
        let mut global_var_count = 0;
        let mut static_global_var_count = 0;
        let mut const_var_count = 0;
        let mut pointer_type_count = 0;
        let mut macro_definition_count = 0;
        let mut conditional_macro_count = 0;
        let mut define_macro_count = 0;
        let mut include_count = 0;
        let mut include_stdio_count = 0;
        let mut include_stdlib_count = 0;
        let mut include_string_count = 0;
        let mut conditional_block_count = 0;
        let mut comment_block_count = 0;
        let mut goto_statement_count = 0;
        let mut switch_statement_count = 0;
        let mut case_statement_count = 0;
        let mut for_loop_count = 0;
        let mut while_loop_count = 0;
        let mut do_while_loop_count = 0;
        let mut break_statement_count = 0;
        let mut continue_statement_count = 0;
        let mut return_statement_count = 0;

        // 变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;

        let mut other_types: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for node in affected_nodes {
            match node.node_type.as_str() {
                "struct_definition" => struct_count += 1,
                "typedef_struct" => typedef_struct_count += 1,
                "union_definition" => union_count += 1,
                "typedef_union" => typedef_union_count += 1,
                "enum_definition" => enum_count += 1,
                "typedef_enum" => typedef_enum_count += 1,
                t if t.contains("function|main_function") => {
                    function_count += 1;
                    main_function_count += 1;
                }
                "function" => function_count += 1,
                "function_declaration" => function_declaration_count += 1,
                "main_function" => main_function_count += 1,
                "global_variable" => global_var_count += 1,
                "static_global_variable" => static_global_var_count += 1,
                "const_variable" => const_var_count += 1,
                "pointer_type" => pointer_type_count += 1,
                "macro_definition" => macro_definition_count += 1,
                "define_macro" => define_macro_count += 1,
                "conditional_macro" => conditional_macro_count += 1,
                "include_directive" => include_count += 1,
                "include_stdio" => include_stdio_count += 1,
                "include_stdlib" => include_stdlib_count += 1,
                "include_string" => include_string_count += 1,
                "conditional_block" => conditional_block_count += 1,
                "comment_block" => comment_block_count += 1,
                "goto_statement" => goto_statement_count += 1,
                "switch_statement" => switch_statement_count += 1,
                "case_statement" => case_statement_count += 1,
                "for_loop" => for_loop_count += 1,
                "while_loop" => while_loop_count += 1,
                "do_while_loop" => do_while_loop_count += 1,
                "break_statement" => break_statement_count += 1,
                "continue_statement" => continue_statement_count += 1,
                "return_statement" => return_statement_count += 1,
                _ => *other_types.entry(node.node_type.clone()).or_insert(0) += 1,
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

        let mut summary = format!("C文件 {} 变更分析：", file_ast.path.display());
        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 结构摘要
        let mut structure_summary = Vec::new();
        if struct_count > 0 {
            structure_summary.push(format!("{}个结构体", struct_count));
        }
        if typedef_struct_count > 0 {
            structure_summary.push(format!("{}个typedef结构体", typedef_struct_count));
        }
        if union_count > 0 {
            structure_summary.push(format!("{}个联合体", union_count));
        }
        if typedef_union_count > 0 {
            structure_summary.push(format!("{}个typedef联合体", typedef_union_count));
        }
        if enum_count > 0 {
            structure_summary.push(format!("{}个枚举", enum_count));
        }
        if typedef_enum_count > 0 {
            structure_summary.push(format!("{}个typedef枚举", typedef_enum_count));
        }
        if function_count > 0 {
            structure_summary.push(format!("{}个函数", function_count));
        }
        if main_function_count > 0 {
            structure_summary.push(format!("{}个主函数", main_function_count));
        }
        if function_declaration_count > 0 {
            structure_summary.push(format!("{}个函数声明", function_declaration_count));
        }
        if global_var_count > 0 {
            structure_summary.push(format!("{}个全局变量", global_var_count));
        }
        if static_global_var_count > 0 {
            structure_summary.push(format!("{}个静态全局变量", static_global_var_count));
        }
        if const_var_count > 0 {
            structure_summary.push(format!("{}个常量", const_var_count));
        }
        if pointer_type_count > 0 {
            structure_summary.push(format!("{}个指针类型", pointer_type_count));
        }
        if macro_definition_count > 0 {
            structure_summary.push(format!("{}个宏定义", macro_definition_count));
        }
        if define_macro_count > 0 {
            structure_summary.push(format!("{}个#define宏", define_macro_count));
        }
        if conditional_macro_count > 0 {
            structure_summary.push(format!("{}个条件宏", conditional_macro_count));
        }
        if include_count > 0 {
            structure_summary.push(format!("{}个包含指令", include_count));
        }
        if include_stdio_count > 0 {
            structure_summary.push(format!("{}个stdio头文件", include_stdio_count));
        }
        if include_stdlib_count > 0 {
            structure_summary.push(format!("{}个stdlib头文件", include_stdlib_count));
        }
        if include_string_count > 0 {
            structure_summary.push(format!("{}个string头文件", include_string_count));
        }
        if conditional_block_count > 0 {
            structure_summary.push(format!("{}个条件编译块", conditional_block_count));
        }
        if comment_block_count > 0 {
            structure_summary.push(format!("{}个注释块", comment_block_count));
        }
        if goto_statement_count > 0 {
            structure_summary.push(format!("{}个goto语句", goto_statement_count));
        }
        if switch_statement_count > 0 {
            structure_summary.push(format!("{}个switch语句", switch_statement_count));
        }
        if case_statement_count > 0 {
            structure_summary.push(format!("{}个case语句", case_statement_count));
        }
        if for_loop_count > 0 {
            structure_summary.push(format!("{}个for循环", for_loop_count));
        }
        if while_loop_count > 0 {
            structure_summary.push(format!("{}个while循环", while_loop_count));
        }
        if do_while_loop_count > 0 {
            structure_summary.push(format!("{}个do-while循环", do_while_loop_count));
        }
        if break_statement_count > 0 {
            structure_summary.push(format!("{}个break语句", break_statement_count));
        }
        if continue_statement_count > 0 {
            structure_summary.push(format!("{}个continue语句", continue_statement_count));
        }
        if return_statement_count > 0 {
            structure_summary.push(format!("{}个return语句", return_statement_count));
        }

        // 其他类型
        for (ty, cnt) in other_types {
            if cnt > 0 && ty != "unknown" {
                structure_summary.push(format!("{}个{}", cnt, ty));
            }
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

        summary.push_str(&format!(
            "。共有{}个新增、{}个删除、{}个修改",
            additions, deletions, modifications
        ));

        summary
    }

    fn generate_cpp_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        // 类型统计
        let mut class_count = 0;
        let mut template_class_count = 0;
        let mut derived_class_count = 0;
        let mut struct_count = 0;
        let mut template_struct_count = 0;
        let mut union_count = 0;
        let mut enum_count = 0;
        let mut enum_class_count = 0;
        let mut namespace_count = 0;
        let mut function_count = 0;
        let mut template_function_count = 0;
        let mut constexpr_function_count = 0;
        let mut virtual_function_count = 0;
        let mut override_function_count = 0;
        let mut static_function_count = 0;
        let mut inline_function_count = 0;
        let mut explicit_function_count = 0;
        let mut operator_overload_count = 0;
        let mut lambda_count = 0;
        let mut constructor_count = 0;
        let mut destructor_count = 0;
        let mut macro_definition_count = 0;
        let mut conditional_macro_count = 0;
        let mut define_macro_count = 0;
        let mut typedef_declaration_count = 0;
        let mut using_declaration_count = 0;
        let mut friend_declaration_count = 0;
        let mut include_count = 0;
        let mut include_vector_count = 0;
        let mut include_string_count = 0;
        let mut include_map_count = 0;
        let mut include_memory_count = 0;
        let mut include_thread_count = 0;
        let mut global_var_count = 0;
        let mut static_global_var_count = 0;
        let mut const_var_count = 0;
        let mut conditional_block_count = 0;
        let mut comment_block_count = 0;
        let mut for_loop_count = 0;
        let mut while_loop_count = 0;
        let mut do_while_loop_count = 0;
        let mut switch_statement_count = 0;
        let mut case_statement_count = 0;
        let mut break_statement_count = 0;
        let mut continue_statement_count = 0;
        let mut return_statement_count = 0;
        let mut goto_statement_count = 0;

        // 变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;
        let mut other_types: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for node in affected_nodes {
            match node.node_type.as_str() {
                "class_definition" => class_count += 1,
                "template_class" => template_class_count += 1,
                "derived_class" => derived_class_count += 1,
                "struct_definition" => struct_count += 1,
                "template_struct" => template_struct_count += 1,
                "union_definition" => union_count += 1,
                "enum_definition" => enum_count += 1,
                "enum_class" => enum_class_count += 1,
                "namespace_block" => namespace_count += 1,
                t if t.contains("function|template") => {
                    function_count += 1;
                    template_function_count += 1;
                }
                "function" => function_count += 1,
                "template" => template_function_count += 1,
                "constexpr" => constexpr_function_count += 1,
                "virtual" => virtual_function_count += 1,
                "override" => override_function_count += 1,
                "static" => static_function_count += 1,
                "inline" => inline_function_count += 1,
                "explicit" => explicit_function_count += 1,
                "operator_overload" => operator_overload_count += 1,
                "lambda" | "lambda_expression" => lambda_count += 1,
                "constructor" => constructor_count += 1,
                "destructor" => destructor_count += 1,
                "macro_definition" | "define_macro" => macro_definition_count += 1,
                "conditional_macro" => conditional_macro_count += 1,
                "typedef_declaration" => typedef_declaration_count += 1,
                "using_declaration" => using_declaration_count += 1,
                "friend_declaration" => friend_declaration_count += 1,
                "include_directive" => include_count += 1,
                "include_vector" => include_vector_count += 1,
                "include_string" => include_string_count += 1,
                "include_map" => include_map_count += 1,
                "include_memory" => include_memory_count += 1,
                "include_thread" => include_thread_count += 1,
                "global_variable" => global_var_count += 1,
                "static_global_variable" => static_global_var_count += 1,
                "const_variable" => const_var_count += 1,
                "conditional_block" => conditional_block_count += 1,
                "comment_block" => comment_block_count += 1,
                "for_loop" => for_loop_count += 1,
                "while_loop" => while_loop_count += 1,
                "do_while_loop" => do_while_loop_count += 1,
                "switch_statement" => switch_statement_count += 1,
                "case_statement" => case_statement_count += 1,
                "break_statement" => break_statement_count += 1,
                "continue_statement" => continue_statement_count += 1,
                "return_statement" => return_statement_count += 1,
                "goto_statement" => goto_statement_count += 1,
                _ => *other_types.entry(node.node_type.clone()).or_insert(0) += 1,
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

        let mut summary = format!("C++文件 {} 变更分析：", file_ast.path.display());
        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 结构摘要
        let mut structure_summary = Vec::new();
        if class_count > 0 {
            structure_summary.push(format!("{}个类", class_count));
        }
        if template_class_count > 0 {
            structure_summary.push(format!("{}个模板类", template_class_count));
        }
        if derived_class_count > 0 {
            structure_summary.push(format!("{}个派生类", derived_class_count));
        }
        if struct_count > 0 {
            structure_summary.push(format!("{}个结构体", struct_count));
        }
        if template_struct_count > 0 {
            structure_summary.push(format!("{}个模板结构体", template_struct_count));
        }
        if union_count > 0 {
            structure_summary.push(format!("{}个联合体", union_count));
        }
        if enum_count > 0 {
            structure_summary.push(format!("{}个枚举", enum_count));
        }
        if enum_class_count > 0 {
            structure_summary.push(format!("{}个枚举类", enum_class_count));
        }
        if namespace_count > 0 {
            structure_summary.push(format!("{}个命名空间", namespace_count));
        }
        if function_count > 0 {
            structure_summary.push(format!("{}个函数", function_count));
        }
        if template_function_count > 0 {
            structure_summary.push(format!("{}个模板函数", template_function_count));
        }
        if constexpr_function_count > 0 {
            structure_summary.push(format!("{}个constexpr函数", constexpr_function_count));
        }
        if virtual_function_count > 0 {
            structure_summary.push(format!("{}个虚函数", virtual_function_count));
        }
        if override_function_count > 0 {
            structure_summary.push(format!("{}个override函数", override_function_count));
        }
        if static_function_count > 0 {
            structure_summary.push(format!("{}个静态函数", static_function_count));
        }
        if inline_function_count > 0 {
            structure_summary.push(format!("{}个内联函数", inline_function_count));
        }
        if explicit_function_count > 0 {
            structure_summary.push(format!("{}个explicit函数", explicit_function_count));
        }
        if operator_overload_count > 0 {
            structure_summary.push(format!("{}个操作符重载", operator_overload_count));
        }
        if lambda_count > 0 {
            structure_summary.push(format!("{}个lambda表达式", lambda_count));
        }
        if constructor_count > 0 {
            structure_summary.push(format!("{}个构造函数", constructor_count));
        }
        if destructor_count > 0 {
            structure_summary.push(format!("{}个析构函数", destructor_count));
        }
        if macro_definition_count > 0 {
            structure_summary.push(format!("{}个宏定义", macro_definition_count));
        }
        if conditional_macro_count > 0 {
            structure_summary.push(format!("{}个条件宏", conditional_macro_count));
        }
        if typedef_declaration_count > 0 {
            structure_summary.push(format!("{}个typedef声明", typedef_declaration_count));
        }
        if using_declaration_count > 0 {
            structure_summary.push(format!("{}个using声明", using_declaration_count));
        }
        if friend_declaration_count > 0 {
            structure_summary.push(format!("{}个友元声明", friend_declaration_count));
        }
        if include_count > 0 {
            structure_summary.push(format!("{}个包含指令", include_count));
        }
        if include_vector_count > 0 {
            structure_summary.push(format!("{}个vector头文件", include_vector_count));
        }
        if include_string_count > 0 {
            structure_summary.push(format!("{}个string头文件", include_string_count));
        }
        if include_map_count > 0 {
            structure_summary.push(format!("{}个map头文件", include_map_count));
        }
        if include_memory_count > 0 {
            structure_summary.push(format!("{}个memory头文件", include_memory_count));
        }
        if include_thread_count > 0 {
            structure_summary.push(format!("{}个thread头文件", include_thread_count));
        }
        if global_var_count > 0 {
            structure_summary.push(format!("{}个全局变量", global_var_count));
        }
        if static_global_var_count > 0 {
            structure_summary.push(format!("{}个静态全局变量", static_global_var_count));
        }
        if const_var_count > 0 {
            structure_summary.push(format!("{}个常量", const_var_count));
        }
        if conditional_block_count > 0 {
            structure_summary.push(format!("{}个条件编译块", conditional_block_count));
        }
        if comment_block_count > 0 {
            structure_summary.push(format!("{}个注释块", comment_block_count));
        }
        if for_loop_count > 0 {
            structure_summary.push(format!("{}个for循环", for_loop_count));
        }
        if while_loop_count > 0 {
            structure_summary.push(format!("{}个while循环", while_loop_count));
        }
        if do_while_loop_count > 0 {
            structure_summary.push(format!("{}个do-while循环", do_while_loop_count));
        }
        if switch_statement_count > 0 {
            structure_summary.push(format!("{}个switch语句", switch_statement_count));
        }
        if case_statement_count > 0 {
            structure_summary.push(format!("{}个case语句", case_statement_count));
        }
        if break_statement_count > 0 {
            structure_summary.push(format!("{}个break语句", break_statement_count));
        }
        if continue_statement_count > 0 {
            structure_summary.push(format!("{}个continue语句", continue_statement_count));
        }
        if return_statement_count > 0 {
            structure_summary.push(format!("{}个return语句", return_statement_count));
        }
        if goto_statement_count > 0 {
            structure_summary.push(format!("{}个goto语句", goto_statement_count));
        }
        // 其他类型
        for (ty, cnt) in other_types {
            if cnt > 0 && ty != "unknown" {
                structure_summary.push(format!("{}个{}", cnt, ty));
            }
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

        summary.push_str(&format!(
            "。共有{}个新增、{}个删除、{}个修改",
            additions, deletions, modifications
        ));

        summary
    }

    fn generate_python_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        // 类型统计
        let mut class_def_count = 0;
        let mut derived_class_count = 0;
        let mut django_model_count = 0;
        let mut function_count = 0;
        let mut static_method_count = 0;
        let mut class_method_count = 0;
        let mut property_method_count = 0;
        let mut constructor_count = 0;
        let mut test_function_count = 0;
        let mut std_api_count = 0;
        let mut decorator_count = 0;
        let mut import_count = 0;
        let mut import_numpy_count = 0;
        let mut import_pandas_count = 0;
        let mut import_torch_count = 0;
        let mut import_tensorflow_count = 0;
        let mut import_sklearn_count = 0;
        let mut import_requests_count = 0;
        let mut import_flask_count = 0;
        let mut import_django_count = 0;
        let mut import_matplotlib_count = 0;
        let mut import_from_count = 0;
        let mut assignment_count = 0;
        let mut lambda_count = 0;
        let mut comment_block_count = 0;
        let mut if_count = 0;
        let mut elif_count = 0;
        let mut else_count = 0;
        let mut for_count = 0;
        let mut while_count = 0;
        let mut try_count = 0;
        let mut except_count = 0;
        let mut finally_count = 0;
        let mut with_count = 0;
        let mut return_count = 0;
        let mut yield_count = 0;
        let mut assert_count = 0;
        let mut async_function_count = 0;
        let mut await_count = 0;

        // 变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;
        let mut other_types: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for node in affected_nodes {
            match node.node_type.as_str() {
                "class_definition" => class_def_count += 1,
                "derived_class" => derived_class_count += 1,
                "django_model" => django_model_count += 1,
                t if t.contains("function|staticmethod") => {
                    function_count += 1;
                    static_method_count += 1;
                }
                t if t.contains("function|classmethod") => {
                    function_count += 1;
                    class_method_count += 1;
                }
                t if t.contains("function|property") => {
                    function_count += 1;
                    property_method_count += 1;
                }
                t if t.contains("function|constructor") => {
                    function_count += 1;
                    constructor_count += 1;
                }
                t if t.contains("function|test_function") => {
                    function_count += 1;
                    test_function_count += 1;
                }
                t if t.contains("function|std_api") => {
                    function_count += 1;
                    std_api_count += 1;
                }
                "function" => function_count += 1,
                "staticmethod" => static_method_count += 1,
                "classmethod" => class_method_count += 1,
                "property" => property_method_count += 1,
                "constructor" => constructor_count += 1,
                "test_function" => test_function_count += 1,
                t if t.starts_with("import_numpy") => import_numpy_count += 1,
                t if t.starts_with("import_pandas") => import_pandas_count += 1,
                t if t.starts_with("import_torch") => import_torch_count += 1,
                t if t.starts_with("import_tensorflow") => import_tensorflow_count += 1,
                t if t.starts_with("import_sklearn") => import_sklearn_count += 1,
                t if t.starts_with("import_requests") => import_requests_count += 1,
                t if t.starts_with("import_flask") => import_flask_count += 1,
                t if t.starts_with("import_django") => import_django_count += 1,
                t if t.starts_with("import_matplotlib") => import_matplotlib_count += 1,
                "import_statement" => import_count += 1,
                "import_from_statement" => import_from_count += 1,
                "decorator" => decorator_count += 1,
                "assignment" => assignment_count += 1,
                "lambda_expression" => lambda_count += 1,
                "comment_block" => comment_block_count += 1,
                "if_statement" => if_count += 1,
                "elif_clause" => elif_count += 1,
                "else_clause" => else_count += 1,
                "for_loop" => for_count += 1,
                "while_loop" => while_count += 1,
                "try_statement" => try_count += 1,
                "except_clause" => except_count += 1,
                "finally_clause" => finally_count += 1,
                "with_statement" => with_count += 1,
                "return_statement" => return_count += 1,
                "yield_statement" => yield_count += 1,
                "assert_statement" => assert_count += 1,
                "async_function" => async_function_count += 1,
                "await_expression" => await_count += 1,
                _ => *other_types.entry(node.node_type.clone()).or_insert(0) += 1,
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

        let mut summary = format!("Python文件 {} 变更分析：", file_ast.path.display());
        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 结构摘要
        let mut structure_summary = Vec::new();
        if class_def_count > 0 {
            structure_summary.push(format!("{}个类", class_def_count));
        }
        if derived_class_count > 0 {
            structure_summary.push(format!("{}个派生类", derived_class_count));
        }
        if django_model_count > 0 {
            structure_summary.push(format!("{}个Django模型", django_model_count));
        }
        if function_count > 0 {
            structure_summary.push(format!("{}个函数", function_count));
        }
        if static_method_count > 0 {
            structure_summary.push(format!("{}个静态方法", static_method_count));
        }
        if class_method_count > 0 {
            structure_summary.push(format!("{}个类方法", class_method_count));
        }
        if property_method_count > 0 {
            structure_summary.push(format!("{}个属性方法", property_method_count));
        }
        if constructor_count > 0 {
            structure_summary.push(format!("{}个构造函数", constructor_count));
        }
        if test_function_count > 0 {
            structure_summary.push(format!("{}个测试函数", test_function_count));
        }
        if std_api_count > 0 {
            structure_summary.push(format!("{}个标准库API调用", std_api_count));
        }
        if decorator_count > 0 {
            structure_summary.push(format!("{}个装饰器", decorator_count));
        }
        if import_count > 0 {
            structure_summary.push(format!("{}个import导入", import_count));
        }
        if import_from_count > 0 {
            structure_summary.push(format!("{}个from导入", import_from_count));
        }
        if import_numpy_count > 0 {
            structure_summary.push(format!("{}次numpy导入", import_numpy_count));
        }
        if import_pandas_count > 0 {
            structure_summary.push(format!("{}次pandas导入", import_pandas_count));
        }
        if import_torch_count > 0 {
            structure_summary.push(format!("{}次torch导入", import_torch_count));
        }
        if import_tensorflow_count > 0 {
            structure_summary.push(format!("{}次tensorflow导入", import_tensorflow_count));
        }
        if import_sklearn_count > 0 {
            structure_summary.push(format!("{}次sklearn导入", import_sklearn_count));
        }
        if import_requests_count > 0 {
            structure_summary.push(format!("{}次requests导入", import_requests_count));
        }
        if import_flask_count > 0 {
            structure_summary.push(format!("{}次flask导入", import_flask_count));
        }
        if import_django_count > 0 {
            structure_summary.push(format!("{}次django导入", import_django_count));
        }
        if import_matplotlib_count > 0 {
            structure_summary.push(format!("{}次matplotlib导入", import_matplotlib_count));
        }
        if assignment_count > 0 {
            structure_summary.push(format!("{}个赋值", assignment_count));
        }
        if lambda_count > 0 {
            structure_summary.push(format!("{}个lambda表达式", lambda_count));
        }
        if comment_block_count > 0 {
            structure_summary.push(format!("{}个注释块", comment_block_count));
        }
        if if_count > 0 {
            structure_summary.push(format!("{}个if语句", if_count));
        }
        if elif_count > 0 {
            structure_summary.push(format!("{}个elif语句", elif_count));
        }
        if else_count > 0 {
            structure_summary.push(format!("{}个else语句", else_count));
        }
        if for_count > 0 {
            structure_summary.push(format!("{}个for循环", for_count));
        }
        if while_count > 0 {
            structure_summary.push(format!("{}个while循环", while_count));
        }
        if try_count > 0 {
            structure_summary.push(format!("{}个try语句", try_count));
        }
        if except_count > 0 {
            structure_summary.push(format!("{}个except语句", except_count));
        }
        if finally_count > 0 {
            structure_summary.push(format!("{}个finally语句", finally_count));
        }
        if with_count > 0 {
            structure_summary.push(format!("{}个with语句", with_count));
        }
        if return_count > 0 {
            structure_summary.push(format!("{}个return语句", return_count));
        }
        if yield_count > 0 {
            structure_summary.push(format!("{}个yield语句", yield_count));
        }
        if assert_count > 0 {
            structure_summary.push(format!("{}个assert语句", assert_count));
        }
        if async_function_count > 0 {
            structure_summary.push(format!("{}个异步函数", async_function_count));
        }
        if await_count > 0 {
            structure_summary.push(format!("{}个await表达式", await_count));
        }

        for (ty, cnt) in other_types {
            if cnt > 0 && ty != "unknown" {
                structure_summary.push(format!("{}个{}", cnt, ty));
            }
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

        summary.push_str(&format!(
            "。共有{}个新增、{}个删除、{}个修改",
            additions, deletions, modifications
        ));

        summary
    }

    fn generate_go_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        // 类型统计
        let mut package_count = 0;
        let mut import_count = 0;
        let mut import_fmt_count = 0;
        let mut import_os_count = 0;
        let mut import_io_count = 0;
        let mut import_bufio_count = 0;
        let mut import_net_count = 0;
        let mut import_http_count = 0;
        let mut import_json_count = 0;
        let mut import_time_count = 0;
        let mut import_context_count = 0;
        let mut import_sync_count = 0;
        let mut struct_def_count = 0;
        let mut interface_def_count = 0;
        let mut type_alias_count = 0;
        let mut type_function_alias_count = 0;
        let mut function_count = 0;
        let mut method_count = 0;
        let mut goroutine_count = 0;
        let mut defer_count = 0;
        let mut var_decl_count = 0;
        let mut short_var_decl_count = 0;
        let mut const_decl_count = 0;
        let mut channel_type_count = 0;
        let mut range_clause_count = 0;
        let mut go_statement_count = 0;
        let mut select_statement_count = 0;
        let mut defer_statement_count = 0;
        let mut for_loop_count = 0;
        let mut if_count = 0;
        let mut switch_count = 0;
        let mut case_count = 0;
        let mut return_count = 0;
        let mut break_count = 0;
        let mut continue_count = 0;
        let mut comment_block_count = 0;

        // 变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;
        let mut other_types: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for node in affected_nodes {
            match node.node_type.as_str() {
                "package_declaration" => package_count += 1,
                "import_declaration" => import_count += 1,
                t if t.starts_with("import_fmt") => import_fmt_count += 1,
                t if t.starts_with("import_os") => import_os_count += 1,
                t if t.starts_with("import_io") => import_io_count += 1,
                t if t.starts_with("import_bufio") => import_bufio_count += 1,
                t if t.starts_with("import_net") => import_net_count += 1,
                t if t.starts_with("import_http") => import_http_count += 1,
                t if t.starts_with("import_json") => import_json_count += 1,
                t if t.starts_with("import_time") => import_time_count += 1,
                t if t.starts_with("import_context") => import_context_count += 1,
                t if t.starts_with("import_sync") => import_sync_count += 1,
                "struct_definition" => struct_def_count += 1,
                "interface_definition" => interface_def_count += 1,
                "type_alias" => type_alias_count += 1,
                "type_function_alias" => type_function_alias_count += 1,
                t if t.contains("function|method") => {
                    function_count += 1;
                    method_count += 1;
                }
                "function" => function_count += 1,
                "method" => method_count += 1,
                "goroutine" => goroutine_count += 1,
                "defer" => defer_count += 1,
                "var_declaration" => var_decl_count += 1,
                "short_var_declaration" => short_var_decl_count += 1,
                "const_declaration" => const_decl_count += 1,
                "channel_type" => channel_type_count += 1,
                "range_clause" => range_clause_count += 1,
                "go_statement" => go_statement_count += 1,
                "select_statement" => select_statement_count += 1,
                "defer_statement" => defer_statement_count += 1,
                "for_loop" => for_loop_count += 1,
                "if_statement" => if_count += 1,
                "switch_statement" => switch_count += 1,
                "case_clause" => case_count += 1,
                "return_statement" => return_count += 1,
                "break_statement" => break_count += 1,
                "continue_statement" => continue_count += 1,
                "comment_block" => comment_block_count += 1,
                _ => *other_types.entry(node.node_type.clone()).or_insert(0) += 1,
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

        let mut summary = format!("Go文件 {} 变更分析：", file_ast.path.display());
        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 结构摘要
        let mut structure_summary = Vec::new();
        if package_count > 0 {
            structure_summary.push(format!("{}个包声明", package_count));
        }
        if import_count > 0 {
            structure_summary.push(format!("{}个import导入", import_count));
        }
        if import_fmt_count > 0 {
            structure_summary.push(format!("{}次fmt导入", import_fmt_count));
        }
        if import_os_count > 0 {
            structure_summary.push(format!("{}次os导入", import_os_count));
        }
        if import_io_count > 0 {
            structure_summary.push(format!("{}次io导入", import_io_count));
        }
        if import_bufio_count > 0 {
            structure_summary.push(format!("{}次bufio导入", import_bufio_count));
        }
        if import_net_count > 0 {
            structure_summary.push(format!("{}次net导入", import_net_count));
        }
        if import_http_count > 0 {
            structure_summary.push(format!("{}次http导入", import_http_count));
        }
        if import_json_count > 0 {
            structure_summary.push(format!("{}次json导入", import_json_count));
        }
        if import_time_count > 0 {
            structure_summary.push(format!("{}次time导入", import_time_count));
        }
        if import_context_count > 0 {
            structure_summary.push(format!("{}次context导入", import_context_count));
        }
        if import_sync_count > 0 {
            structure_summary.push(format!("{}次sync导入", import_sync_count));
        }
        if struct_def_count > 0 {
            structure_summary.push(format!("{}个结构体", struct_def_count));
        }
        if interface_def_count > 0 {
            structure_summary.push(format!("{}个接口", interface_def_count));
        }
        if type_alias_count > 0 {
            structure_summary.push(format!("{}个类型别名", type_alias_count));
        }
        if type_function_alias_count > 0 {
            structure_summary.push(format!("{}个函数类型别名", type_function_alias_count));
        }
        if function_count > 0 {
            structure_summary.push(format!("{}个函数", function_count));
        }
        if method_count > 0 {
            structure_summary.push(format!("{}个方法", method_count));
        }
        if goroutine_count > 0 {
            structure_summary.push(format!("{}个goroutine", goroutine_count));
        }
        if defer_count > 0 {
            structure_summary.push(format!("{}次defer", defer_count));
        }
        if var_decl_count > 0 {
            structure_summary.push(format!("{}个变量声明", var_decl_count));
        }
        if short_var_decl_count > 0 {
            structure_summary.push(format!("{}个短变量声明", short_var_decl_count));
        }
        if const_decl_count > 0 {
            structure_summary.push(format!("{}个常量声明", const_decl_count));
        }
        if channel_type_count > 0 {
            structure_summary.push(format!("{}个通道类型", channel_type_count));
        }
        if range_clause_count > 0 {
            structure_summary.push(format!("{}个range子句", range_clause_count));
        }
        if go_statement_count > 0 {
            structure_summary.push(format!("{}个go语句", go_statement_count));
        }
        if select_statement_count > 0 {
            structure_summary.push(format!("{}个select语句", select_statement_count));
        }
        if defer_statement_count > 0 {
            structure_summary.push(format!("{}个defer语句", defer_statement_count));
        }
        if for_loop_count > 0 {
            structure_summary.push(format!("{}个for循环", for_loop_count));
        }
        if if_count > 0 {
            structure_summary.push(format!("{}个if语句", if_count));
        }
        if switch_count > 0 {
            structure_summary.push(format!("{}个switch语句", switch_count));
        }
        if case_count > 0 {
            structure_summary.push(format!("{}个case语句", case_count));
        }
        if return_count > 0 {
            structure_summary.push(format!("{}个return语句", return_count));
        }
        if break_count > 0 {
            structure_summary.push(format!("{}个break语句", break_count));
        }
        if continue_count > 0 {
            structure_summary.push(format!("{}个continue语句", continue_count));
        }
        if comment_block_count > 0 {
            structure_summary.push(format!("{}个注释块", comment_block_count));
        }
        for (ty, cnt) in other_types {
            if cnt > 0 && ty != "unknown" {
                structure_summary.push(format!("{}个{}", cnt, ty));
            }
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

        summary.push_str(&format!(
            "。共有{}个新增、{}个删除、{}个修改",
            additions, deletions, modifications
        ));

        summary
    }

    fn generate_js_file_summary(
        &self,
        file_ast: &FileAst,
        affected_nodes: &[AffectedNode],
    ) -> String {
        // 类型统计
        let mut function_decl_count = 0;
        let mut function_expr_count = 0;
        let mut arrow_function_count = 0;
        let mut class_count = 0;
        let mut derived_class_count = 0;
        let mut method_count = 0;
        let mut static_method_count = 0;
        let mut async_method_count = 0;
        let mut variable_const_count = 0;
        let mut variable_let_count = 0;
        let mut variable_var_count = 0;
        let mut import_count = 0;
        let mut export_count = 0;
        let mut require_count = 0;
        let mut promise_count = 0;
        let mut std_api_count = 0;
        let mut await_count = 0;
        let mut async_function_count = 0;
        let mut template_literal_count = 0;
        let mut object_destructuring_count = 0;
        let mut array_destructuring_count = 0;
        let mut try_count = 0;
        let mut catch_count = 0;
        let mut finally_count = 0;
        let mut if_count = 0;
        let mut else_count = 0;
        let mut for_count = 0;
        let mut while_count = 0;
        let mut do_while_count = 0;
        let mut switch_count = 0;
        let mut case_count = 0;
        let mut break_count = 0;
        let mut continue_count = 0;
        let mut return_count = 0;
        let mut comment_block_count = 0;

        // 变更类型
        let mut additions = 0;
        let mut deletions = 0;
        let mut modifications = 0;
        let mut other_types: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for node in affected_nodes {
            match node.node_type.as_str() {
                "function_declaration" => function_decl_count += 1,
                "function_expression" => function_expr_count += 1,
                "arrow_function" => arrow_function_count += 1,
                "class_definition" => class_count += 1,
                "derived_class" => derived_class_count += 1,
                t if t.starts_with("method") => method_count += 1,
                t if t.contains("static") => static_method_count += 1,
                t if t.contains("async") && !t.contains("function") => async_method_count += 1,
                "const_variable" => variable_const_count += 1,
                "let_variable" => variable_let_count += 1,
                "var_variable" => variable_var_count += 1,
                "import_statement" => import_count += 1,
                "export_statement" => export_count += 1,
                "require_call" => require_count += 1,
                t if t.starts_with("promise") => promise_count += 1,
                t if t.starts_with("std_api") => std_api_count += 1,
                "await_expression" => await_count += 1,
                "async_function" => async_function_count += 1,
                "template_literal" => template_literal_count += 1,
                "object_destructuring" => object_destructuring_count += 1,
                "array_destructuring" => array_destructuring_count += 1,
                "try_statement" => try_count += 1,
                "catch_clause" => catch_count += 1,
                "finally_clause" => finally_count += 1,
                "if_statement" => if_count += 1,
                "else_clause" => else_count += 1,
                "for_loop" => for_count += 1,
                "while_loop" => while_count += 1,
                "do_while_loop" => do_while_count += 1,
                "switch_statement" => switch_count += 1,
                "case_clause" => case_count += 1,
                "break_statement" => break_count += 1,
                "continue_statement" => continue_count += 1,
                "return_statement" => return_count += 1,
                "comment_block" => comment_block_count += 1,
                _ => *other_types.entry(node.node_type.clone()).or_insert(0) += 1,
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

        let mut summary = format!("JavaScript文件 {} 变更分析：", file_ast.path.display());
        if affected_nodes.is_empty() {
            return format!("{}未检测到结构性变更", summary);
        }

        // 结构摘要
        let mut structure_summary = Vec::new();
        if function_decl_count > 0 {
            structure_summary.push(format!("{}个函数声明", function_decl_count));
        }
        if function_expr_count > 0 {
            structure_summary.push(format!("{}个函数表达式", function_expr_count));
        }
        if arrow_function_count > 0 {
            structure_summary.push(format!("{}个箭头函数", arrow_function_count));
        }
        if class_count > 0 {
            structure_summary.push(format!("{}个类", class_count));
        }
        if derived_class_count > 0 {
            structure_summary.push(format!("{}个派生类", derived_class_count));
        }
        if method_count > 0 {
            structure_summary.push(format!("{}个方法", method_count));
        }
        if static_method_count > 0 {
            structure_summary.push(format!("{}个静态方法", static_method_count));
        }
        if async_method_count > 0 {
            structure_summary.push(format!("{}个异步方法", async_method_count));
        }
        if variable_const_count > 0 {
            structure_summary.push(format!("{}个const变量", variable_const_count));
        }
        if variable_let_count > 0 {
            structure_summary.push(format!("{}个let变量", variable_let_count));
        }
        if variable_var_count > 0 {
            structure_summary.push(format!("{}个var变量", variable_var_count));
        }
        if import_count > 0 {
            structure_summary.push(format!("{}个import语句", import_count));
        }
        if export_count > 0 {
            structure_summary.push(format!("{}个export语句", export_count));
        }
        if require_count > 0 {
            structure_summary.push(format!("{}个require调用", require_count));
        }
        if promise_count > 0 {
            structure_summary.push(format!("{}个Promise创建", promise_count));
        }
        if std_api_count > 0 {
            structure_summary.push(format!("{}个标准库API调用", std_api_count));
        }
        if await_count > 0 {
            structure_summary.push(format!("{}个await表达式", await_count));
        }
        if async_function_count > 0 {
            structure_summary.push(format!("{}个异步函数", async_function_count));
        }
        if template_literal_count > 0 {
            structure_summary.push(format!("{}个模板字符串", template_literal_count));
        }
        if object_destructuring_count > 0 {
            structure_summary.push(format!("{}个对象解构", object_destructuring_count));
        }
        if array_destructuring_count > 0 {
            structure_summary.push(format!("{}个数组解构", array_destructuring_count));
        }
        if try_count > 0 {
            structure_summary.push(format!("{}个try语句", try_count));
        }
        if catch_count > 0 {
            structure_summary.push(format!("{}个catch语句", catch_count));
        }
        if finally_count > 0 {
            structure_summary.push(format!("{}个finally语句", finally_count));
        }
        if if_count > 0 {
            structure_summary.push(format!("{}个if语句", if_count));
        }
        if else_count > 0 {
            structure_summary.push(format!("{}个else语句", else_count));
        }
        if for_count > 0 {
            structure_summary.push(format!("{}个for循环", for_count));
        }
        if while_count > 0 {
            structure_summary.push(format!("{}个while循环", while_count));
        }
        if do_while_count > 0 {
            structure_summary.push(format!("{}个do-while循环", do_while_count));
        }
        if switch_count > 0 {
            structure_summary.push(format!("{}个switch语句", switch_count));
        }
        if case_count > 0 {
            structure_summary.push(format!("{}个case语句", case_count));
        }
        if break_count > 0 {
            structure_summary.push(format!("{}个break语句", break_count));
        }
        if continue_count > 0 {
            structure_summary.push(format!("{}个continue语句", continue_count));
        }
        if return_count > 0 {
            structure_summary.push(format!("{}个return语句", return_count));
        }
        if comment_block_count > 0 {
            structure_summary.push(format!("{}个注释块", comment_block_count));
        }
        for (ty, cnt) in other_types {
            if cnt > 0 && ty != "unknown" {
                structure_summary.push(format!("{}个{}", cnt, ty));
            }
        }

        if !structure_summary.is_empty() {
            summary.push_str(&format!("影响了{}", structure_summary.join("、")));
        }

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
