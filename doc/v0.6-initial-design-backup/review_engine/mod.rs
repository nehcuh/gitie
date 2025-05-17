//! 代码评审规则引擎
//! 
//! 该模块提供了代码评审功能的规则引擎，包括规则接口、规则引擎和评审结果数据结构。

pub mod rules;
pub mod analyzers;

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use crate::tree_sitter_analyzer::core::{GitDiff, ChangedFile, DiffHunk};
use crate::tree_sitter_analyzer::analyzer::{TreeSitterAnalyzer, TreeSitterConfig};

/// 规则类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleCategory {
    /// 代码风格
    Style,
    /// 安全性
    Security,
    /// 性能
    Performance,
    /// 代码复杂度
    Complexity,
    /// 最佳实践
    BestPractices,
    /// 潜在bug
    Bugs,
}

/// 问题严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// 错误：需要立即修复的严重问题
    Error,
    /// 警告：应当修复的问题
    Warning,
    /// 信息：可以改进的地方
    Info,
    /// 提示：细微的改进建议
    Hint,
}

/// 代码位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLocation {
    /// 文件路径
    pub file_path: String,
    /// 开始行（1-based）
    pub start_line: usize,
    /// 结束行（1-based，含）
    pub end_line: usize,
    /// 开始列（可选）
    pub start_column: Option<usize>,
    /// 结束列（可选）
    pub end_column: Option<usize>,
}

/// 评审问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// 问题唯一标识符
    pub id: String,
    /// 问题标题
    pub title: String,
    /// 问题描述
    pub description: String,
    /// 代码位置
    pub location: CodeLocation,
    /// 严重程度
    pub severity: Severity,
    /// 问题类别
    pub category: RuleCategory,
    /// 代码片段
    pub code_snippet: Option<String>,
    /// 修复建议
    pub suggestion: Option<String>,
    /// 详细解释（针对特定语言/框架）
    pub explanation: Option<String>,
}

/// 评审结果摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSummary {
    /// 评审的文件数量
    pub files_count: usize,
    /// 代码行数
    pub lines_count: usize,
    /// 变更行数
    pub changed_lines_count: usize,
    /// 按严重程度统计的问题数量
    pub issues_by_severity: HashMap<Severity, usize>,
    /// 按类别统计的问题数量
    pub issues_by_category: HashMap<RuleCategory, usize>,
    /// 总体评分（0-100）
    pub score: Option<u8>,
    /// 总体评价
    pub overview: String,
}

/// 评审结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// 发现的问题列表
    pub issues: Vec<Issue>,
    /// 评审结果摘要
    pub summary: ReviewSummary,
    /// 按文件分组的问题
    pub issues_by_file: HashMap<String, Vec<Issue>>,
}

/// 规则上下文
pub struct RuleContext<'a> {
    /// 分析的差异
    pub diff: &'a GitDiff,
    /// 原始文件内容（如果可用）
    pub file_contents: HashMap<String, String>,
    /// 工作目录
    pub work_dir: PathBuf,
    /// 分析深度
    pub depth: AnalysisDepth,
    /// 关注领域
    pub focus: Vec<RuleCategory>,
}

/// 分析深度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnalysisDepth {
    /// 基础分析
    Basic,
    /// 标准分析
    Normal,
    /// 深度分析
    Deep,
}

/// 规则接口
pub trait Rule {
    /// 获取规则名称
    fn name(&self) -> &str;
    /// 获取规则类别
    fn category(&self) -> RuleCategory;
    /// 应用规则
    fn apply(&self, context: &RuleContext) -> Vec<Issue>;
    /// 获取规则严重程度
    fn severity(&self) -> Severity;
    /// 判断规则是否适用于特定语言
    fn is_applicable(&self, language: &str) -> bool;
}

/// 规则引擎
pub struct RuleEngine {
    /// 规则集合
    pub rule_sets: HashMap<RuleCategory, Vec<Box<dyn Rule>>>,
    /// 配置
    pub config: RuleConfig,
}

/// 规则配置
pub struct RuleConfig {
    /// 启用的类别
    pub enabled_categories: Vec<RuleCategory>,
    /// 分析深度
    pub depth: AnalysisDepth,
    /// 自定义规则路径
    pub custom_rules_path: Option<PathBuf>,
    /// 忽略的规则
    pub ignore_rules: Vec<String>,
}

impl RuleEngine {
    /// 创建新的规则引擎
    pub fn new(config: RuleConfig) -> Self {
        Self {
            rule_sets: HashMap::new(),
            config,
        }
    }
    
    /// 加载语言规则
    pub fn load_language_rules(&mut self, language: &str) -> Result<(), String> {
        // 此处将在后续实现加载特定语言的规则
        Ok(())
    }
    
    /// 应用规则
    pub fn apply_rules(&self, context: &RuleContext) -> ReviewResult {
        let mut all_issues = Vec::new();
        
        // 应用启用的类别下的所有规则
        for category in &self.config.enabled_categories {
            if let Some(rules) = self.rule_sets.get(category) {
                for rule in rules {
                    // 检查是否在忽略列表中
                    if self.config.ignore_rules.contains(&rule.name().to_string()) {
                        continue;
                    }
                    
                    // 应用规则并收集问题
                    let issues = rule.apply(context);
                    all_issues.extend(issues);
                }
            }
        }
        
        // 构建按文件分组的问题映射
        let mut issues_by_file = HashMap::new();
        for issue in &all_issues {
            issues_by_file
                .entry(issue.location.file_path.clone())
                .or_insert_with(Vec::new)
                .push(issue.clone());
        }
        
        // 构建摘要
        let mut issues_by_severity = HashMap::new();
        let mut issues_by_category = HashMap::new();
        
        for issue in &all_issues {
            *issues_by_severity.entry(issue.severity).or_insert(0) += 1;
            *issues_by_category.entry(issue.category).or_insert(0) += 1;
        }
        
        // 计算评分和概述
        let (score, overview) = self.calculate_score_and_overview(&all_issues);
        
        // 构建评审结果
        ReviewResult {
            issues: all_issues,
            summary: ReviewSummary {
                files_count: issues_by_file.len(),
                lines_count: context.diff.total_lines(),
                changed_lines_count: context.diff.changed_lines(),
                issues_by_severity,
                issues_by_category,
                score,
                overview,
            },
            issues_by_file,
        }
    }
    
    /// 计算评分和概述
    fn calculate_score_and_overview(&self, issues: &[Issue]) -> (Option<u8>, String) {
        if issues.is_empty() {
            return (Some(100), "代码质量优秀，未发现问题。".to_string());
        }
        
        // 按严重程度计算分数
        let error_count = issues.iter().filter(|i| i.severity == Severity::Error).count();
        let warning_count = issues.iter().filter(|i| i.severity == Severity::Warning).count();
        let info_count = issues.iter().filter(|i| i.severity == Severity::Info).count();
        
        // 简单评分算法（可以根据需要调整）
        let base_score = 100;
        let error_penalty = error_count as u8 * 15;
        let warning_penalty = warning_count as u8 * 5;
        let info_penalty = info_count as u8 * 1;
        
        let total_penalty = error_penalty.saturating_add(warning_penalty).saturating_add(info_penalty);
        let score = base_score.saturating_sub(total_penalty);
        
        // 生成概述
        let overview = if error_count > 0 {
            format!("发现 {} 个严重问题，需要优先解决。", error_count)
        } else if warning_count > 0 {
            format!("发现 {} 个警告，建议修复。", warning_count)
        } else {
            format!("代码质量良好，有 {} 个小问题可以改进。", info_count)
        };
        
        (Some(score), overview)
    }
}

/// GitDiff 扩展方法
impl GitDiff {
    /// 统计总行数
    pub fn total_lines(&self) -> usize {
        let mut count = 0;
        for file in &self.changed_files {
            for hunk in &file.hunks {
                count += hunk.lines.len();
            }
        }
        count
    }
    
    /// 统计变更行数
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