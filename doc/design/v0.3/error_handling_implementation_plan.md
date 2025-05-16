# Git 错误智能解释优化实现计划

## 项目概述

本计划详细描述了将 Git 错误智能解释功能集成到 Gitie 的实现步骤和时间表。该功能将允许 Gitie 在用户执行 Git 命令发生错误时，使用 AI 基于 git-master-prompt 提供更友好、更有帮助的错误解释和解决方案。

## 实现阶段

### 阶段 1: 基础架构准备 (1-2天)

1. **更新配置加载逻辑**
   - 添加 git-master-prompt 相关常量定义
   - 扩展配置初始化逻辑以包含加载 git-master-prompt
   - 确保提示词正确加载到 AppConfig.prompts 映射中

2. **交付成果**
   - 更新后的 config.rs 文件
   - 配置加载单元测试
   - git-master-prompt 集成到配置系统中

### 阶段 2: 核心功能开发 (2-3天)

1. **创建 Git 错误解释功能**
   - 在 ai_explainer.rs 中添加 explain_git_error 函数
   - 实现 Git 错误解析和 AI 解释逻辑
   - 确保正确使用 git-master-prompt 系统提示词

2. **重构 Git 命令执行函数**
   - 创建 passthrough_to_git_with_error_handling 函数
   - 更新原始 passthrough_to_git 函数
   - 实现错误捕获和条件处理逻辑

3. **交付成果**
   - 更新的 ai_explainer.rs 文件
   - 重构的 git_commands.rs 文件
   - 单元测试覆盖新功能

### 阶段 3: 集成与优化 (2-3天)

1. **主程序集成**
   - 更新 main.rs 中的流程处理逻辑
   - 集成 AI 错误解释到现有命令执行流程
   - 确保与 --ai 和 --noai 标志的兼容性

2. **用户体验优化**
   - 改进输出格式，使错误解释更易读
   - 确保成功命令的无干扰体验
   - 调整 AI 解释输出样式

3. **交付成果**
   - 更新的 main.rs 文件
   - 集成测试确保功能正常运行
   - 用户体验改进

### 阶段 4: 测试与完善 (2-4天)

1. **全面测试**
   - 单元测试覆盖新增和修改的功能
   - 集成测试确保整体流程正常
   - 手动测试各种 Git 错误场景

2. **性能优化**
   - 确保 AI 调用不影响正常 Git 命令性能
   - 优化错误处理逻辑以减少延迟
   - 实现合理的超时机制

3. **文档与注释**
   - 更新代码注释和函数文档
   - 更新用户文档以说明新功能
   - 添加开发者文档以便未来维护

4. **交付成果**
   - 完整的测试套件
   - 性能优化记录
   - 更新的文档

## 具体实现细节

### 配置加载更新

```rust
// 在 config.rs 中添加
const USER_GIT_MASTER_PROMPT_FILE_NAME: &str = "git-master-prompt";
const GIT_MASTER_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/git-master-prompt";

// 更新 initialize_config 方法
let user_git_master_prompt_path = Self::get_user_file_path(USER_GIT_MASTER_PROMPT_FILE_NAME)?;
user_prompt_paths.insert("git-master".to_string(), user_git_master_prompt_path.clone());

// 添加相应的文件拷贝和检查逻辑
```

### Git 错误解释功能

```rust
// 在 ai_explainer.rs 中添加
pub async fn explain_git_error(
    config: &AppConfig,
    error_output: &str,
    command: &str,
) -> Result<String, AIError> {
    // 实现逻辑...
    
    // 获取 git-master-prompt 系统提示词
    let system_prompt_content = config
        .prompts
        .get("git-master")
        .cloned()
        .unwrap_or_else(|| {
            tracing::warn!("Git master prompt not found in config, using empty string");
            "".to_string()
        });
        
    // 构建用户消息
    let user_message = format!(
        "在执行以下 Git 命令时遇到错误：\n\n命令: {}\n\n错误输出:\n{}\n\n请分析这个错误，解释原因并提供解决方案。",
        command, error_output
    );
    
    // 调用 AI 并返回结果
    // ...
}
```

### Git 命令执行函数重构

```rust
// 在 git_commands.rs 中添加
pub fn passthrough_to_git_with_error_handling(
    args: &[String],
    handle_error: bool,
) -> Result<CommandOutput, AppError> {
    // 实现逻辑...
}

// 修改原有函数
pub fn passthrough_to_git(args: &[String]) -> Result<(), AppError> {
    // 调用新函数，并保持原有行为
}
```

### 主程序集成

```rust
// 在 main.rs 中修改相关部分
// 执行 Git 命令并捕获输出
let use_ai = should_use_ai(&raw_cli_args);
let output = passthrough_to_git_with_error_handling(&filtered_args, use_ai)?;

// 错误处理和 AI 解释逻辑
if use_ai && !output.status.success() {
    // 调用 AI 解释并处理结果
}
```

## 测试策略

1. **单元测试**
   - 为 explain_git_error 函数编写测试
   - 为 passthrough_to_git_with_error_handling 函数编写测试
   - 为配置加载更新编写测试

2. **集成测试**
   - 测试完整的命令执行流程
   - 测试各种错误场景下的行为
   - 测试与不同标志组合的兼容性

3. **测试场景**
   - 命令不存在错误
   - 拼写错误
   - 合并冲突
   - 权限错误
   - 网络连接问题
   - 配置错误

## 风险和缓解策略

1. **AI 服务可用性**
   - 风险：AI 服务不可用导致功能失败
   - 缓解：添加超时和错误处理，在 AI 服务失败时退回到原始错误输出

2. **性能影响**
   - 风险：AI 调用增加命令执行时间
   - 缓解：仅在命令失败时使用 AI，优化 AI 请求参数

3. **提示词质量**
   - 风险：git-master-prompt 不足以生成高质量解释
   - 缓解：持续优化提示词，收集用户反馈进行改进

4. **向后兼容性**
   - 风险：新功能破坏现有使用模式
   - 缓解：保持 --noai 标志的行为一致，确保无干扰的选项

## 部署计划

1. **代码提交**
   - 按功能模块分批提交代码
   - 确保每次提交都通过现有测试
   - 为新功能添加测试

2. **版本发布**
   - 在开发完成后创建 alpha 版本进行内部测试
   - 发布 beta 版本收集早期用户反馈
   - 根据反馈完善后发布正式版本

3. **文档更新**
   - 更新 README 说明新功能
   - 提供使用示例和最佳实践
   - 更新帮助文档

## 进度跟踪

使用项目的 issue tracking 系统跟踪开发进度:

- 为每个技术故事创建对应的 issue
- 使用 milestone 标记各个实现阶段
- 使用标签区分功能、修复和优化

## 总结

本实现计划提供了为 Gitie 添加 Git 错误智能解释功能的详细路线图。通过逐步实施这些阶段，我们将能够增强 Gitie 的用户体验，使其在处理 Git 错误时提供更有价值的帮助信息。