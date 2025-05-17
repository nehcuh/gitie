// src/tree_sitter_analyzer/analyzer.rs
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tree_sitter::{Parser, Query, Language}; // Added Language here
use tracing::{debug, error, warn};

use crate::config_management::settings::TreeSitterConfig;
use crate::core::errors::TreeSitterError; // Updated path
use super::core::{ // Assuming core.rs will export these
    FileAst, DiffHunk, ChangeType, 
    FileAnalysis, DiffAnalysis, AffectedNode,
    ChangeAnalysis,
    get_tree_sitter_rust, get_tree_sitter_java, 
    get_tree_sitter_python, get_tree_sitter_go,
    JavaProjectStructure, // Now imported from core instead of java
    // calculate_hash, // Assuming this will be in core or a utils.rs
    // parse_git_diff, // Assuming this will be in core or a utils.rs
};
use super::java::{ // Import functions from java.rs module
    extract_java_package_name, extract_java_imports, extract_java_class_name,
    extract_java_class_relations, extract_java_methods,
};
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

    fn initialize_languages(&mut self) -> Result<(), TreeSitterError> {
        // Load languages based on config or defaults
        // Example for Rust and Java
        self.languages.insert("rust".to_string(), get_tree_sitter_rust());
        self.languages.insert("java".to_string(), get_tree_sitter_java());
        
        // Add Python and Go based on configuration
        if self.config.languages.contains(&"python".to_string()) {
            self.languages.insert("python".to_string(), get_tree_sitter_python());
        }
        if self.config.languages.contains(&"go".to_string()) {
            self.languages.insert("go".to_string(), get_tree_sitter_go());
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

    pub fn detect_language(&self, path: &Path) -> Result<String, TreeSitterError> {
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match extension {
            "rs" => Ok("rust".to_string()),
            "java" => Ok("java".to_string()),
            "py" => Ok("python".to_string()),
            "go" => Ok("go".to_string()),
            "js" | "ts" | "jsx" | "tsx" => Ok("javascript".to_string()), // Group JS/TS
            _ => Err(TreeSitterError::UnsupportedLanguage(format!(
                "Unsupported file extension: {}",
                extension
            ))),
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
        let lang_id = self.detect_language(file_path)?;
        let language = self.languages.get(&lang_id).ok_or_else(|| {
            TreeSitterError::UnsupportedLanguage(format!("Language '{}' not initialized.", lang_id))
        })?;

        let source_code = fs::read_to_string(file_path)
            .map_err(|e| TreeSitterError::IoError(e))?;
        
        let current_hash = calculate_hash(&source_code);

        if self.is_cache_valid(file_path, &current_hash) {
            if let Some(cached_ast) = self.file_asts.get(file_path) {
                debug!("Using cached AST for {:?}", file_path);
                return Ok(cached_ast.clone());
            }
        }
        
        let mut parser = Parser::new();
        parser.set_language(*language)
            .map_err(|e| TreeSitterError::ParseError(format!("Failed to set language for parser: {}", e)))?;

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
                if node_sexp.starts_with("(visibility_modifier") && node.utf8_text(file_ast.source.as_bytes()).unwrap_or("").contains("pub") {
                    return true;
                }
                // Check if it's a direct child of a `source_file` and doesn't have explicit private/crate visibility
                // This is very simplified and likely incorrect for many Rust visibility rules.
                let mut cursor = node.walk();
                for child_node in node.children(&mut cursor) {
                    if child_node.kind() == "visibility_modifier" {
                        return child_node.utf8_text(file_ast.source.as_bytes()).unwrap_or("").contains("pub");
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
                        let modifiers_text = child_node.utf8_text(file_ast.source.as_bytes()).unwrap_or("");
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
        // This is a placeholder. The actual implementation will involve:
        // 1. Parsing the diff_text (e.g., using a diff parsing library or custom logic)
        //    to get a list of FileDiff objects.
        //    The `parse_git_diff` function from the original file needs to be integrated or called.
        //    Let's assume we have a function `super::core::parse_git_diff`
        let git_diff = super::core::parse_git_diff(diff_text)?;


        let mut file_analyses = Vec::new();

        for file_diff_info in &git_diff.changed_files {
            match file_diff_info.change_type {
                ChangeType::Added | ChangeType::Modified => {
                    // For added/modified files, parse them
                    // The path in FileDiff should be the new path
                    let file_path = self.project_root.join(&file_diff_info.path);
                    if !file_path.exists() {
                        warn!("File {:?} mentioned in diff does not exist in project. Skipping.", file_path);
                        continue;
                    }

                    match self.parse_file(&file_path) {
                        Ok(file_ast) => {
                            // Analyze changes within this file based on hunks
                            let affected_nodes = self.analyze_file_changes(&file_ast, &file_diff_info.hunks)?;
                            
                            // Generate a summary for this file (placeholder)
                            let summary = format!("File {:?} was {:?}. Affected nodes: {}", 
                                                  file_ast.path, file_diff_info.change_type, affected_nodes.len());

                            file_analyses.push(FileAnalysis {
                                path: file_ast.path.clone(),
                                language: file_ast.language_id.clone(),
                                change_type: file_diff_info.change_type.clone(),
                                affected_nodes,
                                summary: Some(summary),
                            });
                        }
                        Err(e) => {
                            error!("Failed to parse file {:?}: {:?}", file_path, e);
                            // Add a FileAnalysis entry indicating error
                            file_analyses.push(FileAnalysis {
                                path: file_path.clone(),
                                language: self.detect_language(&file_path).unwrap_or_default(),
                                change_type: file_diff_info.change_type.clone(),
                                affected_nodes: Vec::new(),
                                summary: Some(format!("Error parsing file: {:?}", e)),
                            });
                        }
                    }
                }
                ChangeType::Deleted | ChangeType::Renamed => {
                     // For deleted/renamed files, we might not parse the new content (if deleted)
                     // or we'd parse the new path if renamed.
                     // The FileDiff struct should ideally hold old_path and new_path for renames.
                    let path_to_log = if file_diff_info.change_type == ChangeType::Deleted {
                        file_diff_info.old_path.as_ref().unwrap_or(&file_diff_info.path)
                    } else {
                        &file_diff_info.path // new path for renames
                    };
                    file_analyses.push(FileAnalysis {
                        path: path_to_log.clone(),
                        language: self.detect_language(path_to_log).unwrap_or_default(),
                        change_type: file_diff_info.change_type.clone(),
                        affected_nodes: Vec::new(), // No AST to analyze for purely deleted
                        summary: Some(format!("File {:?} was {:?}.", path_to_log, file_diff_info.change_type)),
                    });
                }
                _ => {} // Handle other change types like Copied, TypeChanged if necessary
            }
        }
        
        // Generate an overall summary (placeholder)
        let overall_summary = super::core::generate_overall_summary(&git_diff); // Use the one from core

        Ok(DiffAnalysis {
            file_analyses,
            overall_summary,
            change_analysis: ChangeAnalysis::default(), // Placeholder
        })
    }

    fn analyze_file_changes(&self, file_ast: &FileAst, hunks: &[DiffHunk]) -> Result<Vec<AffectedNode>, TreeSitterError> {
        let mut affected_nodes = Vec::new();
        let query = self.queries.get(&file_ast.language_id).ok_or_else(|| 
            TreeSitterError::QueryError(format!("No query found for language {}", file_ast.language_id))
        )?;

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
            if current_line < hunk_start_line && hunk_start_line > 0 { // If hunk_start_line is 0, hunk_start_byte remains 0
                 // Reached EOF before hunk start line, means hunk is likely beyond file end (should not happen in valid diff)
                 warn!("Hunk start line {} is beyond file end for {:?}", hunk_start_line + 1, file_ast.path);
                 continue;
            }


            let mut hunk_end_byte = source_bytes.len(); // Default to end of file
            current_line = 0; // Reset for end_byte calculation
            for (i, byte) in source_bytes.iter().enumerate() {
                 if *byte == b'\n' {
                    current_line += 1;
                }
                if current_line == hunk_end_line { // After processing the last line of the hunk
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


            let mut cursor = tree_sitter::QueryCursor::new();
            let matches = cursor.matches(query, tree_root, source_bytes);

            for mat in matches {
                for capture in mat.captures {
                    let node = capture.node;
                    let node_range = node.byte_range();

                    // Check if the node overlaps with the hunk's byte range
                    if node_range.start < hunk_end_byte && node_range.end > hunk_start_byte {
                        let node_name_capture = mat.captures.iter().find(|c| 
                            query.capture_names()[c.index as usize].ends_with(".name")
                        );
                        
                        let name: String = node_name_capture
                            .map(|c| c.node.utf8_text(source_bytes).unwrap_or("").to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        let kind_capture_index = query.capture_names()[capture.index as usize]
                            .split('.')
                            .next()
                            .unwrap_or("unknown_type");
                        
                        let start_line = node.start_position().row;
                        let end_line = node.end_position().row;

                        affected_nodes.push(AffectedNode {
                            node_type: kind_capture_index.to_string(),
                            name,
                            range: (node_range.start, node_range.end),
                            is_public: self.is_node_public(&node, file_ast),
                            content: Some(node.utf8_text(source_bytes).unwrap_or("").to_string()),
                            line_range: (start_line, end_line),
                        });
                    }
                }
            }
        }
        // Deduplicate affected_nodes if necessary (e.g. by node range and type)
        affected_nodes.sort_by_key(|n| (n.range.0, n.range.1, n.node_type.clone(), n.name.clone()));
        affected_nodes.dedup_by_key(|n| (n.range.0, n.range.1, n.node_type.clone(), n.name.clone()));
        Ok(affected_nodes)
    }
    
    #[allow(dead_code)]
    pub fn analyze_java_project_structure(&mut self, file_paths: &[PathBuf]) -> Result<JavaProjectStructure, TreeSitterError> {
        let mut project_structure = JavaProjectStructure::new();
        for file_path in file_paths {
            if self.detect_language(file_path)? != "java" {
                continue;
            }
            let ast = self.parse_file(file_path)?;
            
            let package_name = match extract_java_package_name(&ast) {
                Ok(name) => name,
                Err(_) => "".to_string()
            };
            
            let class_name = match extract_java_class_name(&ast) {
                Ok(name) => name,
                Err(_) => continue // Skip if we can't extract class name
            };
            
            project_structure.add_class(&package_name, &class_name, file_path);

            match extract_java_imports(&ast) {
                Ok(imports) => {
                    for imp in imports {
                        project_structure.add_import(&package_name, &class_name, &imp);
                    }
                },
                Err(_) => {} // Continue even if imports can't be extracted
            }

            match extract_java_class_relations(&ast) {
                Ok(relations) => {
                    for rel in relations {
                        project_structure.add_relation(&package_name, &class_name, &rel);
                    }
                },
                Err(_) => {} // Continue even if relations can't be extracted
            }

            match extract_java_methods(&ast, self) {
                Ok(methods) => {
                    for method in methods {
                        project_structure.add_method(&package_name, &class_name, &method);
                    }
                },
                Err(_) => {} // Continue even if methods can't be extracted
            }
                
            // Simplified Spring Bean / JPA Entity detection
            // A real implementation would check for specific annotations on the class
            let source_code = &ast.source;
            if source_code.contains("@Entity") { // Very basic check
                project_structure.mark_as_jpa_entity(&package_name, &class_name);
            }
            if source_code.contains("@Component") || source_code.contains("@Service") || source_code.contains("@Repository") { // Basic check
                project_structure.mark_as_spring_bean(&package_name, &class_name);
            }
        }
        Ok(project_structure)
    }
    
    // generate_commit_prompt would also go here, likely calling analyze_diff first.
    pub async fn generate_commit_prompt(&mut self, diff_text: &str, config: &crate::config_management::settings::AppConfig) -> Result<String, AppError> {
        // 1. Analyze the diff
        let diff_analysis = self.analyze_diff(diff_text)
            .map_err(|e| AppError::TreeSitter(e))?;

        // 2. Construct a prompt based on the analysis
        // This is a simplified example. You'd want to be more sophisticated.
        let mut prompt_parts = Vec::new();
        prompt_parts.push("Generate a concise and informative commit message for the following changes:".to_string());
        prompt_parts.push(format!("\nOverall summary: {}", diff_analysis.overall_summary));

        if !diff_analysis.file_analyses.is_empty() {
            prompt_parts.push("\nKey changes per file:".to_string());
            for file_analysis in diff_analysis.file_analyses.iter().take(5) { // Limit for brevity
                prompt_parts.push(format!("- File: {}", file_analysis.path.display()));
                if let Some(summary) = &file_analysis.summary {
                    prompt_parts.push(format!("  Summary: {}", summary));
                }
                if !file_analysis.affected_nodes.is_empty() {
                    prompt_parts.push("  Affected elements:".to_string());
                    for node in file_analysis.affected_nodes.iter().take(3) { // Limit for brevity
                        prompt_parts.push(format!("    - {} {} ({}public)", 
                            node.node_type, node.name, if node.is_public {""} else {"non-"}));
                    }
                }
            }
        }
        
        // Add context from AppConfig if needed (e.g., commit message conventions)
        if let Some(syntax_prompt) = config.prompts.get("commit-syntax") {
            prompt_parts.push("\nAdhere to the following commit message syntax and conventions:".to_string());
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
