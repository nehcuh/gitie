use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tree_sitter::{Language, Tree};
pub use crate::core::errors::TreeSitterError; // Re-export for use in mod.rs

// Add missing variants to TreeSitterError if needed
// Uncomment these if these variants are required
// impl TreeSitterError {
//     pub fn LanguageError(msg: String) -> Self {
//         TreeSitterError::UnsupportedLanguage(msg)
//     }
// }

// Rust语言解析器
pub fn get_tree_sitter_rust() -> Language {
    tree_sitter_rust::language()
}

// Java语言解析器
pub fn get_tree_sitter_java() -> Language {
    tree_sitter_java::language()
}

// Python语言解析器
pub fn get_tree_sitter_python() -> Language {
    tree_sitter_python::language()
}

// Go语言解析器
pub fn get_tree_sitter_go() -> Language {
    tree_sitter_go::language()
}

// 文件AST结构
// 这个结构体代表一个文件的语法分析树(AST)
// 使用tree-sitter提供的实际Tree类型
#[derive(Debug, Clone)]
pub struct FileAst {
    /// 文件路径
    pub path: PathBuf,
    /// tree-sitter解析树
    pub tree: Tree,
    /// 源代码
    pub source: String,
    /// 内容哈希值
    pub content_hash: String,
    /// 最后解析时间
    #[allow(dead_code)]
    pub last_parsed: SystemTime,
    /// 语言标识
    pub language_id: String,
}

impl From<std::io::Error> for TreeSitterError {
    fn from(e: std::io::Error) -> Self {
        TreeSitterError::IoError(e)
    }
}

// Define TreeSitterError enum/struct here or ensure it's imported correctly
// For now, let's assume it might look something like this:
// pub enum TreeSitterError {
//     IoError(std::io::Error),
//     LanguageError(String),
//     QueryError(String),
//     ParseError(String),
//     UnsupportedLanguage(String),
//     Other(String),
// }

// Defines the type of change in a Git diff
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    #[allow(dead_code)]
    Copied,
    #[allow(dead_code)]
    TypeChanged,
}

// Represents a range of lines in a diff hunk
#[derive(Debug, Clone)]
pub struct LineRange {
    pub start: u32,
    pub count: u32,
}

impl LineRange {
    #[allow(dead_code)]
    pub fn new(start: u32, count: u32) -> Self {
        Self { start, count }
    }
}

// Represents a hunk range in git diff format (@@ -a,b +c,d @@)
#[derive(Debug, Clone)]
pub struct HunkRange {
    pub start: usize,
    pub count: usize,
}

// Represents a single hunk in a Git diff
#[derive(Debug, Clone)]
pub struct DiffHunk {
    #[allow(dead_code)]
    pub old_range: HunkRange,
    pub new_range: HunkRange,
    #[allow(dead_code)]
    pub lines: Vec<String>,
}

// Legacy structure, keeping this for backward compatibility,
// but we're migrating to ChangedFile
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub change_type: ChangeType,
    pub hunks: Vec<DiffHunk>,
}

// Conversion functions between FileDiff and ChangedFile
impl From<FileDiff> for ChangedFile {
    fn from(file_diff: FileDiff) -> Self {
        ChangedFile {
            path: file_diff.path,
            change_type: file_diff.change_type,
            hunks: file_diff.hunks,
            file_mode_change: None,
        }
    }
}

impl From<ChangedFile> for FileDiff {
    fn from(changed_file: ChangedFile) -> Self {
        FileDiff {
            path: changed_file.path,
            old_path: None,
            change_type: changed_file.change_type,
            hunks: changed_file.hunks,
        }
    }
}

// Represents a changed file in a Git diff
#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub hunks: Vec<DiffHunk>,
    pub file_mode_change: Option<String>,
}

// Represents the entire Git diff
#[derive(Debug, Clone)]
pub struct GitDiff {
    pub changed_files: Vec<ChangedFile>,
    pub metadata: Option<HashMap<String, String>>,
}

// These conversion functions are no longer needed
// since we're directly creating ChangedFile objects

impl GitDiff {
    /// Counts the total number of lines in the diff
    pub fn total_lines(&self) -> usize {
        let mut count = 0;
        for file in &self.changed_files {
            for hunk in &file.hunks {
                count += hunk.lines.len();
            }
        }
        count
    }
    
    /// Counts the number of changed lines
    pub fn changed_lines(&self) -> usize {
        let mut count = 0;
        for file in &self.changed_files {
            for hunk in &file.hunks {
                for line in &hunk.lines {
                    if line.starts_with('+') || line.starts_with('-') {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

// Represents a node in the AST affected by changes
#[derive(Debug, Clone)]
pub struct AffectedNode {
    pub node_type: String,
    pub name: String,
    pub range: (usize, usize),
    pub is_public: bool,
    #[allow(dead_code)]
    pub content: Option<String>,
    #[allow(dead_code)]
    pub line_range: (usize, usize),
}

impl AffectedNode {
    #[allow(dead_code)]
    pub fn new(node_type: String, name: String, range: (usize, usize), is_public: bool) -> Self {
        Self {
            node_type,
            name,
            range,
            is_public,
            content: None,
            line_range: (0, 0),
        }
    }
}

// Analysis of a single file
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub language: String,
    #[allow(dead_code)]
    pub change_type: ChangeType,
    pub affected_nodes: Vec<AffectedNode>,
    pub summary: Option<String>,
}

// Analysis of changes in a diff
#[derive(Debug, Clone, Default)]
pub struct ChangeAnalysis {
    #[allow(dead_code)]
    pub function_changes: usize,
    #[allow(dead_code)]
    pub type_changes: usize,
    #[allow(dead_code)]
    pub method_changes: usize,
    #[allow(dead_code)]
    pub interface_changes: usize,
    #[allow(dead_code)]
    pub other_changes: usize,
    #[allow(dead_code)]
    pub change_pattern: ChangePattern,
    #[allow(dead_code)]
    pub change_scope: ChangeScope,
}

// Complete analysis of a Git diff
#[derive(Debug, Clone)]
pub struct DiffAnalysis {
    pub file_analyses: Vec<FileAnalysis>,
    pub overall_summary: String,
    #[allow(dead_code)]
    pub change_analysis: ChangeAnalysis,
}

// Mapping between diff and AST
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiffAstMapping {
    pub files: HashMap<String, FileDiffAstMapping>,
}

// Mapping for a single file
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FileDiffAstMapping {
    pub file_path: String,
    pub hunks: Vec<HunkAstMapping>,
}

// Mapping for a single hunk
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HunkAstMapping {
    pub hunk: DiffHunk,
    pub nodes: Vec<AffectedNode>,
}

// Types of change patterns
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangePattern {
    #[allow(dead_code)]
    FeatureImplementation,
    #[allow(dead_code)]
    BugFix,
    #[allow(dead_code)]
    Refactoring,
    #[allow(dead_code)]
    ModelChange,
    #[allow(dead_code)]
    BehaviorChange,
    #[allow(dead_code)]
    ConfigurationChange,
    MixedChange,
    #[allow(dead_code)]
    JavaStructuralChange,
    #[allow(dead_code)]
    JavaVisibilityChange,
    #[allow(dead_code)]
    JavaAnnotationChange,
}

impl Default for ChangePattern {
    fn default() -> Self {
        ChangePattern::MixedChange
    }
}

// Scope of changes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeScope {
    Minor,
    #[allow(dead_code)]
    Moderate,
    #[allow(dead_code)]
    Major,
}

impl Default for ChangeScope {
    fn default() -> Self {
        ChangeScope::Minor
    }
}

// Parse git diff output into a GitDiff structure
pub fn parse_git_diff(diff_text: &str) -> Result<GitDiff, TreeSitterError> {
    // Delegate to the newer parser implementation
    match crate::tree_sitter_analyzer::parse_utils::parse_git_diff_text(diff_text) {
        Ok(git_diff) => Ok(git_diff),
        Err(e) => Err(TreeSitterError::ParseError(format!("Failed to parse diff: {}", e)))
    }
}

// Types of relationships between Java classes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaRelationType {
    #[allow(dead_code)]
    Extends,
    #[allow(dead_code)]
    Implements,
}

// Relationship between Java classes
#[derive(Debug, Clone)]
pub struct JavaClassRelation {
    #[allow(dead_code)]
    pub relation_type: JavaRelationType,
    #[allow(dead_code)]
    pub target_class: String,
}

// Parameter in a Java method
#[derive(Debug, Clone)]
pub struct JavaMethodParam {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub param_type: String,
}

// Java method definition
// Java method
#[derive(Debug, Clone)]
pub struct JavaMethod {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub return_type: String,
    #[allow(dead_code)]
    pub parameters: Vec<JavaMethodParam>,
    #[allow(dead_code)]
    pub is_public: bool,
    #[allow(dead_code)]
    pub is_static: bool,
    #[allow(dead_code)]
    pub is_abstract: bool,
    #[allow(dead_code)]
    pub is_constructor: bool,
    #[allow(dead_code)]
    pub annotations: Vec<String>,
}

// Java class definition
// Java class
#[derive(Debug, Clone)]
pub struct JavaClass {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub path: PathBuf,
    #[allow(dead_code)]
    pub imports: HashSet<String>,
    #[allow(dead_code)]
    pub relations: Vec<JavaClassRelation>,
    #[allow(dead_code)]
    pub methods: Vec<JavaMethod>,
    #[allow(dead_code)]
    pub is_spring_bean: bool,
    #[allow(dead_code)]
    pub is_jpa_entity: bool,
}

impl JavaClass {
    #[allow(dead_code)]
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            imports: HashSet::new(),
            relations: Vec::new(),
            methods: Vec::new(),
            is_spring_bean: false,
            is_jpa_entity: false,
        }
    }
}

// Java package grouping classes
// Java package
#[derive(Debug, Clone)]
pub struct JavaPackage {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub classes: HashMap<String, JavaClass>,
}

impl JavaPackage {
    #[allow(dead_code)]
    pub fn new(name: String) -> Self {
        Self {
            name,
            classes: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn add_class(&mut self, class_name: String, path: &Path) {
        let class = JavaClass::new(class_name.clone(), path.to_path_buf());
        self.classes.insert(class_name, class);
    }

    #[allow(dead_code)]
    pub fn get_classes(&self) -> Vec<&JavaClass> {
        self.classes.values().collect()
    }
}

// Project-wide Java structure
#[derive(Debug, Clone)]
pub struct JavaProjectStructure {
    #[allow(dead_code)]
    pub packages: HashMap<String, JavaPackage>,
}

impl JavaProjectStructure {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn add_class(&mut self, package_name: &str, class_name: &str, path: &Path) {
        let package = self.packages.entry(package_name.to_string())
            .or_insert_with(|| JavaPackage::new(package_name.to_string()));
        package.add_class(class_name.to_string(), path);
    }

    #[allow(dead_code)]
    pub fn add_import(&mut self, package_name: &str, class_name: &str, import: &str) {
        if let Some(package) = self.packages.get_mut(package_name) {
            if let Some(class) = package.classes.get_mut(class_name) {
                class.imports.insert(import.to_string());
            }
        }
    }

    #[allow(dead_code)]
    pub fn add_relation(&mut self, package_name: &str, class_name: &str, relation: &JavaClassRelation) {
        if let Some(package) = self.packages.get_mut(package_name) {
            if let Some(class) = package.classes.get_mut(class_name) {
                class.relations.push(relation.clone());
            }
        }
    }

    #[allow(dead_code)]
    pub fn add_method(&mut self, package_name: &str, class_name: &str, method: &JavaMethod) {
        if let Some(package) = self.packages.get_mut(package_name) {
            if let Some(class) = package.classes.get_mut(class_name) {
                class.methods.push(method.clone());
            }
        }
    }

    #[allow(dead_code)]
    pub fn mark_as_spring_bean(&mut self, package_name: &str, class_name: &str) {
        if let Some(package) = self.packages.get_mut(package_name) {
            if let Some(class) = package.classes.get_mut(class_name) {
                class.is_spring_bean = true;
            }
        }
    }

    #[allow(dead_code)]
    pub fn mark_as_jpa_entity(&mut self, package_name: &str, class_name: &str) {
        if let Some(package) = self.packages.get_mut(package_name) {
            if let Some(class) = package.classes.get_mut(class_name) {
                class.is_jpa_entity = true;
            }
        }
    }

    #[allow(dead_code)]
    pub fn get_packages(&self) -> Vec<&JavaPackage> {
        self.packages.values().collect()
    }

    #[allow(dead_code)]
    pub fn get_package(&self, name: &str) -> Option<&JavaPackage> {
        self.packages.get(name)
    }
}

// Utility functions
pub fn calculate_hash(content: &str) -> String {
    // Simple hash function to avoid dependency on sha2
    let mut hash: u64 = 0;
    for byte in content.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    format!("{:x}", hash)
}

fn parse_line_range(range_str: &str) -> LineRange {
    let cleaned_range_str = range_str.trim_start_matches(|c| c == '-' || c == '+');
    let parts: Vec<&str> = cleaned_range_str.split(',').collect();

    if let Some(start_str) = parts.get(0) {
        let start = start_str.parse::<u32>().unwrap_or(0);
        let count = if let Some(count_str) = parts.get(1) {
            count_str.parse::<u32>().unwrap_or(0)
        } else {
            1  // Default to 1 line if not specified
        };
        
        LineRange { start, count }
    } else {
        LineRange { start: 0, count: 0 }
    }
}

// Function to generate an overall summary from analysis
pub fn generate_overall_summary(file_analyses: &[FileAnalysis]) -> String {
    let mut summary = String::new();
    
    if file_analyses.is_empty() {
        return "No files analyzed".to_string();
    }
    
    let file_count = file_analyses.len();
    let mut total_nodes = 0;
    let mut function_count = 0;
    let mut class_count = 0;
    let mut languages = HashMap::new();
    
    for analysis in file_analyses {
        total_nodes += analysis.affected_nodes.len();
        
        for node in &analysis.affected_nodes {
            if node.node_type.contains("function") || node.node_type.contains("method") {
                function_count += 1;
            } else if node.node_type.contains("class") || node.node_type.contains("struct") || node.node_type.contains("interface") {
                class_count += 1;
            }
        }
        
        let lang = &analysis.language;
        *languages.entry(lang.clone()).or_insert(0) += 1;
    }
    
    summary.push_str(&format!("Analyzed {} files\n", file_count));
    
    if !languages.is_empty() {
        summary.push_str("Languages: ");
        let langs: Vec<String> = languages.iter()
            .map(|(lang, count)| format!("{} ({})", lang, count))
            .collect();
        summary.push_str(&langs.join(", "));
        summary.push_str("\n");
    }
    
    summary.push_str(&format!("Found {} affected nodes\n", total_nodes));
    
    if function_count > 0 || class_count > 0 {
        summary.push_str("Including: ");
        if function_count > 0 {
            summary.push_str(&format!("{} functions/methods", function_count));
        }
        if class_count > 0 {
            if function_count > 0 {
                summary.push_str(", ");
            }
            summary.push_str(&format!("{} classes/structs", class_count));
        }
        summary.push_str("\n");
    }
    
    summary
}