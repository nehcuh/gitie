//! 代码评审模块
//! 
//! 该模块提供了基本的代码评审功能，包括简化的评审结果数据结构。

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::tree_sitter_analyzer::core::GitDiff;
use crate::config_management::settings::TreeSitterConfig;

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

/// 简化的评审结果
#[derive(Debug, Clone)]
pub struct SimpleReviewResult {
    /// 评审标题
    pub title: String,
    /// 评审内容
    pub content: String,
    /// 严重程度
    pub severity: Severity,
}

/// 简单代码评审器
pub struct SimpleReviewer {
    /// 配置
    pub config: TreeSitterConfig,
}

impl SimpleReviewer {
    /// 创建新的代码评审器
    pub fn new(config: TreeSitterConfig) -> Self {
        Self { config }
    }
    
    /// 执行简单评审
    pub fn review(&self, diff: &GitDiff) -> Vec<SimpleReviewResult> {
        let mut results = Vec::new();
        
        // 简单检查硬编码的凭据
        for file in &diff.changed_files {
            for hunk in &file.hunks {
                for line in &hunk.lines {
                    if line.starts_with('+') && (
                        line.contains("password") || 
                        line.contains("secret") || 
                        line.contains("token") || 
                        line.contains("api_key")
                    ) {
                        results.push(SimpleReviewResult {
                            title: "检测到硬编码凭证".to_string(),
                            content: "代码中可能包含硬编码的敏感信息，建议使用环境变量或配置文件存储".to_string(),
                            severity: Severity::Error,
                        });
                    }
                    
                    // 检查长行
                    if line.starts_with('+') && line.len() > 100 {
                        results.push(SimpleReviewResult {
                            title: "行长度过长".to_string(),
                            content: "检测到长度超过100字符的行，建议拆分以提高可读性".to_string(),
                            severity: Severity::Info,
                        });
                    }
                }
            }
        }
        
        // 如果没有发现问题，添加一个积极的反馈
        if results.is_empty() {
            results.push(SimpleReviewResult {
                title: "代码质量良好".to_string(),
                content: "未发现明显问题，代码质量良好".to_string(),
                severity: Severity::Info,
            });
        }
        
        results
    }
}