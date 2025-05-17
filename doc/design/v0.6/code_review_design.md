# Gitie 代码评审功能技术设计

## 1. 概述

本文档描述了 Gitie v0.6 版本中实现的代码评审功能的技术设计。该功能利用 Tree-sitter 提供的语法分析能力，结合 Git diff 功能，实现对代码变更的深度理解和评审。

### 1.1 设计目标

- 提供直观的代码评审命令 (`gitie review`)
- 集成评审功能到提交流程 (`gitie commit --review`)
- 利用 Tree-sitter 进行结构化代码分析
- 确保命令行参数与现有 Git 命令兼容
- 提供可配置的评审深度和关注点

### 1.2 功能概述

代码评审功能将支持以下核心能力：
- 分析代码结构变化（函数、类、方法的添加/修改/删除）
- 检查代码质量和可读性
- 识别潜在的安全风险
- 提供代码风格建议
- 评估代码性能影响

## 2. 功能需求

### 2.1 独立评审命令

```
gitie review [options] [<commit>] [<commit>]
```

支持的选项：
- `--depth=<level>` - 分析深度：basic, normal, deep
- `--focus=<area>` - 关注领域：security, performance, style, all
- `--lang=<language>` - 限定特定语言
- `--format=<format>` - 输出格式：text, json, html
- `--output=<file>` - 输出到文件

### 2.2 集成提交评审

```
gitie commit [--review] [--depth=<level>] [其他commit参数]
```

流程：
1. 执行代码评审
2. 显示评审结果
3. 用户选择：继续提交、修复问题、忽略问题、取消操作

### 2.3 参数设计（避免冲突）

为避免与 Git 现有选项冲突，使用以下参数约定：
- 使用长参数形式 `--review` 而非短参数 `-r`
- 使用 `--ts` 代替 `--tree-sitter`
- 组合参数采用连字符形式：`--review-ts`
- 保留所有原有 Git 参数功能

## 3. 技术架构

### 3.1 组件架构

```
                              ┌─────────────────┐
                              │                 │
                              │    命令解析器   │
                              │                 │
                              └────────┬────────┘
                                       │
                                       ▼
                              ┌─────────────────┐
                              │                 │
                              │   Git Diff 提取 │
                              │                 │
                              └────────┬────────┘
                                       │
                                       ▼
┌─────────────────┐              ┌─────────────────┐             ┌─────────────────┐
│                 │              │                 │             │                 │
│  Diff 结构化解析├─────────────►│  Tree-sitter 解析├────────────►│   分析规则引擎  │
│                 │              │                 │             │                 │
└─────────────────┘              └─────────────────┘             └────────┬────────┘
                                                                          │
                                                                          ▼
                                 ┌─────────────────┐             ┌─────────────────┐
                                 │                 │             │                 │
                                 │   报告生成器    │◄────────────┤    AI 增强     │
                                 │                 │             │                 │
                                 └────────┬────────┘             └─────────────────┘
                                          │
                                          ▼
                                 ┌─────────────────┐
                                 │                 │
                                 │   用户界面显示  │
                                 │                 │
                                 └─────────────────┘
```

### 3.2 核心组件

#### 3.2.1 命令解析器

```rust
pub struct ReviewCommand {
    pub options: ReviewOptions,
    pub commit_range: Option<CommitRange>,
}

pub struct ReviewOptions {
    pub depth: AnalysisDepth,
    pub focus: Vec<AnalysisFocus>,
    pub language: Option<String>,
    pub format: OutputFormat,
    pub output_file: Option<PathBuf>,
}

pub enum AnalysisDepth {
    Basic,
    Normal,
    Deep,
}

pub enum AnalysisFocus {
    Security,
    Performance,
    Style,
    Semantics,
    All,
}

pub enum OutputFormat {
    Text,
    Json,
    Html,
}
```

#### 3.2.2 Diff 结构化解析器

```rust
pub struct DiffAnalyzer {
    pub diff_parser: DiffParser,
}

impl DiffAnalyzer {
    pub fn new() -> Self;
    pub fn extract_diff(&self, commit_range: Option<CommitRange>) -> Result<GitDiff, Error>;
    pub fn parse_diff(&self, diff_text: &str) -> Result<GitDiff, Error>;
    pub fn structure_diff(&self, diff: GitDiff) -> Result<StructuredDiff, Error>;
}

pub struct StructuredDiff {
    pub files: Vec<DiffFile>,
    pub summary: DiffSummary,
}

pub struct DiffFile {
    pub path: PathBuf,
    pub language: Option<String>,
    pub change_type: ChangeType,
    pub hunks: Vec<DiffHunk>,
}

pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed { old_path: PathBuf },
    Binary,
}
```

#### 3.2.3 Tree-sitter 解析器

```rust
pub struct CodeAnalyzer {
    pub language_manager: LanguageManager,
    pub ast_mapper: AstMapper,
}

impl CodeAnalyzer {
    pub fn new() -> Self;
    pub fn analyze_diff(&self, structured_diff: StructuredDiff) -> Result<CodeAnalysis, Error>;
    pub fn identify_structure_changes(&self, file: &DiffFile) -> Result<StructureChanges, Error>;
}

pub struct CodeAnalysis {
    pub file_analyses: Vec<FileAnalysis>,
    pub cross_file_analysis: Option<CrossFileAnalysis>,
}

pub struct FileAnalysis {
    pub path: PathBuf,
    pub structure_changes: StructureChanges,
    pub code_metrics: CodeMetrics,
}

pub struct StructureChanges {
    pub functions: Vec<FunctionChange>,
    pub classes: Vec<ClassChange>,
    pub imports: Vec<ImportChange>,
    pub namespace_changes: Vec<NamespaceChange>,
}
```

#### 3.2.4 分析规则引擎

```rust
pub struct RuleEngine {
    pub rule_sets: HashMap<RuleCategory, Vec<Box<dyn Rule>>>,
    pub config: RuleConfig,
}

impl RuleEngine {
    pub fn new(config: RuleConfig) -> Self;
    pub fn apply_rules(&self, analysis: CodeAnalysis) -> Result<ReviewResult, Error>;
    pub fn load_language_rules(&mut self, language: &str) -> Result<(), Error>;
}

pub struct RuleConfig {
    pub enabled_categories: Vec<RuleCategory>,
    pub depth: AnalysisDepth,
    pub custom_rules_path: Option<PathBuf>,
    pub ignore_rules: Vec<String>,
}

pub enum RuleCategory {
    Style,
    Security,
    Performance,
    Complexity,
    BestPractices,
    Bugs,
}

pub trait Rule {
    fn name(&self) -> &str;
    fn category(&self) -> RuleCategory;
    fn apply(&self, context: &RuleContext) -> Vec<Issue>;
    fn severity(&self) -> Severity;
    fn is_applicable(&self, language: &str) -> bool;
}
```

#### 3.2.5 报告生成器

```rust
pub struct ReportGenerator {
    pub format: OutputFormat,
    pub template_engine: TemplateEngine,
}

impl ReportGenerator {
    pub fn new(format: OutputFormat) -> Self;
    pub fn generate(&self, review_result: ReviewResult) -> Result<Report, Error>;
    pub fn save_to_file(&self, report: &Report, path: &Path) -> Result<(), Error>;
}

pub struct Report {
    pub issues: Vec<Issue>,
    pub summary: ReviewSummary,
    pub rendered_content: String,
}

pub struct Issue {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: CodeLocation,
    pub severity: Severity,
    pub category: RuleCategory,
    pub code_snippet: Option<String>,
    pub suggestion: Option<String>,
}

pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}
```

#### 3.2.6 AI 增强模块

```rust
pub struct AIReviewer {
    pub ai_client: AIClient,
    pub prompt_manager: PromptManager,
}

impl AIReviewer {
    pub fn new(ai_client: AIClient, prompt_manager: PromptManager) -> Self;
    pub fn enhance_review(&self, code_analysis: &CodeAnalysis, language: &str) -> Result<AIReviewInsights, Error>;
    pub fn generate_improvement_suggestions(&self, issues: &[Issue]) -> Result<Vec<Suggestion>, Error>;
}

pub struct PromptManager {
    pub language_prompts: HashMap<String, String>,
    pub default_prompt: String,
}

impl PromptManager {
    pub fn new() -> Self;
    pub fn load_language_prompt(&mut self, language: &str) -> Result<(), Error>;
    pub fn get_prompt_for_language(&self, language: &str) -> &str;
}

pub struct AIReviewInsights {
    pub security_concerns: Vec<SecurityConcern>,
    pub code_quality_insights: Vec<CodeQualityInsight>,
    pub improvement_suggestions: Vec<Suggestion>,
}
```

#### 3.2.7 用户界面

```rust
pub struct ReviewUI {
    pub interactive: bool,
    pub color_enabled: bool,
}

impl ReviewUI {
    pub fn new(interactive: bool, color_enabled: bool) -> Self;
    pub fn display_report(&self, report: &Report) -> Result<(), Error>;
    pub fn prompt_user_action(&self, report: &Report) -> Result<UserAction, Error>;
}

pub enum UserAction {
    Continue,
    FixAndReview,
    IgnoreAndContinue { ignored_issues: Vec<String> },
    Cancel,
}
```

### 3.3 与现有架构集成

代码评审功能将与现有的 Gitie 组件集成：
- 使用已实现的 Tree-sitter 语法分析能力
- 利用现有的 Git 命令执行模块
- 与 AI 助手模块集成，提供智能建议
- 复用配置管理系统

## 4. 数据流

### 4.1 独立评审命令流程

1. 解析 `gitie review` 命令参数
2. 提取指定 commit 范围的 diff
3. 解析 diff 内容，构建结构化表示
4. 使用 Tree-sitter 分析代码结构变化
5. 应用评审规则引擎
6. 生成评审报告
7. 显示评审结果

### 4.2 集成提交评审流程

1. 解析 `gitie commit --review` 命令参数
2. 检查暂存区变更
3. 提取暂存区 diff
4. 执行完整评审流程（步骤 3-6 同上）
5. 显示评审结果
6. 提示用户操作选择：
   - 继续提交
   - 修复后重新评审
   - 忽略问题并提交
   - 取消操作
7. 根据用户选择执行相应操作

## 5. 命令行接口设计

### 5.0 语言专用安全审核提示

项目已经为不同编程语言配置了专用的安全审核提示（prompt），这些提示文件存放在 `assets/` 目录下，命名格式为 `review-<language>-prompt.md`：

| 语言 | 提示文件 |
|------|----------|
| C | `review-c-prompt.md` |
| C++ | `review-cpp-prompt.md` |
| Go | `review-go-prompt.md` |
| Java | `review-java-prompt.md` |
| JavaScript | `review-js-prompt.md` |
| Python | `review-python-prompt.md` |
| Rust | `review-rust-prompt.md` |

每个语言的提示文件包含了专门针对该语言的：
- 安全框架和核心原则
- 常见风险类别和对应的安全模式
- 技术实施建议
- 常见漏洞（CWE）映射
- 典型的危险模式和安全替代方案

在执行代码评审时，系统会根据被评审代码的语言自动加载相应的提示作为 AI 的系统提示（system prompt），以确保生成的评审结果充分考虑到特定语言的安全特性和最佳实践。

### 5.1 `gitie review` 命令

```
USAGE:
    gitie review [OPTIONS] [COMMIT1] [COMMIT2]

ARGS:
    <COMMIT1>    起始提交（可选，默认为 HEAD~1）
    <COMMIT2>    结束提交（可选，默认为 HEAD）

OPTIONS:
    --depth <LEVEL>          分析深度 [可选: basic, normal, deep] [默认: normal]
    --focus <AREA>           关注领域 [可选: security, performance, style, all] [默认: all]
    --lang <LANGUAGE>        限定分析语言 [可选: python, rust, java, cpp, go, js, all] [默认: all]
    --format <FORMAT>        输出格式 [可选: text, json, html] [默认: text]
    --output <FILE>          输出到文件 [默认: 控制台输出]
    --ts                     使用 Tree-sitter 增强分析 [默认: 启用]
    --no-ts                  禁用 Tree-sitter 分析
    --review-ts              --review 和 --ts 的组合形式
    --help                   显示帮助信息
```

### 5.2 `gitie commit` 集成

```
USAGE:
    gitie commit [OPTIONS] [-- <GIT COMMIT OPTIONS>]

OPTIONS:
    --review                 提交前执行代码评审
    --depth <LEVEL>          分析深度 [可选: basic, normal, deep] [默认: normal]
    --focus <AREA>           关注领域 [可选: security, performance, style, all] [默认: all]
    --ts                     使用 Tree-sitter 增强分析 [默认: 启用]
    --no-ts                  禁用 Tree-sitter 分析
    --review-ts              --review 和 --ts 的组合形式
    --help                   显示帮助信息

    其他所有 git commit 标准选项也受支持
```

## 6. 配置系统

### 6.1 评审配置项

```toml
[review]
# 评审深度设置
depth = "normal"  # basic, normal, deep

# 关注领域
focus = ["security", "style"]  # 可选：security, performance, style, semantics

# 报告设置
max_issues = 50  # 限制显示的问题数量

[languages]
# 启用的语言
enabled = ["rust", "python", "java", "cpp", "go", "javascript"]
default = "all"

[rules]
# 忽略特定规则
ignore = ["unused_variable", "long_function"]
# 自定义规则路径
custom_rules = "path/to/custom/rules"

[output]
# 输出格式
format = "terminal"  # terminal, json, html
colorize = true
grouping = "severity"  # severity, file, type

[ai]
# AI相关配置
enable = true
# 是否使用语言专用提示
use_language_prompts = true
# 默认提示文件（当找不到语言专用提示时使用）
default_prompt = "assets/expert-prompt.md"
# 提示文件路径模板
prompt_template = "assets/review-{language}-prompt.md"
```

## 7. 实现计划

### 7.1 阶段一：基础架构（1-2周）

- 设计并实现 `gitie review` 命令基础结构
- 开发 Diff 解析器组件
- 集成现有 Tree-sitter 模块
- 实现基础规则引擎框架
- 集成语言专用安全审核提示系统

### 7.2 阶段二：核心功能（2-3周）

- 实现 Rust 代码分析引擎
- 创建基础规则集
- 开发报告生成器
- 实现终端输出格式
- 设计并实现参数缩写系统
- 集成 `commit` 和 `review` 命令
- 完善 AI 增强模块与安全审核提示的集成

### 7.3 阶段三：优化与完善（1周）

- 实现交互式评审结果处理
- 完成配置系统
- 进行测试与性能优化
- 编写文档与示例

## 8. 测试策略

### 8.1 单元测试

- 命令解析器测试
- Diff 解析器测试
- 规则引擎测试
- 报告生成器测试

### 8.2 集成测试

- 端到端测试 `gitie review` 命令
- 端到端测试 `gitie commit --review` 流程
- 测试不同语言的代码评审
- 测试各种评审深度和关注点

### 8.3 性能测试

- 大型仓库评审性能测试
- 多语言项目评审性能测试
- 内存占用测试

## 9. 风险与缓解

1. **性能问题**：大型项目可能导致分析性能下降
   - 缓解：实现增量分析和结果缓存

2. **语言支持不完善**：某些语言特性可能无法完全解析
   - 缓解：按优先级实现语言支持，提供降级功能

3. **Git 集成复杂性**：与 Git 工作流集成可能引入兼容性问题
   - 缓解：全面测试各种 Git 工作流场景

4. **用户体验**：交互式界面可能不符合用户期望
   - 缓解：进行用户测试，提供可配置选项

## 10. 总结

代码评审功能将显著增强 Gitie 的能力，提供深入的代码理解和分析能力。通过与 Tree-sitter 和 AI 组件的紧密集成，我们能够提供比传统工具更有见解的代码评审。

这个设计实现了独立的 `gitie review` 命令和集成到 `gitie commit` 的评审功能，为用户提供了灵活的使用方式。特别注意了参数设计与 Git 的兼容性，确保不破坏现有工作流。