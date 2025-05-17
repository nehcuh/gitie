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

// Represents a single hunk in a Git diff
#[derive(Debug, Clone)]
pub struct DiffHunk {
    #[allow(dead_code)]
    pub old_range: LineRange,
    pub new_range: LineRange,
    #[allow(dead_code)]
    pub lines: Vec<String>,
}

// Represents a changed file in a Git diff
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub change_type: ChangeType,
    pub hunks: Vec<DiffHunk>,
}

// Represents the entire Git diff
#[derive(Debug, Clone)]
pub struct GitDiff {
    pub changed_files: Vec<FileDiff>,
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
        self.classes.entry(class_name.clone())
            .or_insert_with(|| JavaClass::new(class_name, path.to_path_buf()));
    }
    
    #[allow(dead_code)]
    pub fn get_classes(&self) -> Vec<&JavaClass> {
        self.classes.values().collect()
    }
}

// Java project structure (packages and classes)
#[derive(Debug, Clone, Default)]
pub struct JavaProjectStructure {
    #[allow(dead_code)]
    pub packages: HashMap<String, JavaPackage>,
}

impl JavaProjectStructure {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { packages: HashMap::new() }
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
    pub fn get_package(&self, package_name: &str) -> Option<&JavaPackage> {
        self.packages.get(package_name)
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

// Parse git diff output
pub fn parse_git_diff(diff_text: &str) -> Result<GitDiff, TreeSitterError> {
    let mut changed_files = Vec::new();
    let mut current_file_diff: Option<FileDiff> = None;
    let mut current_hunks: Vec<DiffHunk> = Vec::new();
    let mut current_hunk_lines: Vec<String> = Vec::new();
    let mut current_hunk_header: Option<String> = None;
    let mut old_range: Option<LineRange> = None;
    let mut new_range: Option<LineRange> = None;

    for line in diff_text.lines() {
        if line.starts_with("diff --git") {
            if let Some(mut file_diff) = current_file_diff.take() {
                if let Some(_h_header) = current_hunk_header.take() {
                     if let (Some(or), Some(nr)) = (old_range.take(), new_range.take()) {
                        current_hunks.push(DiffHunk {
                            old_range: or,
                            new_range: nr,
                            lines: std::mem::take(&mut current_hunk_lines),
                        });
                    }
                }
                file_diff.hunks = std::mem::take(&mut current_hunks);
                changed_files.push(file_diff);
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let old_file_name = parts[2][2..].to_string();
                let new_file_name = parts[3][2..].to_string();
                current_file_diff = Some(FileDiff {
                    old_path: Some(PathBuf::from(old_file_name)),
                    path: PathBuf::from(new_file_name),
                    hunks: Vec::new(),
                    change_type: ChangeType::Modified,
                });
            }
        } else if line.starts_with("--- a/") {
            if let Some(ref mut file_diff) = current_file_diff {
                file_diff.old_path = Some(PathBuf::from(&line[6..]));
            }
        } else if line.starts_with("+++ b/") {
            if let Some(ref mut file_diff) = current_file_diff {
                file_diff.path = PathBuf::from(&line[6..]);
            }
        } else if line.starts_with("@@ ") {
            if let Some(_) = current_hunk_header.take() {
                if let (Some(or), Some(nr)) = (old_range.take(), new_range.take()) {
                    current_hunks.push(DiffHunk {
                        old_range: or,
                        new_range: nr,
                        lines: std::mem::take(&mut current_hunk_lines),
                    });
                }
            }
            current_hunk_header = Some(line.to_string());
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                 old_range = Some(parse_line_range(parts[1]));
                 new_range = Some(parse_line_range(parts[2]));
            }
        } else if current_hunk_header.is_some() {
            current_hunk_lines.push(line.to_string());
        } else if line.starts_with("new file mode") {
            if let Some(ref mut file) = current_file_diff {
                file.change_type = ChangeType::Added;
            }
        } else if line.starts_with("deleted file mode") {
            if let Some(ref mut file) = current_file_diff {
                file.change_type = ChangeType::Deleted;
                if let Some(old_path) = &file.old_path {
                     file.path = old_path.clone();
                }
            }
        } else if line.starts_with("rename from ") {
             if let Some(ref mut file) = current_file_diff {
                file.change_type = ChangeType::Renamed;
                file.old_path = Some(PathBuf::from(&line[12..]));
            }
        } else if line.starts_with("rename to ") {
            if let Some(ref mut file) = current_file_diff {
                file.path = PathBuf::from(&line[10..]);
            }
        }
    }

    if let Some(mut file_diff) = current_file_diff.take() {
        if let Some(_) = current_hunk_header.take() {
            if let (Some(or), Some(nr)) = (old_range.take(), new_range.take()) {
                current_hunks.push(DiffHunk {
                    old_range: or,
                    new_range: nr,
                    lines: std::mem::take(&mut current_hunk_lines),
                });
            }
        }
        file_diff.hunks = std::mem::take(&mut current_hunks);
        changed_files.push(file_diff);
    }

    Ok(GitDiff { changed_files })
}

fn parse_line_range(range_str: &str) -> LineRange {
    let cleaned_range_str = range_str.trim_start_matches(|c| c == '-' || c == '+');
    let parts: Vec<&str> = cleaned_range_str.split(',').collect();
    let start = parts[0].parse::<u32>().unwrap_or(1);
    let count = if parts.len() > 1 {
        parts[1].parse::<u32>().unwrap_or(0)
    } else {
        1
    };
    LineRange { start, count }
}

// Generate an overall summary of changes based on a GitDiff
pub fn generate_overall_summary(diff: &GitDiff) -> String {
    let file_count = diff.changed_files.len();
    
    let added_count = diff.changed_files.iter()
        .filter(|f| f.change_type == ChangeType::Added)
        .count();
        
    let modified_count = diff.changed_files.iter()
        .filter(|f| f.change_type == ChangeType::Modified)
        .count();
        
    let deleted_count = diff.changed_files.iter()
        .filter(|f| f.change_type == ChangeType::Deleted)
        .count();
        
    let renamed_count = diff.changed_files.iter()
        .filter(|f| f.change_type == ChangeType::Renamed)
        .count();
    
    let java_files_count = diff.changed_files.iter()
        .filter(|f| f.path.extension().map_or(false, |ext| ext == "java"))
        .count();
        
    let rust_files_count = diff.changed_files.iter()
        .filter(|f| f.path.extension().map_or(false, |ext| ext == "rs"))
        .count();
    
    let python_files_count = diff.changed_files.iter()
        .filter(|f| f.path.extension().map_or(false, |ext| ext == "py"))
        .count();
        
    let go_files_count = diff.changed_files.iter()
        .filter(|f| f.path.extension().map_or(false, |ext| ext == "go"))
        .count();
        
    let js_files_count = diff.changed_files.iter()
        .filter(|f| f.path.extension().map_or(false, |ext| ext == "js" || ext == "ts" || ext == "jsx" || ext == "tsx"))
        .count();
        
    let mut summary = format!(
        "本次变更涉及 {} 个文件：新增 {}，修改 {}，删除 {}，重命名 {}",
        file_count, added_count, modified_count, deleted_count, renamed_count
    );
    
    let language_counts = [
        (java_files_count, "Java"),
        (rust_files_count, "Rust"),
        (python_files_count, "Python"),
        (go_files_count, "Go"),
        (js_files_count, "JavaScript/TypeScript")
    ];
    
    let language_stats: Vec<String> = language_counts.into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, lang)| format!("{} ({} 个文件)", lang, count))
        .collect();
    
    if !language_stats.is_empty() {
        summary.push_str(&format!("\n涉及语言：{}", language_stats.join(", ")));
    }
    
    summary
}

