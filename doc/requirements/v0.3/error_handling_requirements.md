# Gitie Git 错误智能解释优化 PRD

## 项目背景

Gitie 是一个 Git 命令增强工具，目前当用户执行不包含 `-h` 或 `--help` 的普通 Git 命令时，Gitie 会直接将参数传递给 Git 并执行，没有额外的错误处理逻辑。这种模式下，当 Git 命令执行出错时，用户只能看到原始的 Git 错误信息，可能难以理解或找到解决方案。

## 优化需求

增强 Gitie 的错误处理能力，当用户执行普通 Git 命令出现错误时，系统将自动捕获错误信息，并使用 AI（基于 git-master-prompt 系统提示词）提供更友好的帮助信息，提升用户体验。同时，当命令正常执行时，保持原有的无干扰体验。

## 功能描述

1. **Git 错误捕获功能**:
   - 修改当前的 passthrough_to_git 函数，使其捕获 Git 命令的标准输出和错误输出
   - 分析输出内容，识别是否存在错误（通过命令退出状态码和错误输出内容）

2. **Git 错误 AI 解释**:
   - 当检测到 Git 命令执行错误时，使用 git-master-prompt 作为系统提示词
   - 将错误信息发送给 AI 进行解析和解释
   - 提供清晰的错误原因、解决方案和相关建议

3. **正常流程保持**:
   - 当 Git 命令正常执行时，保持原有行为，直接输出 Git 的结果
   - 不干扰用户的正常 Git 工作流

## 技术实现

1. **修改 `passthrough_to_git` 函数**:
   - 将其改为使用与 `execute_git_command_and_capture_output` 类似的方式捕获输出
   - 分析命令执行状态和输出内容
   - 根据分析结果决定是使用原始输出还是提供 AI 解释

2. **创建错误处理专用 AI 解释函数**:
   - 新增 `explain_git_error` 函数，使用 git-master-prompt 系统提示词
   - 将捕获的 Git 错误信息传递给 AI 进行分析
   - 返回更友好、更有帮助的错误解释和解决方案

3. **加载 git-master-prompt 系统提示词**:
   - 更新 AppConfig 的提示词加载逻辑，添加对 git-master-prompt 的支持
   - 将 git-master-prompt 正确加载到应用配置中
   - 确保在错误解释场景下使用该提示词

## 用户故事

### 用户故事 1：基础错误捕获与解释

**作为** Gitie 用户  
**我希望** 当我执行错误的 Git 命令时，能得到更清晰的错误解释  
**以便于** 我能更快地理解问题并找到解决方案

**验收标准**:
- 当 Git 命令执行失败时，Gitie 能捕获错误信息
- 使用 AI 分析错误原因并提供清晰的解释
- 显示原始错误信息和 AI 增强的解释，帮助用户快速理解问题

### 用户故事 2：提供解决方案建议

**作为** Gitie 用户  
**我希望** 看到针对遇到的 Git 错误的具体解决方案  
**以便于** 我能快速修复问题并继续工作

**验收标准**:
- AI 提供针对特定错误类型的可行解决方案
- 解决方案包含具体的命令示例
- 解决方案应考虑上下文，提供最相关的建议

### 用户故事 3：保持正常指令的无缝体验

**作为** Gitie 用户  
**我希望** 当 Git 命令正常执行时，不受任何额外操作的干扰  
**以便于** 保持我熟悉的 Git 使用体验

**验收标准**:
- 成功执行的命令保持原始 Git 输出，不添加额外信息
- 不影响命令执行性能
- 用户体验与直接使用 Git 一致

### 用户故事 4：整合 git-master-prompt 系统提示词

**作为** Gitie 开发者  
**我希望** 正确加载和使用 git-master-prompt 系统提示词  
**以便于** 提供专业的 Git 错误解释和帮助信息

**验收标准**:
- git-master-prompt 被正确加载到应用配置中
- 错误解释使用该提示词生成响应
- 提示词变更能够正确反映到错误解释中

## 技术任务拆分

1. **修改配置加载机制**
   - 更新 `config.rs` 中的配置加载逻辑，加入 git-master-prompt 的支持
   - 添加相关常量定义和路径处理

2. **新增 Git 错误解释函数**
   - 在 `ai_explainer.rs` 中创建 `explain_git_error` 函数
   - 使用 git-master-prompt 作为系统提示词
   - 处理错误信息并生成解释

3. **重构 passthrough_to_git 函数**
   - 修改 `git_commands.rs` 中的 `passthrough_to_git` 函数
   - 增加输出捕获和错误检测逻辑
   - 根据执行结果决定是否调用 AI 解释

4. **整合错误处理到主流程**
   - 更新 `main.rs` 中的处理逻辑
   - 确保错误捕获和 AI 解释正确集成到现有流程中

## 示例场景

1. **命令不存在错误**:
   ```
   $ gitie cmmit -m "fix bug"
   
   【原始 Git 错误】
   git: 'cmmit' is not a git command. See 'git --help'.
   
   【Gitie AI 帮助】
   您输入的命令有拼写错误。'cmmit' 应该是 'commit'。
   
   正确的命令应为:
   git commit -m "fix bug"
   
   'commit' 命令用于将更改记录到仓库的历史记录中。
   ```

2. **合并冲突错误**:
   ```
   $ gitie merge feature
   
   【原始 Git 错误】
   CONFLICT (content): Merge conflict in src/main.rs
   Automatic merge failed; fix conflicts and then commit the result.
   
   【Gitie AI 帮助】
   合并操作遇到了冲突，需要手动解决。
   
   解决步骤:
   1. 使用 `git status` 查看冲突文件
   2. 打开 src/main.rs 文件，找到并解决冲突部分
      - 冲突部分会被 <<<<<<< HEAD, =======, 和 >>>>>>> feature 标记
   3. 修改完成后，执行 `git add src/main.rs` 标记冲突已解决
   4. 使用 `git commit` 完成合并
   
   如需取消此次合并，可以执行 `git merge --abort`
   ```

3. **推送权限错误**:
   ```
   $ gitie push origin main
   
   【原始 Git 错误】
   ERROR: Permission to user/repo.git denied to currentuser.
   
   【Gitie AI 帮助】
   您没有推送到此仓库的权限。可能的原因:
   
   1. 认证问题: 您的 SSH 密钥或账号凭证未正确配置
      - 检查 `git config user.name` 和 `git config user.email` 是否正确
      - 验证您的 SSH 密钥: `ssh -T git@github.com`
   
   2. 仓库权限不足: 您可能没有写入权限
      - 确认您是仓库的协作者或拥有适当权限
      - 如果这是别人的仓库，考虑 fork 后推送到自己的 fork
   
   3. 远程 URL 错误: 验证远程 URL 是否正确
      - 检查当前 URL: `git remote -v`
   ```

## 开发优先级

1. 错误捕获功能（高）- 成功捕获 Git 命令错误输出是后续功能的基础
2. git-master-prompt 集成（高）- 确保系统能正确加载和使用专用提示词
3. AI 错误解释（中）- 实现错误解释逻辑并确保解释质量
4. 无干扰成功流程（中）- 确保不影响正常命令执行体验

## 后续扩展可能性

1. 错误类型分类库 - 建立常见 Git 错误类型库，提供更精确的解释
2. 用户习惯学习 - 记录用户常见错误，提供个性化帮助
3. 离线错误解释 - 为常见错误提供本地解释，减少对 AI 服务的依赖