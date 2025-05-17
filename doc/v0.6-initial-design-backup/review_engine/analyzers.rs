//! 代码语法分析器模块
//! 
//! 该模块提供了多种语言的代码分析功能，用于支持代码评审。
//! 利用Tree-sitter语法解析提供更深入的代码理解和结构化分析。

use crate::tree_sitter_analyzer::{analyzer::TreeSitterAnalyzer, core::AnalysisDepth};
use crate::review_engine::{RuleContext, CodeLocation, Issue, Severity, RuleCategory};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 代码分析器接口
pub trait CodeAnalyzer {
    /// 分析指定文件
    fn analyze_file(&self, file_path: &str, content: &str) -> Result<FileAnalysis, String>;
    
    /// 获取支持的语言
    fn supported_languages(&self) -> Vec<String>;
    
    /// 判断是否支持某种语言
    fn supports_language(&self, language: &str) -> bool {
        self.supported_languages().contains(&language.to_lowercase())
    }
}

/// 基于Tree-sitter的代码分析器
pub struct TreeSitterCodeAnalyzer {
    /// Tree-sitter分析器
    analyzer: TreeSitterAnalyzer,
    /// 分析深度
    depth: AnalysisDepth,
}

impl TreeSitterCodeAnalyzer {
    /// 创建新的Tree-sitter代码分析器
    pub fn new(depth: AnalysisDepth) -> Result<Self, String> {
        let analyzer = TreeSitterAnalyzer::new()
            .map_err(|e| format!("初始化Tree-sitter分析器失败: {}", e))?;
            
        Ok(Self {
            analyzer,
            depth,
        })
    }
    
    /// 检测文件语言
    pub fn detect_file_language(&self, file_path: &str) -> Option<String> {
        let path = Path::new(file_path);
        let extension = path.extension().and_then(|e| e.to_str())?;
        
        match extension.to_lowercase().as_str() {
            "rs" => Some("rust".to_string()),
            "py" => Some("python".to_string()),
            "java" => Some("java".to_string()),
            "js" | "jsx" => Some("javascript".to_string()),
            "ts" | "tsx" => Some("typescript".to_string()),
            "c" => Some("c".to_string()),
            "cpp" | "cc" | "cxx" => Some("cpp".to_string()),
            "h" | "hpp" => Some("cpp".to_string()),
            "go" => Some("go".to_string()),
            _ => None,
        }
    }
}

impl CodeAnalyzer for TreeSitterCodeAnalyzer {
    fn analyze_file(&self, file_path: &str, content: &str) -> Result<FileAnalysis, String> {
        // 检测语言
        let language = self.detect_file_language(file_path)
            .ok_or_else(|| format!("无法确定文件语言: {}", file_path))?;
        
        // 分析文件结构
        let mut functions = Vec::new();
        let mut classes = Vec::new();
        let mut imports = Vec::new();
        
        // 使用Tree-sitter分析器处理代码
        if let Ok(language_id) = self.analyzer.get_language_by_name(&language) {
            if let Ok(tree) = self.analyzer.parse_code(content, language_id) {
                // 分析函数和方法
                if let Ok(funcs) = self.analyzer.find_functions(&tree, content) {
                    for func in funcs {
                        functions.push(CodeFunction {
                            name: func.name,
                            start_line: func.start_position.0,
                            end_line: func.end_position.0,
                            complexity: calculate_complexity(&func.content, &language),
                            parameters: func.parameters,
                        });
                    }
                }
                
                // 分析类和结构体
                if let Ok(cls) = self.analyzer.find_classes(&tree, content) {
                    for class in cls {
                        classes.push(CodeClass {
                            name: class.name,
                            start_line: class.start_position.0,
                            end_line: class.end_position.0,
                            methods: class.methods.len(),
                            fields: class.fields.len(),
                        });
                    }
                }
                
                // 分析导入语句
                if let Ok(imps) = self.analyzer.find_imports(&tree, content) {
                    for imp in imps {
                        imports.push(CodeImport {
                            module: imp.module,
                            start_line: imp.start_position.0,
                            is_relative: imp.module.starts_with('.'),
                        });
                    }
                }
            }
        }
        
        // 创建文件分析结果
        Ok(FileAnalysis {
            path: file_path.to_string(),
            language,
            functions,
            classes,
            imports,
            loc: content.lines().count(),
            complexity_score: calculate_file_complexity(&functions),
        })
    }
    
    fn supported_languages(&self) -> Vec<String> {
        vec![
            "rust".to_string(),
            "python".to_string(),
            "java".to_string(),
            "javascript".to_string(),
            "typescript".to_string(),
            "c".to_string(),
            "cpp".to_string(),
            "go".to_string(),
        ]
    }
}

/// 计算代码复杂度
fn calculate_complexity(code: &str, language: &str) -> u32 {
    // 简单的复杂度计算方法，后期可以替换为更复杂的算法
    let mut complexity = 1;
    
    // 条件分支增加复杂度
    let branch_keywords = match language {
        "rust" => vec!["if", "else", "match", "while", "for", "loop"],
        "python" => vec!["if", "elif", "else", "for", "while", "try", "except"],
        "java" | "javascript" | "typescript" | "c" | "cpp" => vec!["if", "else", "switch", "case", "for", "while", "do", "try", "catch"],
        "go" => vec!["if", "else", "switch", "case", "for", "select"],
        _ => vec!["if", "else", "switch", "for", "while"],
    };
    
    for keyword in branch_keywords {
        // 简单计数，实际应该使用语法树来准确识别
        let count = code.matches(&format!(" {} ", keyword)).count();
        complexity += count as u32;
    }
    
    complexity
}

/// 计算文件整体复杂度
fn calculate_file_complexity(functions: &[CodeFunction]) -> u32 {
    let mut total = 0;
    for func in functions {
        total += func.complexity;
    }
    total
}

/// 文件分析结果
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    /// 文件路径
    pub path: String,
    /// 编程语言
    pub language: String,
    /// 函数列表
    pub functions: Vec<CodeFunction>,
    /// 类列表
    pub classes: Vec<CodeClass>,
    /// 导入语句
    pub imports: Vec<CodeImport>,
    /// 代码行数
    pub loc: usize,
    /// 整体复杂度评分
    pub complexity_score: u32,
}

/// 代码函数信息
#[derive(Debug, Clone)]
pub struct CodeFunction {
    /// 函数名
    pub name: String,
    /// 起始行
    pub start_line: usize,
    /// 结束行
    pub end_line: usize,
    /// 圈复杂度
    pub complexity: u32,
    /// 参数列表
    pub parameters: Vec<String>,
}

/// 代码类信息
#[derive(Debug, Clone)]
pub struct CodeClass {
    /// 类名
    pub name: String,
    /// 起始行
    pub start_line: usize,
    /// 结束行
    pub end_line: usize,
    /// 方法数量
    pub methods: usize,
    /// 字段数量
    pub fields: usize,
}

/// 导入语句信息
#[derive(Debug, Clone)]
pub struct CodeImport {
    /// 模块名
    pub module: String,
    /// 起始行
    pub start_line: usize,
    /// 是否相对导入
    pub is_relative: bool,
}

/// 项目分析器
pub struct ProjectAnalyzer {
    /// 代码分析器
    code_analyzer: Box<dyn CodeAnalyzer>,
    /// 工作目录
    work_dir: PathBuf,
    /// 缓存的分析结果
    cache: HashMap<String, FileAnalysis>,
}

impl ProjectAnalyzer {
    /// 创建新的项目分析器
    pub fn new(code_analyzer: Box<dyn CodeAnalyzer>, work_dir: PathBuf) -> Self {
        Self {
            code_analyzer,
            work_dir,
            cache: HashMap::new(),
        }
    }
    
    /// 分析项目文件
    pub fn analyze_project_file(&mut self, relative_path: &str, content: &str) -> Result<&FileAnalysis, String> {
        // 检查缓存
        if !self.cache.contains_key(relative_path) {
            // 分析文件
            let analysis = self.code_analyzer.analyze_file(relative_path, content)?;
            // 存入缓存
            self.cache.insert(relative_path.to_string(), analysis);
        }
        
        // 返回缓存的分析结果
        Ok(self.cache.get(relative_path).unwrap())
    }
    
    /// 获取所有已分析的文件
    pub fn get_analyzed_files(&self) -> Vec<&str> {
        self.cache.keys().map(|s| s.as_str()).collect()
    }
    
    /// 清除缓存
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
    
    /// 生成项目分析报告
    pub fn generate_report(&self) -> ProjectAnalysisReport {
        let mut total_files = 0;
        let mut total_functions = 0;
        let mut total_classes = 0;
        let mut total_lines = 0;
        let mut languages = HashMap::new();
        let mut complex_functions = Vec::new();
        
        // 聚合分析数据
        for (path, analysis) in &self.cache {
            total_files += 1;
            total_functions += analysis.functions.len();
            total_classes += analysis.classes.len();
            total_lines += analysis.loc;
            
            // 统计语言
            *languages.entry(analysis.language.clone()).or_insert(0) += 1;
            
            // 收集复杂函数
            for func in &analysis.functions {
                if func.complexity > 10 {  // 阈值可配置
                    complex_functions.push((path.clone(), func.clone()));
                }
            }
        }
        
        // 生成报告
        ProjectAnalysisReport {
            file_count: total_files,
            function_count: total_functions,
            class_count: total_classes,
            total_loc: total_lines,
            languages,
            complex_functions,
        }
    }
}

/// 项目分析报告
#[derive(Debug)]
pub struct ProjectAnalysisReport {
    /// 文件数量
    pub file_count: usize,
    /// 函数数量
    pub function_count: usize,
    /// 类数量
    pub class_count: usize,
    /// 总代码行数
    pub total_loc: usize,
    /// 语言统计
    pub languages: HashMap<String, usize>,
    /// 复杂函数列表（路径，函数信息）
    pub complex_functions: Vec<(String, CodeFunction)>,
}