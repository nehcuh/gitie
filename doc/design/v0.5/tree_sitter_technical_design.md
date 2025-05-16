# Gitie Tree-sitter 集成技术设计

## 1. 技术架构概述

### 1.1 系统架构

Gitie的Tree-sitter集成架构由以下核心组件组成：

```
┌─────────────────────────────────┐
│           用户界面层            │
│   (命令行界面 / 可能的 GUI)     │
└───────────────┬─────────────────┘
                │
┌───────────────▼─────────────────┐
│           核心逻辑层            │
├─────────────────────────────────┤
│  ┌─────────────┐ ┌────────────┐ │
│  │Git 交互模块 │ │配置管理模块│ │
│  └──────┬──────┘ └────────────┘ │
│         │                       │
│  ┌──────▼──────┐ ┌────────────┐ │
│  │Diff 解析模块│ │ AI 集成模块│ │
│  └──────┬──────┘ └─────┬──────┘ │
│         │              │        │
│  ┌──────▼──────────────▼──────┐ │
│  │      语法树分析模块        │ │
│  └───────────────────────────┬┘ │
└───────────────┬───────────────┘
                │
┌───────────────▼─────────────────┐
│            集成层               │
├─────────────────────────────────┤
│ ┌───────────┐  ┌───────────────┐│
│ │Tree-sitter│  │LLM API 客户端 ││
│ └───────────┘  └───────────────┘│
└─────────────────────────────────┘
```

### 1.2 技术栈选择

- **主要语言**: Rust
- **语法树解析**: Tree-sitter
- **语言绑定**: rust-tree-sitter
- **Git 交互**: Git2 Rust 库 + 命令行调用
- **AI 集成**: Reqwest HTTP 客户端
- **配置管理**: TOML 配置
- **缓存机制**: SQLite 或文件系统缓存
- **测试框架**: Rust 内置测试框架

## 2. 核心组件设计

### 2.1 Tree-Sitter 集成模块

#### 2.1.1 语言解析器管理

```rust
/// 管理各种编程语言的 Tree-Sitter 解析器
pub struct LanguageManager {
    /// 已加载的语言解析器映射
    languages: HashMap<String, Language>,
    /// 语言检测策略
    detector: LanguageDetector,
    /// 预编译的查询映射
    queries: HashMap<String, PrecompiledQuery>,
}

impl LanguageManager {
    /// 创建新的语言管理器实例
    pub fn new() -> Self { /* ... */ }
    
    /// 根据文件扩展名或路径识别语言
    pub fn detect_language(&self, file_path: &Path) -> Option<&Language> { /* ... */ }
    
    /// 动态加载语言解析器
    pub fn load_language(&mut self, language_id: &str) -> Result<(), TreeSitterError> { /* ... */ }
    
    /// 获取预编译查询
    pub fn get_query(&self, language_id: &str, query_type: &str) -> Option<&Query> { /* ... */ }
    
    /// 预编译并缓存查询
    pub fn precompile_query(
        &mut self, 
        language_id: &str, 
        query_type: &str, 
        query_string: &str
    ) -> Result<(), QueryError> { /* ... */ }
}

/// 预编译查询封装
struct PrecompiledQuery {
    query: Query,
    creation_time: SystemTime,
}
```

#### 2.1.2 项目语法树缓存

```rust
/// 管理整个项目的语法树缓存
pub struct ProjectAst {
    /// 文件路径到AST的映射
    file_asts: HashMap<PathBuf, FileAst>,
    /// 上次解析时间戳
    last_update: SystemTime,
    /// 项目根目录
    root_path: PathBuf,
    /// 缓存设置
    cache_config: CacheConfig,
}

/// 单个文件的AST表示
pub struct FileAst {
    /// Tree-sitter解析树
    tree: Tree,
    /// 文件内容
    source: String,
    /// 文件哈希，用于检测变化
    content_hash: String,
    /// 上次解析时间
    last_parsed: SystemTime,
    /// 语言标识符
    language_id: String,
}

/// 缓存配置
pub struct CacheConfig {
    /// 最大缓存大小
    max_cache_size: usize,
    /// 过期时间
    expiry_duration: Duration,
    /// 是否使用持久化缓存
    use_persistent_cache: bool,
    /// 持久化缓存路径
    cache_path: Option<PathBuf>,
}
```

### 2.2 Git Diff 解析模块

```rust
/// Git差异信息表示
pub struct GitDiff {
    /// 变更的文件列表
    changed_files: Vec<ChangedFile>,
    /// 提交相关元数据
    metadata: CommitMetadata,
}

/// 变更文件的表示
pub struct ChangedFile {
    /// 文件路径
    path: PathBuf,
    /// 变更类型 (添加/修改/删除)
    change_type: ChangeType,
    /// 变更的行信息
    hunks: Vec<DiffHunk>,
    /// 文件模式变更
    file_mode_change: Option<FileModeChange>,
}

/// 差异块的表示
pub struct DiffHunk {
    /// 原始文件的行范围
    old_range: LineRange,
    /// 新文件的行范围
    new_range: LineRange,
    /// 变更的行
    lines: Vec<DiffLine>,
}
```

### 2.3 语法树与 Diff 集成模块

```rust
/// 将Git差异映射到语法树节点
pub struct DiffAstMapper {
    /// 项目AST缓存
    project_ast: ProjectAst,
    /// 语言管理器
    lang_manager: LanguageManager,
    /// 查询游标设置
    cursor_config: QueryCursorConfig,
}

impl DiffAstMapper {
    /// 将Git差异映射到语法树节点
    pub fn map_diff_to_ast(&self, diff: &GitDiff) -> Result<DiffAstMapping, MappingError> { /* ... */ }
    
    /// 对于单个文件，将差异映射到AST节点
    fn map_file_diff_to_ast(&self, file_diff: &ChangedFile) -> Result<FileDiffAstMapping, MappingError> { /* ... */ }
    
    /// 找出受影响的AST节点，使用优化的查询
    fn find_affected_nodes(&self, file_ast: &FileAst, hunk: &DiffHunk) -> Vec<AffectedNode> { /* ... */ }
}

/// 查询游标配置
pub struct QueryCursorConfig {
    /// 最大匹配深度
    max_depth: u32,
    /// 最大匹配数量
    match_limit: u32,
}

/// 表示差异到AST节点的映射
pub struct DiffAstMapping {
    /// 文件级别的映射
    file_mappings: Vec<FileDiffAstMapping>,
    /// 高级变更分析
    change_analysis: ChangeAnalysis,
}

/// 单个文件的差异到AST映射
pub struct FileDiffAstMapping {
    /// 文件路径
    path: PathBuf,
    /// 受影响的AST节点
    affected_nodes: Vec<AffectedNode>,
    /// 节点上下文
    context: NodeContext,
}
```

### 2.4 AI 提示生成模块

```rust
/// 负责生成AI提示
pub struct PromptGenerator {
    /// 基础提示模板
    base_templates: HashMap<String, String>,
    /// 配置
    config: PromptConfig,
}

impl PromptGenerator {
    /// 创建新的提示生成器
    pub fn new(config: PromptConfig) -> Self { /* ... */ }
    
    /// 根据AST分析和差异生成提示
    pub fn generate_commit_prompt(&self, mapping: &DiffAstMapping) -> String { /* ... */ }
    
    /// 提取关键上下文信息
    fn extract_context(&self, mapping: &DiffAstMapping) -> ContextInfo { /* ... */ }
    
    /// 格式化受影响的结构
    fn format_affected_structures(&self, mapping: &DiffAstMapping) -> String { /* ... */ }
}

/// 上下文信息
pub struct ContextInfo {
    /// 变更的函数/方法名
    changed_functions: Vec<String>,
    /// 变更的类/结构体
    changed_types: Vec<String>,
    /// 变更的API
    api_changes: Vec<ApiChange>,
    /// 变更模式 (功能添加/bug修复/重构等)
    change_patterns: Vec<ChangePattern>,
}
```

## 3. 详细实现设计

### 3.1 项目语法树构建流程

1. **初始化阶段**:
   ```rust
   // 初始化语言管理器并加载常用语言
   let mut lang_manager = LanguageManager::new();
   lang_manager.load_language("rust");
   lang_manager.load_language("javascript");
   // ...其他语言
   
   // 预编译常用查询
   lang_manager.precompile_query("rust", "functions", 
       "(function_item name: (identifier) @func.name) @function");
   lang_manager.precompile_query("rust", "structs", 
       "(struct_item name: (identifier) @struct.name) @struct");
   
   // 初始化项目AST
   let project_ast = ProjectAst::new(project_root_path);
   ```

2. **整个项目解析**:
   ```rust
   // 递归遍历项目文件(使用并行处理)
   pub fn parse_project(&mut self) -> Result<(), ParseError> {
       use rayon::prelude::*;
       
       let files: Vec<PathBuf> = WalkDir::new(&self.root_path)
           .into_iter()
           .filter_map(Result::ok)
           .filter(|e| e.file_type().is_file())
           .filter(|e| self.should_parse_file(e.path()))
           .map(|e| e.path().to_path_buf())
           .collect();
           
       // 并行处理文件解析
       let results: Vec<_> = files.par_iter()
           .map(|path| self.parse_file(path))
           .collect();
       
       // 处理结果
       for result in results {
           if let Err(e) = result {
               log::warn!("解析失败: {}", e);
           }
       }
       
       self.last_update = SystemTime::now();
       Ok(())
   }
   ```

3. **单文件解析(增量更新)**:
   ```rust
   pub fn parse_file(&mut self, path: &Path) -> Result<&FileAst, ParseError> {
       // 检测文件语言
       let language = self.lang_manager.detect_language(path)?;
       
       // 读取文件内容
       let source = fs::read_to_string(path)?;
       let content_hash = calculate_hash(&source);
       
       // 检查是否需要重新解析或使用增量更新
       if let Some(existing_ast) = self.file_asts.get_mut(path) {
           if existing_ast.content_hash == content_hash {
               return Ok(existing_ast);
           }
           
           // 使用增量更新进行解析
           let old_tree = &mut existing_ast.tree;
           let old_source = &existing_ast.source;
           
           // 计算编辑操作
           let edit = calculate_edit(old_source, &source);
           
           // 应用编辑到旧树
           old_tree.edit(&edit);
           
           // 创建新解析器并重用已编辑的旧树
           let mut parser = Parser::new();
           parser.set_language(language)?;
           
           if let Some(new_tree) = parser.parse(&source, Some(old_tree)) {
               // 更新AST
               *old_tree = new_tree;
               existing_ast.source = source;
               existing_ast.content_hash = content_hash;
               existing_ast.last_parsed = SystemTime::now();
               
               return Ok(existing_ast);
           }
       }
       
       // 完全重新解析
       let mut parser = Parser::new();
       parser.set_language(language)?;
       let tree = parser.parse(&source, None)?;
       
       // 存储AST
       let file_ast = FileAst {
           tree,
           source,
           content_hash,
           last_parsed: SystemTime::now(),
           language_id: language.name().to_string(),
       };
       
       self.file_asts.insert(path.to_path_buf(), file_ast);
       Ok(self.file_asts.get(path).unwrap())
   }
   ```

### 3.2 优化的 Diff-AST 映射算法

1. **差异解析**:
   ```rust
   pub fn parse_diff(&self, diff_text: &str) -> Result<GitDiff, DiffParseError> {
       let mut parser = DiffParser::new();
       parser.parse_text(diff_text)
   }
   ```

2. **优化的差异映射流程**:
   ```rust
   pub fn map_diff_to_ast(&self, diff: &GitDiff) -> Result<DiffAstMapping, MappingError> {
       let mut file_mappings = Vec::new();
       
       // 并行处理文件映射
       use rayon::prelude::*;
       let results: Vec<Result<FileDiffAstMapping, MappingError>> = diff.changed_files
           .par_iter()
           .filter(|file| self.is_supported_file_type(&file.path))
           .map(|changed_file| {
               match changed_file.change_type {
                   ChangeType::Added => self.handle_added_file(changed_file),
                   ChangeType::Modified => self.handle_modified_file(changed_file),
                   ChangeType::Deleted => self.handle_deleted_file(changed_file),
                   ChangeType::Renamed => self.handle_renamed_file(changed_file),
               }
           })
           .collect();
       
       // 处理结果
       for result in results {
           match result {
               Ok(mapping) => file_mappings.push(mapping),
               Err(e) => log::warn!("映射失败: {}", e),
           }
       }
       
       let change_analysis = self.analyze_changes(&file_mappings);
       
       Ok(DiffAstMapping {
           file_mappings,
           change_analysis,
       })
   }
   ```

3. **优化的受影响节点识别**:
   ```rust
   fn find_affected_nodes(&self, file_ast: &FileAst, hunk: &DiffHunk) -> Vec<AffectedNode> {
       let mut affected_nodes = Vec::new();
       let tree = &file_ast.tree;
       let source = &file_ast.source;
       
       // 将差异行转换为字节偏移
       let start_byte = line_to_byte(source, hunk.new_range.start);
       let end_byte = line_to_byte(source, hunk.new_range.end);
       
       // 1. 使用预编译查询
       let language = file_ast.language_id.as_str();
       let mut query_cursor = QueryCursor::new();
       
       // 2. 设置查询限制控制内存使用
       query_cursor.set_max_start_depth(self.cursor_config.max_depth);
       query_cursor.set_match_limit(self.cursor_config.match_limit);
       
       // 3. 创建限定范围的查询
       let range = tree_sitter::Range {
           start_point: byte_to_point(source, start_byte),
           end_point: byte_to_point(source, end_byte),
           start_byte,
           end_byte,
       };
       
       // 4. 使用精确匹配模式和谓词
       let query = self.lang_manager.get_query(language, "structures")
           .unwrap_or_else(|| {
               // 回退查询，通过精确模式限定结构类型
               let fallback_query = format!(
                   "(function_definition name: (_) @func.name) @function
                    (class_definition name: (_) @class.name) @class
                    (method_definition name: (_) @method.name) @method
                    (interface_definition name: (_) @interface.name) @interface"
               );
               &tree_sitter::Query::new(
                   self.lang_manager.languages.get(language).unwrap(),
                   &fallback_query
               ).unwrap()
           });
       
       // 执行范围限定查询
       let matches = query_cursor.matches(query, tree.root_node(), source.as_bytes(), Some(range));
       
       // 5. 通过捕获命名优化处理
       for match_ in matches {
           for capture in match_.captures {
               let node = capture.node;
               
               // 通过名称分类处理不同节点
               let capture_name = query.capture_names()[capture.index as usize];
               
               // 创建受影响节点
               if capture_name.ends_with(".name") {
                   continue; // 跳过名称捕获，只处理整体结构
               }
               
               let node_type = capture_name.to_string();
               let name = self.extract_node_name(&node, source, &node_type);
               
               affected_nodes.push(AffectedNode {
                   node_type,
                   name,
                   start_position: node.start_position(),
                   end_position: node.end_position(),
                   parent_info: self.extract_parent_info(node, source),
               });
           }
       }
       
       // 释放资源
       query_cursor.reset();
       
       affected_nodes
   }
   ```

### 3.3 变更分析逻辑

```rust
fn analyze_changes(&self, file_mappings: &[FileDiffAstMapping]) -> ChangeAnalysis {
    let mut analysis = ChangeAnalysis::default();
    
    // 使用并行处理加速分析
    use rayon::prelude::*;
    
    // 并行收集节点类型统计
    let node_counts: Vec<(String, usize)> = file_mappings.par_iter()
        .flat_map(|mapping| {
            mapping.affected_nodes.iter()
                .map(|node| (node.node_type.clone(), 1))
                .collect::<Vec<_>>()
        })
        .collect();
    
    // 合并统计结果
    for (node_type, count) in node_counts {
        match node_type.as_str() {
            "function" => analysis.function_changes += count,
            "class" | "struct" => analysis.type_changes += count,
            "method" => analysis.method_changes += count,
            _ => {} // 其他类型
        }
    }
    
    // 检测API变更
    analysis.api_changes = self.detect_api_changes(file_mappings);
    
    // 推断变更模式
    analysis.change_pattern = self.infer_change_pattern(&analysis, file_mappings);
    
    // 评估变更规模
    analysis.change_scope = self.evaluate_change_scope(file_mappings);
    
    analysis
}
```

### 3.4 AI 提示生成和结果处理

```rust
pub fn generate_commit_prompt(&self, mapping: &DiffAstMapping) -> String {
    let template = self.base_templates.get("commit").unwrap_or(&String::new());
    
    // 提取上下文信息
    let context = self.extract_context(mapping);
    
    // 构建提示
    let mut prompt = template.clone();
    
    // 添加文件变更概述
    prompt.push_str("\n\n## 变更文件概述\n");
    for file_mapping in &mapping.file_mappings {
        prompt.push_str(&format!("- {}: ", file_mapping.path.display()));
        prompt.push_str(&format!("{} 个结构受影响\n", file_mapping.affected_nodes.len()));
    }
    
    // 添加结构变更详情
    prompt.push_str("\n## 结构变更详情\n");
    prompt.push_str(&self.format_affected_structures(mapping));
    
    // 添加变更模式信息
    prompt.push_str("\n## 推断的变更模式\n");
    prompt.push_str(&format!("- 变更类型: {}\n", mapping.change_analysis.change_pattern));
    prompt.push_str(&format!("- 变更范围: {}\n", mapping.change_analysis.change_scope));
    
    // 添加API变更信息
    if !mapping.change_analysis.api_changes.is_empty() {
        prompt.push_str("\n## API变更\n");
        for api_change in &mapping.change_analysis.api_changes {
            prompt.push_str(&format!("- {}: {} -> {}\n", 
                api_change.name, 
                api_change.old_signature.as_deref().unwrap_or("无"), 
                api_change.new_signature.as_deref().unwrap_or("无")));
        }
    }
    
    // 添加原始diff信息
    prompt.push_str("\n## 原始Diff信息\n```diff\n");
    // 这里添加原始diff
    prompt.push_str("```\n");
    
    prompt
}
```

## 4. Tree-sitter 优化实现

### 4.1 精确匹配模式实现

```rust
/// 为不同语言创建精确的查询模式
fn create_optimized_query_for_language(&self, language_id: &str) -> String {
    match language_id {
        "rust" => r#"
            ; 函数定义
            (function_item
              name: (identifier) @func.name
              parameters: (parameters) @func.params
              return_type: (_)? @func.return
            ) @function
            
            ; 结构体定义
            (struct_item
              name: (identifier) @struct.name
              body: (field_declaration_list)? @struct.body
            ) @struct
            
            ; 方法定义
            (impl_item
              trait: (_)? @impl.trait
              type: (_) @impl.type
              body: (declaration_list 
                (function_item) @impl.method
              )
            ) @impl
        "#.to_string(),
        
        "javascript" | "typescript" => r#"
            ; 函数声明
            (function_declaration
              name: (identifier) @func.name
              parameters: (formal_parameters) @func.params
              body: (statement_block) @func.body
            ) @function
            
            ; 类声明
            (class_declaration
              name: (identifier) @class.name
              body: (class_body) @class.body
            ) @class
            
            ; 方法定义
            (method_definition
              name: (property_identifier) @method.name
              parameters: (formal_parameters) @method.params
              body: (statement_block) @method.body
            ) @method
        "#.to_string(),
        
        _ => {
            // 通用回退查询
            r#"
                (function) @function
                (class) @class
                (method) @method
                (struct) @struct
                (interface) @interface
            "#.to_string()
        }
    }
}
```

### 4.2 查询预编译实现

```rust
/// 预编译常用查询并缓存
fn initialize_precompiled_queries(&mut self) -> Result<(), QueryError> {
    // 为每种支持的语言预编译查询
    for language_id in &["rust", "javascript", "typescript", "python", "java", "c", "cpp"] {
        if let Some(language) = self.languages.get(*language_id) {
            // 获取优化的查询模式
            let query_str = self.create_optimized_query_for_language(language_id);
            
            // 编译查询
            let query = tree_sitter::Query::new(*language, &query_str)?;
            
            // 缓存预编译的查询
            self.queries.insert(
                format!("{}.structures", language_id),
                PrecompiledQuery {
                    query,
                    creation_time: SystemTime::now(),
                }
            );
            
            // 可以添加更多特定查询类型
            // ...
        }
    }
    
    Ok(())
}

/// 获取预编译查询
pub fn get_precompiled_query(&self, language_id: &str, query_type: &str) -> Option<&Query> {
    let key = format!("{}.{}", language_id, query_type);
    self.queries.get(&key).map(|pq| &pq.query)
}
```

### 4.3 分页处理大型文件实现

```rust
/// 分块处理大型文件的节点查询
fn process_large_file(&self, file_ast: &FileAst, processor: &mut dyn NodeProcessor) -> Result<(), ProcessError> {
    const CHUNK_SIZE: usize = 500; // 每次处理500行
    
    let tree = &file_ast.tree;
    let source = &file_ast.source;
    let mut query_cursor = QueryCursor::new();
    
    // 获取文件行数
    let line_count = source.lines().count();
    
    // 分块处理
    for chunk_start in (0..line_count).step_by(CHUNK_SIZE) {
        let chunk_end = std::cmp::min(chunk_start + CHUNK_SIZE, line_count);
        
        // 计算块的字节范围
        let start_byte = line_to_byte(source, chunk_start);
        let end_byte = line_to_byte(source, chunk_end);
        
        // 创建查询范围
        let range = tree_sitter::Range {
            start_point: byte_to_point(source, start_byte),
            end_point: byte_to_point(source, end_byte),
            start_byte,
            end_byte,
        };
        
        // 获取当前文件语言的查询
        let query = self.get_language_query(&file_ast.language_id)?;
        
        // 执行范围查询
        for match_ in query_cursor.matches(&query, tree.root_node(), source.as_bytes(), Some(range)) {
            processor.process_match(match_, source, &query)?;
        }
        
        // 重置游标状态
        query_cursor.reset();
    }
    
    Ok(())
}
```

### 4.4 谓词优化实现

```rust
/// 创建带谓词的优化查询
fn create_query_with_predicates(&self, language_id: &str, patterns: &[&str]) -> Result<Query, QueryError> {
    let language = self.get_language(language_id)?;
    
    // 构建带谓词的查询
    let mut query_parts = Vec::new();
    
    for pattern in patterns {
        query_parts.push(pattern.to_string());
    }
    
    // 添加谓词过滤
    query_parts.push(r#"
        ; 谓词过滤条件
        ((identifier) @id
         (#match? @id "^(test|spec|benchmark)"))
         
        ; 类型过滤
        ((type_identifier) @type
         (#not-match? @type "^(Test|Spec|Mock)"))
         
        ; 注释过滤
        ((comment) @comment
         (#contains? @comment "TODO" "FIXME" "HACK"))
    "#.to_string());
    
    // 合并查询部分
    let query_str = query_parts.join("\n");
    
    // 创建查询
    Ok(tree_sitter::Query::new(language, &query_str)?)
}
```

### 4.5 捕获管理实现

```rust
/// 处理查询捕获
fn process_captures(&self, match_: &QueryMatch, source: &str, query: &Query) -> Result<Vec<AffectedNode>, ProcessError> {
    let mut nodes = Vec::new();
    
    // 按照捕获名称分类处理
    for capture in &match_.captures {
        let capture_name = query.capture_names()[capture.index as usize];
        let node = capture.node;
        
        match capture_name {
            "function" | "class" | "struct" | "impl" | "method" | "interface" => {
                // 处理主要结构节点
                let node_type = capture_name.to_string();
                let name = self.extract_node_name_by_query(node, source, query, match_);
                
                nodes.push(AffectedNode {
                    node_type,
                    name,
                    start_position: node.start_position(),
                    end_position: node.end_position(),
                    parent_info: self.extract_parent_info(node, source),
                    change_type: self.determine_change_type(node, source),
                });
            },
            "func.name" | "class.name" | "struct.name" | "method.name" => {
                // 这些是名称节点，它们的父节点已经在上面处理过了
                continue;
            },
            "comment" => {
                // 处理注释节点，可能包含重要信息
                let text = node.utf8_text(source.as_bytes())
                    .map_err(|e| ProcessError::TextExtraction(e.to_string()))?;
                
                if text.contains("TODO") || text.contains("FIXME") {
                    nodes.push(AffectedNode {
                        node_type: "comment".to_string(),
                        name: text.lines().next().unwrap_or("").to_string(),
                        start_position: node.start_position(),
                        end_position: node.end_position(),
                        parent_info: None,
                        change_type: ChangeType::Modified,
                    });
                }
            },
            _ => {
                // 其他类型的节点
                log::debug!("未处理的捕获类型: {}", capture_name);
            }
        }
    }
    
    Ok(nodes)
}
```

### 4.6 错误恢复处理实现

```rust
/// 处理语法错误节点
fn handle_syntax_errors(&self, file_ast: &FileAst) -> Vec<SyntaxError> {
    let mut errors = Vec::new();
    let source = &file_ast.source;
    let tree = &file_ast.tree;
    
    // 创建专门查找ERROR节点的查询
    let error_query_str = "(ERROR) @syntax_error";
    let language = self.lang_manager.get_language(&file_ast.language_id)
        .expect("语言应该已加载");
    
    let error_query = Query::new(language, error_query_str)
        .expect("ERROR查询应该有效");
    
    let mut query_cursor = QueryCursor::new();
    
    // 查找所有ERROR节点
    for match_ in query_cursor.matches(&error_query, tree.root_node(), source.as_bytes(), None) {
        for capture in &match_.captures {
            let node = capture.node;
            let range = node.byte_range();
            
            // 获取错误上下文
            let line_start = node.start_position().row;
            let line_end = node.end_position().row;
            
            // 提取包含错误的行
            let context = source.lines()
                .skip(line_start)
                .take(line_end - line_start + 1)
                .collect::<Vec<_>>()
                .join("\n");
            
            errors.push(SyntaxError {
                position: node.start_position(),
                range,
                context,
            });
            
            log::warn!("在 {}:{} 发现语法错误", line_start + 1, node.start_position().column + 1);
        }
    }
    
    // 重置游标
    query_cursor.reset();
    
    errors
}

/// 表示语法错误
#[derive(Debug, Clone)]
pub struct SyntaxError {
    /// 错误位置
    position: tree_sitter::Point,
    /// 错误范围
    range: std::ops::Range<usize>,
    /// 错误上下文
    context: String,
}
```

### 4.7 增量更新优化实现

```rust
/// 增量更新文件AST
pub fn update_file_ast(&mut self, path: &Path, edit: &TextEdit) -> Result<(), ParseError> {
    if let Some(file_ast) = self.file_asts.get_mut(path) {
        // 转换为tree-sitter的编辑格式
        let ts_edit = tree_sitter::InputEdit {
            start_byte: edit.start_byte,
            old_end_byte: edit.old_end_byte,
            new_end_byte: edit.new_end_byte,
            start_position: edit.start_position,
            old_end_position: edit.old_end_position,
            new_end_position: edit.new_end_position,
        };
        
        // 应用编辑到树
        file_ast.tree.edit(&ts_edit);
        
        // 更新源代码
        let new_source = fs::read_to_string(path)?;
        file_ast.source = new_source;
        file_ast.content_hash = calculate_hash(&file_ast.source);
        
        // 重新解析使用已编辑的树
        let language = self.lang_manager.get_language(&file_ast.language_id)?;
        let mut parser = Parser::new();
        parser.set_language(language)?;
        
        if let Some(new_tree) = parser.parse(&file_ast.source, Some(&file_ast.tree)) {
            file_ast.tree = new_tree;
            file_ast.last_parsed = SystemTime::now();
        } else {
            return Err(ParseError::ParseFailed(path.to_string_lossy().to_string()));
        }
    } else {
        // 文件不在缓存中，执行完整解析
        self.parse_file(path)?;
    }
    
    Ok(())
}

/// 文本编辑描述
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// 编辑开始字节位置
    pub start_byte: usize,
    /// 编辑前结束字节位置
    pub old_end_byte: usize,
    /// 编辑后结束字节位置
    pub new_end_byte: usize,
    /// 编辑开始位置(行,列)
    pub start_position: tree_sitter::Point,
    /// 编辑前结束位置
    pub old_end_position: tree_sitter::Point,
    /// 编辑后结束位置
    pub new_end_position: tree_sitter::Point,
}
```

### 4.8 并行处理实现

```rust
/// 并行处理多个文件的查询
pub fn parallel_query_files(&self, file_paths: &[PathBuf], query_type: &str) -> Result<Vec<QueryResult>, QueryError> {
    use rayon::prelude::*;
    
    // 检查文件在缓存中
    let files_to_parse: Vec<_> = file_paths.iter()
        .filter(|p| !self.file_asts.contains_key(*p))
        .collect();
    
    // 并行解析缺失的文件
    if !files_to_parse.is_empty() {
        files_to_parse.par_iter()
            .try_for_each(|p| -> Result<(), ParseError> {
                self.parse_file(p)?;
                Ok(())
            })?;
    }
    
    // 并行执行查询
    let results: Vec<Result<QueryResult, QueryError>> = file_paths.par_iter()
        .filter_map(|path| self.file_asts.get(path).map(|ast| (path, ast)))
        .map(|(path, ast)| {
            let query = self.lang_manager.get_query(&ast.language_id, query_type)
                .ok_or_else(|| QueryError::QueryNotFound(ast.language_id.clone(), query_type.to_string()))?;
            
            let mut matches = Vec::new();
            let mut query_cursor = QueryCursor::new();
            
            for match_ in query_cursor.matches(query, ast.tree.root_node(), ast.source.as_bytes(), None) {
                matches.push(match_.clone());
            }
            
            Ok(QueryResult {
                path: path.clone(),
                matches,
                source: ast.source.clone(),
            })
        })
        .collect();
    
    // 处理结果
    results.into_iter().collect()
}

/// 查询结果
#[derive(Debug)]
pub struct QueryResult {
    /// 文件路径
    pub path: PathBuf,
    /// 查询匹配
    pub matches: Vec<QueryMatch>,
    /// 源代码
    pub source: String,
}
```

## 5. 数据流程

### 5.1 执行流程图

```
1. 用户执行 `gitie commit`
   │
2. 获取暂存的变更 (git diff --staged)
   │
3. 解析 git diff 输出
   │
4. 更新/构建受影响文件的语法树
   │  ┌─────────────────────┐
   │  │ - 检查语法树缓存    │
   │  │ - 应用增量更新      │
   │  │ - 并行处理多文件    │
   │  └─────────────────────┘
   │
5. 映射 diff 到语法树节点
   │  ┌─────────────────────┐
   │  │ - 使用优化查询      │
   │  │ - 应用谓词过滤      │
   │  │ - 分页处理大文件    │
   │  └─────────────────────┘
   │
6. 分析变更的语法结构和上下文
   │
7. 生成增强的 AI 提示
   │  ┌─────────────────────┐
   │  │ - 包含结构信息      │
   │  │ - 提供变更上下文    │
   │  │ - 添加API变更分析   │
   │  └─────────────────────┘
   │
8. 发送提示到 LLM
   │
9. 接收生成的 commit 消息
   │
10. 执行 git commit 操作
```

### 5.2 语法树缓存管理

1. **缓存存储位置**:
   - 默认在 `~/.config/gitie/ast_cache/` 目录下
   - 每个项目一个子目录，使用项目路径的哈希值命名

2. **缓存实现**:
   ```rust
   pub struct AstCache {
       /// 基础缓存目录
       cache_dir: PathBuf,
       /// 项目标识符
       project_id: String,
       /// 内存中的缓存
       memory_cache: LruCache<PathBuf, FileAst>,
       /// 磁盘缓存启用状态
       disk_cache_enabled: bool,
   }
   
   impl AstCache {
       /// 创建新缓存
       pub fn new(project_path: &Path, config: CacheConfig) -> Self { /* ... */ }
       
       /// 从缓存获取AST
       pub fn get(&mut self, path: &Path) -> Option<&FileAst> { /* ... */ }
       
       /// 存储AST到缓存
       pub fn put(&mut self, path: PathBuf, ast: FileAst) { /* ... */ }
       
       /// 清理过期缓存
       pub fn cleanup_expired(&mut self) { /* ... */ }
   }
   ```

3. **缓存过期策略**:
   ```rust
   fn is_cache_valid(&self, path: &Path, file_ast: &FileAst) -> bool {
       // 检查文件是否已更改
       if let Ok(metadata) = fs::metadata(path) {
           if let Ok(modified_time) = metadata.modified() {
               // 如果文件修改时间晚于解析时间，缓存无效
               if modified_time > file_ast.last_parsed {
                   return false;
               }
               
               // 检查文件大小是否变化
               if metadata.len() as usize != file_ast.source.len() {
                   return false;
               }
               
               // 检查内容哈希
               if let Ok(content) = fs::read_to_string(path) {
                   let current_hash = calculate_hash(&content);
                   return current_hash == file_ast.content_hash;
               }
           }
       }
       
       // 默认视为无效
       false
   }
   ```

## 6. API 设计

### 6.1 公共 API

```rust
/// Gitie Tree-sitter 集成插件接口
pub trait GitieTreeSitterPlugin {
    /// 初始化插件
    fn initialize(&mut self, config: &Config) -> Result<(), Error>;
    
    /// 分析已暂存的更改
    fn analyze_staged_changes(&self) -> Result<DiffAstMapping, Error>;
    
    /// 生成 commit 消息
    fn generate_commit_message(&self, mapping: &DiffAstMapping) -> Result<String, Error>;
    
    /// 执行 commit 操作
    fn commit(&self, message: &str, args: &[String]) -> Result<(), Error>;
}

/// 插件实现
pub struct GitieTreeSitterImpl {
    project_ast: ProjectAst,
    diff_mapper: DiffAstMapper,
    prompt_generator: PromptGenerator,
    ai_client: AiClient,
    config: TreeSitterConfig,
}

impl GitieTreeSitterPlugin for GitieTreeSitterImpl {
    fn initialize(&mut self, config: &Config) -> Result<(), Error> {
        // 加载配置
        self.config = config.tree_sitter.clone();
        
        // 初始化语言管理器
        let mut lang_manager = LanguageManager::new();
        for language_id in &self.config.languages {
            if let Err(e) = lang_manager.load_language(language_id) {
                log::warn!("无法加载语言 {}: {}", language_id, e);
            }
        }
        
        // 初始化项目AST
        self.project_ast = ProjectAst::new(
            config.project_root.clone(),
            self.config.cache.clone(),
        );
        
        // 初始化映射器
        self.diff_mapper = DiffAstMapper::new(
            self.project_ast.clone(),
            lang_manager,
            self.config.query_cursor.clone(),
        );
        
        // 初始化提示生成器
        self.prompt_generator = PromptGenerator::new(
            self.config.prompt.clone(),
        );
        
        // 初始化AI客户端
        self.ai_client = AiClient::new(
            config.ai.clone(),
        );
        
        Ok(())
    }
    
    fn analyze_staged_changes(&self) -> Result<DiffAstMapping, Error> {
        // 获取暂存的变更
        let diff_text = self.get_staged_diff()?;
        
        // 解析diff
        let diff = self.parse_diff(&diff_text)?;
        
        // 映射到AST
        let mapping = self.diff_mapper.map_diff_to_ast(&diff)?;
        
        Ok(mapping)
    }
    
    fn generate_commit_message(&self, mapping: &DiffAstMapping) -> Result<String, Error> {
        // 生成提示
        let prompt = self.prompt_generator.generate_commit_prompt(mapping);
        
        // 调用AI
        let message = self.ai_client.generate_response(&prompt)?;
        
        Ok(message)
    }
    
    fn commit(&self, message: &str, args: &[String]) -> Result<(), Error> {
        // 构建commit命令
        let mut cmd_args = vec!["commit".to_string(), "-m".to_string(), message.to_string()];
        cmd_args.extend_from_slice(args);
        
        // 执行commit
        execute_git_command(&cmd_args)
    }
}
```

### 6.2 配置 API

```rust
#[derive(Deserialize, Debug, Clone)]
pub struct TreeSitterConfig {
    /// 是否启用语法树分析
    pub enabled: bool,
    
    /// 缓存配置
    pub cache: CacheConfig,
    
    /// 支持的语言列表
    pub languages: Vec<String>,
    
    /// 查询游标配置
    pub query_cursor: QueryCursorConfig,
    
    /// 提示配置
    pub prompt: PromptConfig,
    
    /// 分析深度
    pub analysis_depth: AnalysisDepth,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CacheConfig {
    /// 缓存目录
    pub cache_dir: Option<PathBuf>,
    
    /// 缓存过期时间（秒）
    pub expiry_seconds: u64,
    
    /// 最大缓存大小（MB）
    pub max_size_mb: u64,
    
    /// 是否启用磁盘缓存
    pub use_disk_cache: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct QueryCursorConfig {
    /// 最大匹配深度
    pub max_depth: u32,
    
    /// 最大匹配数量
    pub match_limit: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub enum AnalysisDepth {
    /// 只分析直接受影响的节点
    Shallow,
    
    /// 分析受影响节点及其父级
    Medium,
    
    /// 深度分析，包括相关引用和依赖
    Deep,
}
```

## 7. 性能考量

### 7.1 大型项目的性能优化

1. **增量解析**:
   - 只解析变更的文件
   - 使用文件哈希检测真正的变更

2. **并行处理**:
   - 使用 Rayon 并行解析多个文件
   - 线程池管理，避免创建过多线程

   ```rust
   pub fn parse_files_parallel(&mut self, paths: &[PathBuf]) -> Result<(), ParseError> {
       use rayon::prelude::*;
       
       let results: Vec<Result<(), ParseError>> = paths
           .par_iter()
           .map(|path| {
               match self.parse_file(path) {
                   Ok(_) => Ok(()),
                   Err(e) => Err(e),
               }
           })
           .collect();
       
       // 处理错误
       for (idx, result) in results.iter().enumerate() {
           if let Err(e) = result {
               log::warn!("解析 {} 失败: {}", paths[idx].display(), e);
           }
       }
       
       Ok(())
   }
   ```

3. **内存管理**:
   - LRU缓存策略，淘汰最少使用的AST
   - 延迟加载机制，按需解析文件

   ```rust
   pub fn get_file_ast(&mut self, path: &Path) -> Result<&FileAst, ParseError> {
       // 检查缓存
       if self.file_asts.contains_key(path) {
           let ast = self.file_asts.get(path).unwrap();
           
           // 验证缓存有效性
           if self.is_cache_valid(path, ast) {
               return Ok(ast);
           }
       }
       
       // 缓存无效或不存在，解析文件
       self.parse_file(path)
   }
   ```

### 7.2 查询优化

1. **查询限制**:
   ```rust
   fn setup_optimized_cursor(&self) -> QueryCursor {
       let mut cursor = QueryCursor::new();
       cursor.set_max_start_depth(self.cursor_config.max_depth);
       cursor.set_match_limit(self.cursor_config.match_limit);
       cursor
   }
   ```

2. **避免大范围查询**:
   ```rust
   fn process_diff_hunks(&self, file_ast: &FileAst, hunks: &[DiffHunk]) -> Vec<AffectedNode> {
       let mut affected_nodes = Vec::new();
       
       for hunk in hunks {
           // 仅查询变更范围
           let start_byte = line_to_byte(&file_ast.source, hunk.new_range.start);
           let end_byte = line_to_byte(&file_ast.source, hunk.new_range.end);
           
           // 创建范围限定查询
           let range = tree_sitter::Range {
               start_point: byte_to_point(&file_ast.source, start_byte),
               end_point: byte_to_point(&file_ast.source, end_byte),
               start_byte,
               end_byte,
           };
           
           // 针对当前范围执行查询
           let hunk_nodes = self.query_range(file_ast, range);
           affected_nodes.extend(hunk_nodes);
       }
       
       affected_nodes
   }
   ```

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_diff_parsing() {
        let diff_text = "diff --git a/src/main.rs b/src/main.rs\n...";
        let parser = DiffParser::new();
        let result = parser.parse_text(diff_text);
        assert!(result.is_ok());
        let diff = result.unwrap();
        assert_eq!(diff.changed_files.len(), 1);
        // 更多断言...
    }
    
    #[test]
    fn test_ast_mapping() {
        // 准备测试数据
        let diff = prepare_test_diff();
        let mapper = prepare_test_mapper();
        
        // 执行映射
        let mapping = mapper.map_diff_to_ast(&diff).unwrap();
        
        // 验证映射结果
        assert!(!mapping.file_mappings.is_empty());
        let file_mapping = &mapping.file_mappings[0];
        assert!(!file_mapping.affected_nodes.is_empty());
        // 更多断言...
    }
    
    #[test]
    fn test_query_optimization() {
        // 准备大型代码文件
        let source = prepare_large_source();
        
        // 测试优化前的查询
        let start_time = Instant::now();
        let unoptimized_results = run_unoptimized_query(&source);
        let unoptimized_duration = start_time.elapsed();
        
        // 测试优化后的查询
        let start_time = Instant::now();
        let optimized_results = run_optimized_query(&source);
        let optimized_duration = start_time.elapsed();
        
        // 验证结果相同且性能提升
        assert_eq!(unoptimized_results.len(), optimized_results.len());
        assert!(optimized_duration < unoptimized_duration);
    }
}
```

### 8.2 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_end_to_end_commit_flow() {
        // 设置测试仓库
        let repo_path = setup_test_repository();
        
        // 创建文件变更
        create_test_file_changes(&repo_path);
        
        // 执行gitie提交
        let result = execute_gitie_commit(&repo_path);
        
        // 验证结果
        assert!(result.is_ok());
        let commit_message = get_last_commit_message(&repo_path);
        assert!(commit_message.contains("function add"));
        // 更多断言...
    }
    
    #[test]
    fn test_syntax_error_handling() {
        // 准备带语法错误的文件
        let file_with_errors = create_file_with_syntax_errors();
        
        // 解析文件
        let mut project_ast = ProjectAst::new(file_with_errors.parent().unwrap().to_path_buf());
        let result = project_ast.parse_file(&file_with_errors);
        
        // 验证能够处理错误
        assert!(result.is_ok());
        let ast = result.unwrap();
        
        // 检查错误节点识别
        let mapper = DiffAstMapper::new(project_ast, LanguageManager::new());
        let errors = mapper.handle_syntax_errors(ast);
        assert!(!errors.is_empty());
    }
}
```

### 8.3 性能测试

```rust
#[cfg(test)]
mod benchmark_tests {
    use super::*;
    use test::Bencher;
    
    #[bench]
    fn bench_parse_large_project(b: &mut Bencher) {
        // 准备大型项目
        let project_path = prepare_large_test_project();
        
        b.iter(|| {
            let mut project_ast = ProjectAst::new(project_path.clone());
            project_ast.parse_project().unwrap();
        });
    }
    
    #[bench]
    fn bench_diff_analysis(b: &mut Bencher) {
        // 准备差异数据
        let (diff, mapper) = prepare_benchmark_diff_data();
        
        b.iter(|| {
            mapper.map_diff_to_ast(&diff).unwrap();
        });
    }
    
    #[bench]
    fn bench_optimized_vs_unoptimized_query(b: &mut Bencher) {
        // 准备大型源代码
        let source = prepare_large_source();
        let tree = parse_source(&source);
        
        // 基准测试优化版本
        b.iter(|| {
            run_optimized_query(&source, &tree);
        });
    }
}
```

## 9. 实现路线图

### 9.1 第一阶段: 基础架构 (2-3周)

1. **Week 1**: Tree-Sitter 集成
   - 集成 tree-sitter 库
   - 实现基础语言解析器管理
   - 开发文件解析核心功能

2. **Week 2**: Diff 解析与映射
   - 实现 Git diff 解析器
   - 设计并实现基础的 AST 映射算法
   - 开发语法结构识别功能

3. **Week 3**: 缓存系统
   - 实现语法树缓存机制
   - 开发增量更新功能
   - 设计持久化存储

### 9.2 第二阶段: 核心功能 (3-4周)

1. **Week 4-5**: 查询优化
   - 实现优化的查询技术
   - 开发谓词过滤系统
   - 实现分页处理大型文件

2. **Week 6**: 变更分析
   - 开发变更识别算法
   - 实现类型和结构变更分析
   - 设计 API 变更检测

3. **Week 7**: AI 提示生成
   - 增强提示模板
   - 结构化上下文提取
   - 集成到提交流程

### 9.3 第三阶段: 优化与完善 (2-3周)

1. **Week 8**: 性能优化
   - 实现并行处理
   - 内存使用优化
   - 解决大型项目性能问题

2. **Week 9**: 多语言支持
   - 扩展语言支持
   - 优化特定语言查询
   - 完善特定语言规则

3. **Week 10**: 测试与文档
   - 完成测试套件
   - 编写用户文档
   - 准备发布

## 10. 总结

本技术设计提供了一个详细的框架，用于通过 Tree-Sitter 增强 Gitie 的代码理解能力。通过深入分析代码结构和变更影响，可以显著提高 AI 生成的提交消息质量，使其更好地反映代码变更的实际语义和目的。

设计中重点考虑了性能优化，包括：
1. 精确匹配模式减少不必要的遍历
2. 查询预编译避免重复解析开销
3. 分页处理大型文件降低内存消耗
4. 谓词优化提前过滤无效匹配
5. 并行处理提高多文件分析效率
6. 增量更新避免重复解析
7. 合理设置查询深度和匹配限制

该设计注重性能和可扩展性，同时保持与现有 Git 工作流的无缝集成。实施这一设计将有助于开发者更高效地管理代码变更历史，并改善团队协作。通过结合 Tree-sitter 的语法分析能力与 LLM 的智能理解，Gitie 将能够生成更精准、更有价值的 commit 信息，真正理解代码变更的语义和意图。