# Tree-Sitter 代码分析技术设计

## 1. 概述

### 1.1 背景

Gitie 的核心功能之一是自动生成 commit 信息，为了提高生成信息的质量，我们需要深入理解代码结构和变更的语义。Tree-sitter 作为一个通用的语法解析库，提供了高效的代码解析和查询功能，能够帮助我们分析代码变更的结构和上下文。

### 1.2 设计目标

1. 使用 Tree-sitter 库解析多种编程语言的代码结构
2. 分析 Git diff 中被修改的代码片段
3. 识别变更的函数、类、方法等高级结构
4. 为 AI 提供结构化的代码变更信息，提高提交信息生成质量
5. 设计可扩展架构，便于未来支持更多语言

## 2. 系统架构

### 2.1 组件概览

Tree-sitter 分析器由以下主要组件组成：

```
tree_sitter_analyzer/
├── mod.rs       # 模块定义和公共接口
├── analyzer.rs  # 主分析逻辑和 Tree-sitter 集成
├── core.rs      # 核心数据结构和分析工具
├── rust.rs      # Rust 语言特定分析
├── java.rs      # Java 语言特定分析
└── python.rs    # Python 语言特定分析（未来扩展）
```

### 2.2 组件交互

```
                  ┌───────────────────┐
                  │  command_processing │
                  │     (commit.rs)    │
                  └─────────┬─────────┘
                            │ 请求分析
                            ▼
┌───────────────────────────────────────────┐
│         TreeSitterAnalyzer (analyzer.rs)  │
└───┬────────────────┬─────────────┬────────┘
    │                │             │
    ▼                ▼             ▼
┌─────────┐     ┌─────────┐   ┌─────────┐
│ rust.rs │     │ java.rs │   │python.rs│
└─────────┘     └─────────┘   └─────────┘
```

## 3. 核心数据结构

### 3.1 文件抽象语法树

```rust
pub struct FileAst {
    pub path: PathBuf,              // 文件路径
    pub tree: Tree,                 // Tree-sitter 解析树
    pub source: String,             // 源代码
    pub content_hash: String,       // 内容哈希（用于缓存）
    pub last_parsed: SystemTime,    // 最后解析时间
    pub language_id: String,        // 语言标识符
}
```

### 3.2 Git Diff 表示

```rust
pub struct GitDiff {
    pub changed_files: Vec<FileDiff>,  // 修改的文件列表
}

pub struct FileDiff {
    pub path: PathBuf,                 // 文件路径
    pub old_path: Option<PathBuf>,     // 重命名前的路径（如果有）
    pub change_type: ChangeType,       // 变更类型（新增、修改、删除等）
    pub hunks: Vec<DiffHunk>,          // 变更块
}

pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
}
```

### 3.3 代码结构表示

```rust
pub struct AffectedNode {
    pub node_type: String,          // 节点类型（函数、类、方法等）
    pub name: String,               // 节点名称
    pub range: (usize, usize),      // 在源码中的位置范围
    pub is_public: bool,            // 是否是公共可见的
    pub content: Option<String>,    // 节点内容
    pub line_range: (usize, usize), // 行范围
}

pub struct FileAnalysis {
    pub path: PathBuf,              // 文件路径
    pub language: String,           // 编程语言
    pub change_type: ChangeType,    // 变更类型
    pub affected_nodes: Vec<AffectedNode>, // 受影响的代码节点
    pub summary: Option<String>,    // 变更摘要
}

pub struct DiffAnalysis {
    pub file_analyses: Vec<FileAnalysis>,  // 文件分析结果
    pub overall_summary: String,           // 整体摘要
    pub change_analysis: ChangeAnalysis,   // 变更分析结果
}
```

## 4. 关键功能实现

### 4.1 语言检测与解析

```rust
impl TreeSitterAnalyzer {
    /// 根据文件扩展名检测编程语言
    pub fn detect_language(&self, path: &Path) -> Result<String, TreeSitterError> {
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
            
        match extension {
            "rs" => Ok("rust".to_string()),
            "java" => Ok("java".to_string()),
            "py" | "pyi" => Ok("python".to_string()),
            "go" => Ok("go".to_string()),
            // 添加更多语言支持
            _ => Err(TreeSitterError::UnsupportedLanguage(
                format!("Unsupported file extension: {}", extension)
            ))
        }
    }
    
    /// 解析文件生成语法树
    pub fn parse_file(&mut self, file_path: &Path) -> Result<FileAst, TreeSitterError> {
        // 检测语言
        let lang_id = self.detect_language(file_path)?;
        
        // 获取对应的 Tree-sitter 语言
        let language = self.languages.get(&lang_id)
            .ok_or_else(|| TreeSitterError::UnsupportedLanguage(
                format!("Language '{}' not initialized", lang_id)
            ))?;
            
        // 读取文件内容
        let source_code = fs::read_to_string(file_path)
            .map_err(|e| TreeSitterError::IoError(e))?;
            
        // 计算内容哈希
        let content_hash = calculate_hash(&source_code);
        
        // 构建解析器并解析
        let mut parser = Parser::new();
        parser.set_language(*language)
            .map_err(|e| TreeSitterError::ParseError(
                format!("Failed to set language: {}", e)
            ))?;
            
        let tree = parser.parse(&source_code, None)
            .ok_or_else(|| TreeSitterError::ParseError(
                format!("Failed to parse file: {}", file_path.display())
            ))?;
            
        // 返回文件 AST
        Ok(FileAst {
            path: file_path.to_path_buf(),
            tree,
            source: source_code,
            content_hash,
            last_parsed: SystemTime::now(),
            language_id: lang_id,
        })
    }
}
```

### 4.2 Git Diff 解析

```rust
/// 解析 git diff 输出
pub fn parse_git_diff(diff_text: &str) -> Result<GitDiff, TreeSitterError> {
    let mut changed_files = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut current_hunks = Vec::new();
    
    // 解析 git diff 输出的每一行
    for line in diff_text.lines() {
        if line.starts_with("diff --git ") {
            // 处理新文件
            if let Some(file) = current_file.take() {
                changed_files.push(file);
            }
            
            // 初始化新文件差异
            current_hunks = Vec::new();
            
            // 解析文件路径
            // ...
        }
        else if line.starts_with("--- ") || line.starts_with("+++ ") {
            // 处理文件元数据
            // ...
        }
        else if line.starts_with("@@ ") {
            // 解析变更块（hunk）头部
            // ...
            
            // 添加新的变更块
            current_hunks.push(DiffHunk {
                old_range,
                new_range,
                lines: Vec::new(),
            });
        }
        else if let Some(hunk) = current_hunks.last_mut() {
            // 添加变更行到当前变更块
            hunk.lines.push(line.to_string());
        }
    }
    
    // 处理最后一个文件
    if let Some(file) = current_file {
        changed_files.push(file);
    }
    
    Ok(GitDiff { changed_files })
}
```

### 4.3 代码结构分析

```rust
impl TreeSitterAnalyzer {
    /// 分析 diff 并返回结构化信息
    pub fn analyze_diff(&mut self, diff_text: &str) -> Result<DiffAnalysis, TreeSitterError> {
        // 解析 diff
        let git_diff = parse_git_diff(diff_text)?;
        
        // 分析每个变更的文件
        let mut file_analyses = Vec::new();
        
        for file_diff in &git_diff.changed_files {
            // 跳过删除的文件
            if file_diff.change_type == ChangeType::Deleted {
                // 简单记录删除
                file_analyses.push(FileAnalysis {
                    path: file_diff.path.clone(),
                    language: "unknown".to_string(),
                    change_type: ChangeType::Deleted,
                    affected_nodes: Vec::new(),
                    summary: Some(format!("Deleted file {}", file_diff.path.display())),
                });
                continue;
            }
            
            // 解析文件
            let file_path = if let Some(root) = &self.project_root {
                root.join(&file_diff.path)
            } else {
                file_diff.path.clone()
            };
            
            // 尝试解析文件（如果存在）
            match self.parse_file(&file_path) {
                Ok(file_ast) => {
                    // 查找受影响的代码节点
                    let affected_nodes = self.find_affected_nodes(&file_ast, file_diff)?;
                    
                    // 创建文件分析结果
                    file_analyses.push(FileAnalysis {
                        path: file_diff.path.clone(),
                        language: file_ast.language_id.clone(),
                        change_type: file_diff.change_type.clone(),
                        affected_nodes,
                        summary: None, // 将在后续步骤生成
                    });
                },
                Err(e) => {
                    // 记录解析错误
                    file_analyses.push(FileAnalysis {
                        path: file_diff.path.clone(),
                        language: "unknown".to_string(),
                        change_type: file_diff.change_type.clone(),
                        affected_nodes: Vec::new(),
                        summary: Some(format!("Failed to analyze: {}", e)),
                    });
                }
            }
        }
        
        // 生成整体摘要
        let overall_summary = generate_overall_summary(&file_analyses);
        
        // 分析变更特性
        let change_analysis = analyze_changes(&file_analyses);
        
        Ok(DiffAnalysis {
            file_analyses,
            overall_summary,
            change_analysis,
        })
    }
    
    /// 查找受影响的代码节点
    fn find_affected_nodes(&self, file_ast: &FileAst, file_diff: &FileDiff) 
        -> Result<Vec<AffectedNode>, TreeSitterError> {
        
        // 根据语言选择合适的分析方法
        match file_ast.language_id.as_str() {
            "rust" => self.find_affected_rust_nodes(file_ast, file_diff),
            "java" => self.find_affected_java_nodes(file_ast, file_diff),
            "python" => self.find_affected_python_nodes(file_ast, file_diff),
            "go" => self.find_affected_go_nodes(file_ast, file_diff),
            _ => Err(TreeSitterError::UnsupportedLanguage(
                format!("Language '{}' analysis not implemented", file_ast.language_id)
            )),
        }
    }
}
```

### 4.4 语言特定分析

以 Rust 为例：

```rust
impl TreeSitterAnalyzer {
    /// 查找受影响的 Rust 代码节点
    fn find_affected_rust_nodes(&self, file_ast: &FileAst, file_diff: &FileDiff) 
        -> Result<Vec<AffectedNode>, TreeSitterError> {
        
        let mut affected_nodes = Vec::new();
        
        // 构建函数和结构体查询
        let query_str = r#"
            (function_item name: (identifier) @function.name) @function.definition
            (struct_item name: (identifier) @struct.name) @struct.definition
            (impl_item) @impl.definition
            (trait_item name: (identifier) @trait.name) @trait.definition
            (mod_item name: (identifier) @module.name) @module.definition
        "#;
        
        let query = Query::new(tree_sitter_rust::language(), query_str)
            .map_err(|e| TreeSitterError::QueryError(format!("Failed to create Rust query: {}", e)))?;
            
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, file_ast.tree.root_node(), file_ast.source.as_bytes());
        
        // 处理受影响的行
        let affected_lines = self.get_affected_lines(file_diff);
        
        // 遍历查询结果
        for m in matches {
            for capture in m.captures {
                let capture_name = &query.capture_names()[capture.index as usize];
                
                // 检查节点是否与变更行重叠
                if self.node_overlaps_affected_lines(&capture.node, &affected_lines) {
                    // 根据节点类型创建 AffectedNode
                    if capture_name.ends_with(".definition") {
                        let node_type = match capture_name.as_str() {
                            "function.definition" => "function",
                            "struct.definition" => "struct",
                            "impl.definition" => "impl",
                            "trait.definition" => "trait",
                            "module.definition" => "module",
                            _ => "unknown",
                        };
                        
                        // 获取节点名称
                        let name = self.get_rust_node_name(&capture.node, file_ast, node_type);
                        
                        // 检查是否是公有的
                        let is_public = self.is_rust_node_public(&capture.node, file_ast);
                        
                        // 添加到受影响节点列表
                        affected_nodes.push(AffectedNode {
                            node_type: node_type.to_string(),
                            name,
                            range: (capture.node.start_byte(), capture.node.end_byte()),
                            is_public,
                            content: Some(capture.node.utf8_text(file_ast.source.as_bytes())
                                .unwrap_or("").to_string()),
                            line_range: (
                                capture.node.start_position().row,
                                capture.node.end_position().row,
                            ),
                        });
                    }
                }
            }
        }
        
        Ok(affected_nodes)
    }
}
```

## 5. 多语言支持扩展

### 5.1 语言支持接口

为了便于添加新语言支持，我们定义了标准接口：

```rust
trait LanguageAnalyzer {
    /// 执行语言特定的代码分析
    fn analyze_file(&self, file_ast: &FileAst) -> Result<Vec<AffectedNode>, TreeSitterError>;
    
    /// 查找受影响的代码节点
    fn find_affected_nodes(&self, file_ast: &FileAst, file_diff: &FileDiff) 
        -> Result<Vec<AffectedNode>, TreeSitterError>;
        
    /// 检查节点可见性
    fn is_node_public(&self, node: &Node, file_ast: &FileAst) -> bool;
    
    /// 获取查询模式
    fn get_query_pattern(&self) -> &'static str;
}
```

### 5.2 添加新语言步骤

1. 添加 Tree-sitter 语法库依赖（例如 `tree-sitter-python`）
2. 创建新的语言特定模块（例如 `python.rs`）
3. 实现 `LanguageAnalyzer` trait
4. 在 `TreeSitterAnalyzer` 中集成新语言支持
5. 更新语言检测逻辑

## 6. 性能优化

### 6.1 语法树缓存

为了避免重复解析，我们实现了文件 AST 的缓存：

```rust
impl TreeSitterAnalyzer {
    // 检查文件是否需要重新解析
    fn is_cache_valid(&self, path: &Path, current_hash: &str) -> bool {
        if !self.config.cache_enabled {
            return false;
        }
        
        match self.file_asts.get(path) {
            Some(cached_ast) => cached_ast.content_hash == current_hash,
            None => false,
        }
    }
    
    // 从缓存获取或解析文件
    fn get_or_parse_file(&mut self, path: &Path) -> Result<FileAst, TreeSitterError> {
        // 读取文件内容并计算哈希
        let source_code = fs::read_to_string(path)?;
        let content_hash = calculate_hash(&source_code);
        
        // 检查缓存
        if self.is_cache_valid(path, &content_hash) {
            if let Some(cached_ast) = self.file_asts.get(path) {
                return Ok(cached_ast.clone());
            }
        }
        
        // 缓存无效，重新解析
        let ast = self.parse_file(path)?;
        
        // 更新缓存
        if self.config.cache_enabled {
            self.file_asts.insert(path.to_path_buf(), ast.clone());
        }
        
        Ok(ast)
    }
}
```

### 6.2 增量分析

对于大型项目，我们实现了增量分析策略：

1. 只解析被修改的文件
2. 缓存解析结果，避免重复解析
3. 使用变更行信息，只分析变更部分的代码结构

## 7. 与 AI 集成

### 7.1 生成结构化提示

```rust
pub fn generate_commit_prompt(diff_analysis: &DiffAnalysis) -> String {
    let mut prompt = String::new();
    
    // 添加基础提示
    prompt.push_str("Please analyze the following code changes and generate a commit message.\n\n");
    
    // 添加整体摘要
    prompt.push_str(&format!("# Overall Changes Summary\n{}\n\n", diff_analysis.overall_summary));
    
    // 添加变更文件信息
    prompt.push_str("# Changed Files\n");
    for analysis in &diff_analysis.file_analyses {
        prompt.push_str(&format!("- {} ({}): ", 
            analysis.path.display(), 
            analysis.change_type.to_string()));
            
        if let Some(summary) = &analysis.summary {
            prompt.push_str(summary);
        } else {
            prompt.push_str(&format!("{} affected nodes", analysis.affected_nodes.len()));
        }
        prompt.push_str("\n");
    }
    prompt.push_str("\n");
    
    // 添加代码结构变更信息
    prompt.push_str("# Affected Code Structures\n");
    for analysis in &diff_analysis.file_analyses {
        if analysis.affected_nodes.is_empty() {
            continue;
        }
        
        prompt.push_str(&format!("## In {}\n", analysis.path.display()));
        
        for node in &analysis.affected_nodes {
            let visibility = if node.is_public { "public" } else { "private" };
            prompt.push_str(&format!("- {} {} `{}`\n", 
                visibility, node.node_type, node.name));
        }
        prompt.push_str("\n");
    }
    
    // 添加变更分析信息
    prompt.push_str("# Change Analysis\n");
    prompt.push_str(&format!("- Change pattern: {}\n", diff_analysis.change_analysis.change_pattern.to_string()));
    prompt.push_str(&format!("- Change scope: {}\n", diff_analysis.change_analysis.change_scope.to_string()));
    prompt.push_str(&format!("- Function changes: {}\n", diff_analysis.change_analysis.function_changes));
    prompt.push_str(&format!("- Type changes: {}\n", diff_analysis.change_analysis.type_changes));
    prompt.push_str(&format!("- Method changes: {}\n", diff_analysis.change_analysis.method_changes));
    prompt.push_str(&format!("- Interface changes: {}\n", diff_analysis.change_analysis.interface_changes));
    
    prompt
}
```

### 7.2 集成流程

Tree-sitter 分析器在 commit 流程中的集成：

1. 从 `git diff --staged` 获取变更内容
2. 使用 `TreeSitterAnalyzer` 分析变更
3. 生成结构化提示，包含代码结构信息
4. 将提示发送给 AI 模型
5. 处理 AI 响应，生成最终的提交信息

## 8. 测试策略

### 8.1 单元测试

针对不同组件的单元测试：

1. 语言检测和解析测试
2. Git diff 解析测试
3. 代码结构分析测试
4. 语言特定节点识别测试

### 8.2 集成测试

端到端测试场景：

1. 模拟 Git 操作和代码变更
2. 执行完整分析流程
3. 验证分析结果

## 9. 总结

Tree-sitter 代码分析模块为 Gitie 提供了深入理解代码结构的能力，大大增强了 AI 生成的提交信息质量。通过模块化设计，系统支持多种编程语言，并且容易扩展以支持更多语言。性能优化措施确保了即使在大型项目中也能高效运行。

未来的改进方向包括：
1. 扩展支持更多编程语言
2. 增强语义分析能力
3. 改进变更模式识别
4. 优化大型项目的性能