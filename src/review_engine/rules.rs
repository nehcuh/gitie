//! 代码评审规则集合
//! 
//! 本模块提供了各种代码评审规则的实现，包括通用规则和特定于语言的规则。

use crate::review_engine::{Rule, RuleCategory, Severity, Issue, RuleContext, CodeLocation};
use std::collections::HashMap;

// Language-specific modules to be implemented
// These will contain language-specific rules

/// 规则工厂，负责创建和管理规则
pub struct RuleFactory;

impl RuleFactory {
    /// 创建全部规则集
    pub fn create_all_rules() -> HashMap<RuleCategory, Vec<Box<dyn Rule>>> {
        let mut rule_sets = HashMap::new();
        
        // 添加风格规则
        rule_sets.insert(RuleCategory::Style, Self::create_style_rules());
        
        // 添加安全规则
        rule_sets.insert(RuleCategory::Security, Self::create_security_rules());
        
        // 添加性能规则
        rule_sets.insert(RuleCategory::Performance, Self::create_performance_rules());
        
        // 添加复杂度规则
        rule_sets.insert(RuleCategory::Complexity, Self::create_complexity_rules());
        
        // 添加最佳实践规则
        rule_sets.insert(RuleCategory::BestPractices, Self::create_best_practices_rules());
        
        // 添加Bug检测规则
        rule_sets.insert(RuleCategory::Bugs, Self::create_bug_rules());
        
        rule_sets
    }
    
    /// 创建指定语言的规则集
    pub fn create_language_rules(language: &str) -> Vec<Box<dyn Rule>> {
        let mut rules = Vec::new();
        
        // 创建基本的通用规则
        rules.extend(Self::create_style_rules());
        rules.extend(Self::create_security_rules());
        
        // 为特定语言添加更多规则
        match language.to_lowercase().as_str() {
            "rust" => {
                // TODO: 实现Rust特定规则
            },
            "python" => {
                // TODO: 实现Python特定规则
            },
            "java" => {
                // TODO: 实现Java特定规则
            },
            "cpp" | "c++" | "c" => {
                // TODO: 实现C/C++特定规则
            },
            "javascript" | "js" | "typescript" | "ts" => {
                // TODO: 实现JavaScript特定规则
            },
            "go" => {
                // TODO: 实现Go特定规则
            },
            _ => {
                // 对于未知语言，使用已有通用规则
            }
        }
        
        rules
    }
    
    /// 创建风格规则
    fn create_style_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(common::LineLength { 
                max_length: 100,
                severity: Severity::Info,
            }),
            Box::new(common::ConsistentIndentation { 
                severity: Severity::Info,
            }),
            Box::new(common::NamingConvention { 
                severity: Severity::Info,
            }),
        ]
    }
    
    /// 创建安全规则
    fn create_security_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(common::HardcodedCredentials { 
                severity: Severity::Error,
            }),
            Box::new(common::UnsafeInput { 
                severity: Severity::Warning,
            }),
        ]
    }
    
    /// 创建性能规则
    fn create_performance_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(common::InefficientAlgorithm { 
                severity: Severity::Warning,
            }),
            Box::new(common::ResourceLeak { 
                severity: Severity::Warning,
            }),
        ]
    }
    
    /// 创建复杂度规则
    fn create_complexity_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(common::LongFunction { 
                max_lines: 50,
                severity: Severity::Info,
            }),
            Box::new(common::ComplexCondition { 
                max_logical_ops: 3,
                severity: Severity::Warning,
            }),
        ]
    }
    
    /// 创建最佳实践规则
    fn create_best_practices_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(common::MagicNumber { 
                severity: Severity::Info,
            }),
            Box::new(common::MissingComments { 
                severity: Severity::Info,
            }),
        ]
    }
    
    /// 创建Bug检测规则
    fn create_bug_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(common::DeadCode { 
                severity: Severity::Warning,
            }),
            Box::new(common::UnusedVariable { 
                severity: Severity::Warning,
            }),
        ]
    }
}

/// 通用规则模块
pub mod common {
    use super::*;
    use regex::Regex;
    use lazy_static::lazy_static;
    
    /// 创建所有通用规则
    pub fn create_common_rules() -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(LineLength { max_length: 100, severity: Severity::Info }),
            Box::new(ConsistentIndentation { severity: Severity::Info }),
            Box::new(NamingConvention { severity: Severity::Info }),
            Box::new(LongFunction { max_lines: 50, severity: Severity::Info }),
            Box::new(ComplexCondition { max_logical_ops: 3, severity: Severity::Warning }),
            Box::new(MagicNumber { severity: Severity::Info }),
            Box::new(MissingComments { severity: Severity::Info }),
            Box::new(DeadCode { severity: Severity::Warning }),
            Box::new(UnusedVariable { severity: Severity::Warning }),
            Box::new(HardcodedCredentials { severity: Severity::Error }),
            Box::new(UnsafeInput { severity: Severity::Warning }),
            Box::new(InefficientAlgorithm { severity: Severity::Warning }),
            Box::new(ResourceLeak { severity: Severity::Warning }),
        ]
    }
    
    /// 行长度检查规则
    pub struct LineLength {
        pub max_length: usize,
        pub severity: Severity,
    }
    
    impl Rule for LineLength {
        fn name(&self) -> &str {
            "line-length"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Style
        }
        
        fn apply(&self, context: &RuleContext) -> Vec<Issue> {
            let mut issues = Vec::new();
            
            for file in &context.diff.changed_files {
                let mut line_number = 0;
                for hunk in &file.hunks {
                    for line in &hunk.lines {
                        line_number += 1;
                        
                        // 只检查新增或修改的行
                        if line.starts_with('+') && line.len() > self.max_length {
                            issues.push(Issue {
                                id: format!("{}:{}", self.name(), issues.len() + 1),
                                title: format!("行长度超过 {} 字符", self.max_length),
                                description: format!("该行长度为 {} 字符，超过了推荐的最大长度 {} 字符", line.len(), self.max_length),
                                location: CodeLocation {
                                    file_path: file.path.to_string_lossy().to_string(),
                                    start_line: line_number,
                                    end_line: line_number,
                                    start_column: None,
                                    end_column: None,
                                },
                                severity: self.severity,
                                category: self.category(),
                                code_snippet: Some(line.clone()),
                                suggestion: Some(format!("考虑将该行拆分为多行，保持每行不超过 {} 字符", self.max_length)),
                                explanation: None,
                            });
                        }
                    }
                }
            }
            
            issues
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true // 适用于所有语言
        }
    }
    
    /// 缩进一致性检查
    pub struct ConsistentIndentation {
        pub severity: Severity,
    }
    
    impl Rule for ConsistentIndentation {
        fn name(&self) -> &str {
            "consistent-indentation"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Style
        }
        
        fn apply(&self, context: &RuleContext) -> Vec<Issue> {
            let mut issues = Vec::new();
            
            for file in &context.diff.changed_files {
                let mut detected_indentation = None;
                let mut line_number = 0;
                
                for hunk in &file.hunks {
                    for line in &hunk.lines {
                        line_number += 1;
                        
                        // 只检查新增的行
                        if line.starts_with('+') {
                            let content = &line[1..]; // 去掉 '+' 前缀
                            if !content.trim().is_empty() {
                                // 计算前导空格
                                let leading_spaces = content.len() - content.trim_start().len();
                                
                                // 检测首次缩进
                                if detected_indentation.is_none() && leading_spaces > 0 {
                                    detected_indentation = Some(leading_spaces);
                                }
                                
                                // 检查缩进一致性
                                if let Some(standard_indent) = detected_indentation {
                                    if leading_spaces > 0 && leading_spaces % standard_indent != 0 {
                                        issues.push(Issue {
                                            id: format!("{}:{}", self.name(), issues.len() + 1),
                                            title: "缩进不一致".to_string(),
                                            description: format!("检测到首选缩进为 {} 空格，但该行使用了 {} 空格", 
                                                standard_indent, leading_spaces),
                                            location: CodeLocation {
                                                file_path: file.path.to_string_lossy().to_string(),
                                                start_line: line_number,
                                                end_line: line_number,
                                                start_column: None,
                                                end_column: None,
                                            },
                                            severity: self.severity,
                                            category: self.category(),
                                            code_snippet: Some(line.clone()),
                                            suggestion: Some(format!("使用一致的缩进（{}空格的倍数）", standard_indent)),
                                            explanation: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            issues
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true // 适用于所有语言
        }
    }
    
    /// 命名规范检查
    pub struct NamingConvention {
        pub severity: Severity,
    }
    
    impl Rule for NamingConvention {
        fn name(&self) -> &str {
            "naming-convention"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Style
        }
        
        fn apply(&self, context: &RuleContext) -> Vec<Issue> {
            // 由于命名规范检查需要更深入的语法分析，实际实现会更复杂
            // 这里提供一个简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 函数长度检查
    pub struct LongFunction {
        pub max_lines: usize,
        pub severity: Severity,
    }
    
    impl Rule for LongFunction {
        fn name(&self) -> &str {
            "long-function"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Complexity
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 复杂条件检查
    pub struct ComplexCondition {
        pub max_logical_ops: usize,
        pub severity: Severity,
    }
    
    impl Rule for ComplexCondition {
        fn name(&self) -> &str {
            "complex-condition"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Complexity
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 魔法数字检查
    pub struct MagicNumber {
        pub severity: Severity,
    }
    
    impl Rule for MagicNumber {
        fn name(&self) -> &str {
            "magic-number"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::BestPractices
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 缺少注释检查
    pub struct MissingComments {
        pub severity: Severity,
    }
    
    impl Rule for MissingComments {
        fn name(&self) -> &str {
            "missing-comments"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::BestPractices
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 死代码检查
    pub struct DeadCode {
        pub severity: Severity,
    }
    
    impl Rule for DeadCode {
        fn name(&self) -> &str {
            "dead-code"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Bugs
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 未使用变量检查
    pub struct UnusedVariable {
        pub severity: Severity,
    }
    
    impl Rule for UnusedVariable {
        fn name(&self) -> &str {
            "unused-variable"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Bugs
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 硬编码凭证检查
    pub struct HardcodedCredentials {
        pub severity: Severity,
    }
    
    impl Rule for HardcodedCredentials {
        fn name(&self) -> &str {
            "hardcoded-credentials"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Security
        }
        
        fn apply(&self, context: &RuleContext) -> Vec<Issue> {
            let mut issues = Vec::new();
            
            lazy_static! {
                static ref PASSWORD_REGEX: Regex = Regex::new(
                    r"(?i)(password|passwd|pwd|secret|key|token|api_key|apikey|access_token)\s*[=:]"
                ).unwrap();
            }
            
            for file in &context.diff.changed_files {
                let mut line_number = 0;
                for hunk in &file.hunks {
                    for line in &hunk.lines {
                        line_number += 1;
                        
                        // 只检查新增的行
                        if line.starts_with('+') {
                            let content = &line[1..]; // 去掉 '+' 前缀
                            
                            // 检查是否包含硬编码凭证
                            if PASSWORD_REGEX.is_match(content) {
                                issues.push(Issue {
                                    id: format!("{}:{}", self.name(), issues.len() + 1),
                                    title: "检测到硬编码凭证".to_string(),
                                    description: "代码中包含硬编码的密码或密钥或令牌, 这是一个安全风险".to_string(),
                                    location: CodeLocation {
                                        file_path: file.path.to_string_lossy().to_string(),
                                        start_line: line_number,
                                        end_line: line_number,
                                        start_column: None,
                                        end_column: None,
                                    },
                                    severity: self.severity,
                                    category: self.category(),
                                    code_snippet: Some(line.clone()),
                                    suggestion: Some("使用环境变量或配置文件或安全的凭证管理服务来存储敏感信息".to_string()),
                                    explanation: Some("硬编码凭证可能导致敏感信息泄露, 特别是当代码被推送到公共仓库时.".to_string()),
                                });
                            }
                        }
                    }
                }
            }
            
            issues
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 不安全输入检查
    pub struct UnsafeInput {
        pub severity: Severity,
    }
    
    impl Rule for UnsafeInput {
        fn name(&self) -> &str {
            "unsafe-input"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Security
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 低效算法检查
    pub struct InefficientAlgorithm {
        pub severity: Severity,
    }
    
    impl Rule for InefficientAlgorithm {
        fn name(&self) -> &str {
            "inefficient-algorithm"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Performance
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
    
    /// 资源泄露检查
    pub struct ResourceLeak {
        pub severity: Severity,
    }
    
    impl Rule for ResourceLeak {
        fn name(&self) -> &str {
            "resource-leak"
        }
        
        fn category(&self) -> RuleCategory {
            RuleCategory::Performance
        }
        
        fn apply(&self, _context: &RuleContext) -> Vec<Issue> {
            // 需要语法分析支持，简化版本
            Vec::new()
        }
        
        fn severity(&self) -> Severity {
            self.severity
        }
        
        fn is_applicable(&self, _language: &str) -> bool {
            true
        }
    }
}