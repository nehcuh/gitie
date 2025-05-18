# Gitie 项目技术设计文档

## 1. 技术架构概述

### 1.1 系统架构

Gitie 的增强版架构将由以下核心组件组成：

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
}

impl LanguageManager {
    /// 创建新的语言管理器实例
    pub fn new() -> Self { /* ... */ }
    
    /// 根据文件扩展名或路径识别语言
    pub fn detect_language(&self, file_path: &Path) -> Option<&Language> { /* ... */ }
    
    /// 动态加载语言解析器
    pub fn load_language(&mut self, language_id: &str) -> Result<(), TreeSitterError> { /* ... */ }
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
}

impl DiffAstMapper {
    /// 将Git差异映射到语法树节点
    pub fn map_diff_to_ast(&self, diff: &GitDiff) -> Result<DiffAstMapping, MappingError> { /* ... */ }
    
    /// 对于单个文件，将差异映射到AST节点
    fn map_file_diff_to_ast(&self, file_diff: &ChangedFile) -> Result<FileDiffAstMapping, MappingError> { /* ... */ }
    
    /// 找出受影响的AST节点
    fn find_affected_nodes(&self, file_ast: &FileAst, hunk: &DiffHunk) -> Vec<AffectedNode> { /* ... */ }
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
   
   // 初始化项目AST
   let project_ast = ProjectAst::new(project_root_path);
   ```

2. **整个项目解析**:
   ```rust
   // 递归遍历项目文件
   pub fn parse_project(&mut self) -> Result<(), ParseError> {
       let files = WalkDir::new(&self.root_path)
           .into_iter()
           .filter_map(Result::ok)
           .filter(|e| e.file_type().is_file())
           .map(|e| e.path().to_path_buf());
           
       for file_path in files {
           self.parse_file(&file_path)?;
       }
       self.last_update = SystemTime::now();
       Ok(())
   }
   ```

3. **单文件解析**:
   ```rust
   pub fn parse_file(&mut self, path: &Path) -> Result<&FileAst, ParseError> {
       // 检测文件语言
       let language = self.lang_manager.detect_language(path)?;
       
       // 读取文件内容
       let source = fs::read_to_string(path)?;
       let content_hash = calculate_hash(&source);
       
       // 检查是否需要重新解析
       if let Some(existing_ast) = self.file_asts.get(path) {
           if existing_ast.content_hash == content_hash {
               return Ok(existing_ast);
           }
       }
       
       // 解析新AST
       let parser = Parser::new();
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

### 3.2 Diff-AST 映射算法

1. **差异解析**:
   ```rust
   pub fn parse_diff(&self, diff_text: &str) -> Result<GitDiff, DiffParseError> {
       let mut parser = DiffParser::new();
       parser.parse_text(diff_text)
   }
   ```

2. **差异映射**:
   ```rust
   pub fn map_diff_to_ast(&self, diff: &GitDiff) -> Result<DiffAstMapping, MappingError> {
       let mut file_mappings = Vec::new();
       
       for changed_file in &diff.changed_files {
           // 跳过二进制文件或不支持的文件类型
           if !self.is_supported_file_type(&changed_file.path) {
               continue;
           }
           
           let file_mapping = match changed_file.change_type {
               ChangeType::Added => self.handle_added_file(changed_file)?,
               ChangeType::Modified => self.handle_modified_file(changed_file)?,
               ChangeType::Deleted => self.handle_deleted_file(changed_file)?,
               ChangeType::Renamed => self.handle_renamed_file(changed_file)?,
           };
           
           file_mappings.push(file_mapping);
       }
       
       let change_analysis = self.analyze_changes(&file_mappings);
       
       Ok(DiffAstMapping {
           file_mappings,
           change_analysis,
       })
   }
   ```

3. **受影响节点识别**:
   ```rust
   fn find_affected_nodes(&self, file_ast: &FileAst, hunk: &DiffHunk) -> Vec<AffectedNode> {
       let mut affected_nodes = Vec::new();
       let tree = &file_ast.tree;
       let source = &file_ast.source;
       
       // 将差异行转换为字节偏移
       let start_byte = line_to_byte(source, hunk.new_range.start);
       let end_byte = line_to_byte(source, hunk.new_range.end);
       
       // 查询包含此范围的节点
       let query = format!(
           "(function_definition) @func
           (struct_definition) @struct
           (class_definition) @class
           (method_definition) @method
           (interface_definition) @interface"
       );
       
       let mut query_cursor = QueryCursor::new();
       let matches = query_cursor.matches(&query, tree.root_node(), source.as_bytes());
       
       for match_ in matches {
           for capture in match_.captures {
               let node = capture.node;
               
               // 检查节点是否与变更范围重叠
               if (node.start_byte() <= end_byte && node.end_byte() >= start_byte) {
                   // 提取节点类型和名称
                   let node_type = node.kind().to_string();
                   let name = extract_node_name(node, source);
                   
                   affected_nodes.push(AffectedNode {
                       node_type,
                       name,
                       start_position: node.start_position(),
                       end_position: node.end_position(),
                       parent_info: extract_parent_info(node, source),
                   });
               }
           }
       }
       
       affected_nodes
   }
   ```

### 3.3 变更分析逻辑

```rust
fn analyze_changes(&self, file_mappings: &[FileDiffAstMapping]) -> ChangeAnalysis {
    let mut analysis = ChangeAnalysis::default();
    
    // 统计变更类型
    for mapping in file_mappings {
        for node in &mapping.affected_nodes {
            match node.node_type.as_str() {
                "function_definition" => {
                    analysis.function_changes += 1;
                },
                "class_definition" | "struct_definition" => {
                    analysis.type_changes += 1;
                },
                "method_definition" => {
                    analysis.method_changes += 1;
                },
                // ... 其他类型
                _ => {}
            }
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
    
    // 添加原始diff信息
    prompt.push_str("\n## 原始Diff信息\n```diff\n");
    // 这里添加原始diff
    prompt.push_str("```\n");
    
    prompt
}
```

## 4. 数据流程

### 4.1 执行流程图

```
1. 用户执行 `gitie commit`
   │
2. 获取暂存的变更 (git diff --staged)
   │
3. 解析 git diff 输出
   │
4. 更新/构建受影响文件的语法树
   │
5. 映射 diff 到语法树节点
   │
6. 分析变更的语法结构和上下文
   │
7. 生成增强的 AI 提示
   │
8. 发送提示到 LLM
   │
9. 接收生成的 commit 消息
   │
10. 执行 git commit 操作
```

### 4.2 语法树缓存管理

1. **缓存存储位置**:
   - 默认在 `~/.config/gitie/ast_cache/` 目录下
   - 每个项目一个子目录，使用项目路径的哈希值命名

2. **缓存条目**:
   - 文件元数据: 路径、语言、修改时间、大小
   - 序列化的 AST: 使用 bincode 或类似格式
   - 文件内容哈希: 用于检测变更

3. **缓存过期策略**:
   - 对比文件的修改时间与缓存时间
   - 检查文件内容哈希是否匹配
   - 定期全量刷新（可配置间隔）

## 5. API 设计

### 5.1 公共 API

```rust
// 主要公共接口
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

// 实现
pub struct GitieTreeSitterImpl {
    project_ast: ProjectAst,
    diff_mapper: DiffAstMapper,
    prompt_generator: PromptGenerator,
    ai_client: AiClient,
}

impl GitieTreeSitterPlugin for GitieTreeSitterImpl {
    // 实现方法...
}
```

### 5.2 配置 API

```rust
#[derive(Deserialize, Debug, Clone)]
pub struct TreeSitterConfig {
    /// 是否启用语法树分析
    pub enabled: bool,
    
    /// 缓存配置
    pub cache: CacheConfig,
    
    /// 语言配置
    pub languages: HashMap<String, LanguageConfig>,
    
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

## 6. 性能考量

### 6.1 大型项目的性能优化

1. **增量解析**:
   - 只解析变更的文件
   - 使用文件哈希检测真正的变更

2. **并行处理**:
   - 并行解析多个文件的语法树
   - 使用线程池处理文件解析

   ```rust
   pub fn parse_files_parallel(&mut self, paths: &[PathBuf]) -> Result<(), ParseError> {
       use rayon::prelude::*;
       
       let results: Vec<Result<FileAst, ParseError>> = paths
           .par_iter()
           .map(|path| self.parse_file_internal(path))
           .collect();
       
       // 处理结果和错误
       for (path, result) in paths.iter().zip(results) {
           match result {
               Ok(ast) => { self.file_asts.insert(path.clone(), ast); }
               Err(e) => { log::warn!("Failed to parse {}: {}", path.display(), e); }
           }
       }
       
       Ok(())
   }
   ```

3. **内存管理**:
   - 使用懒加载策略
   - 实现LRU缓存淘汰策略

### 6.2 树遍历优化

1. **索引构建**:
   - 为关键节点类型建立索引
   - 使用结构化查询预筛选节点

2. **位置查询加速**:
   - 使用行/列->字节偏移的映射表
   - 缓存常用查询路径

## 7. 测试策略

### 7.1 单元测试

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
}
```

### 7.2 集成测试

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
}
```

### 7.3 性能测试

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
}
```

## 8. 实现路线图

### 8.1 第一阶段: 基础架构 (2-3周)

1. 集成 Tree-Sitter 库
2. 实现基础的文件解析功能
3. 开发 Diff 解析器
4. 设计并实现缓存机制

### 8.2 第二阶段: 核心功能 (3-4周)

1. 实现 Diff-AST 映射算法
2. 开发变更分析逻辑
3. 增强提示生成器
4. 集成到现有的 Gitie 流程

### 8.3 第三阶段: 优化与完善 (2-3周)

1. 性能优化
2. 多语言支持扩展
3. 高级分析功能
4. 用户配置与自定义选项

## 9. 风险与缓解

1. **性能风险**: 大型项目可能导致解析时间过长
   - 缓解: 增量解析、并行处理、缓存优化

2. **准确性风险**: 语法树映射可能不准确
   - 缓解: 多种启发式规则、回退机制、持续改进的测试集

3. **兼容性风险**: 不同语言和文件类型的支持
   - 缓解: 逐步扩展语言支持、适配器设计模式

4. **用户体验风险**: 可能增加命令延迟
   - 缓解: 异步处理、后台预解析、进度提示

## 10. 总结

本技术设计提供了一个详细的框架，用于通过 Tree-Sitter 增强 Gitie 的代码理解能力。通过深入分析代码结构和变更影响，可以显著提高 AI 生成的提交消息质量，使其更好地反映代码变更的实际语义和目的。

该设计注重性能和可扩展性，同时保持与现有 Git 工作流的无缝集成。实施这一设计将有助于开发者更高效地管理代码变更历史，并改善团队协作。