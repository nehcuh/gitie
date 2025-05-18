# Git 错误智能解释优化技术用户故事

## 技术故事 1: 更新配置加载机制支持 git-master-prompt

**作为** Gitie 开发者  
**我想要** 扩展配置加载机制，支持加载 git-master-prompt 系统提示词  
**以便于** 可以在错误解释场景中使用专门的提示词模板

### 验收标准
- config.rs 中添加了 git-master-prompt 相关的常量定义
- 配置加载逻辑能够正确加载 git-master-prompt 文件
- 提示词被正确添加到 AppConfig.prompts 映射中
- 提供测试确保配置加载正确

### 实现步骤
1. 在 config.rs 中添加常量定义:
   ```rust
   const USER_GIT_MASTER_PROMPT_FILE_NAME: &str = "git-master-prompt";
   const GIT_MASTER_PROMPT_EXAMPLE_FILE_NAME: &str = "assets/git-master-prompt";
   ```

2. 修改 `initialize_config` 方法，添加 git-master-prompt 文件路径处理:
   ```rust
   let user_git_master_prompt_path = Self::get_user_file_path(USER_GIT_MASTER_PROMPT_FILE_NAME)?;
   user_prompt_paths.insert("git-master".to_string(), user_git_master_prompt_path.clone());
   ```

3. 添加拷贝 git-master-prompt 文件的逻辑
   ```rust
   // 获取 git-master-prompt 提示文件源路径
   let assets_git_master_prompt_path = if in_test {
       // 测试环境逻辑...
   } else {
       PathBuf::from(
           std::env::var("GITIE_ASSETS_GIT_MASTER_PROMPT")
               .unwrap_or_else(|_| GIT_MASTER_PROMPT_EXAMPLE_FILE_NAME.to_string()),
       )
   };
   
   // 检查源文件存在并复制
   if !user_git_master_prompt_path.exists() {
       fs::copy(
           &assets_git_master_prompt_path,
           &user_git_master_prompt_path,
       ).map_err(|e| {
           ConfigError::FileWrite(
               format!(
                   "Failed to copy source git-master prompt file {} to target prompt file {}",
                   assets_git_master_prompt_path.display(),
                   user_git_master_prompt_path.display()
               ),
               e,
           )
       })?;
   }
   ```

### 相关文件
- `gitie/src/config.rs`

## 技术故事 2: 开发 Git 错误 AI 解释功能

**作为** Gitie 开发者  
**我想要** 创建专门用于解释 Git 错误的 AI 功能  
**以便于** 用户在遇到 Git 错误时能获得更有帮助的解释和解决方案

### 验收标准
- ai_explainer.rs 中添加了 `explain_git_error` 函数
- 该函数使用 git-master-prompt 作为系统提示词
- 错误解释包含原始错误和 AI 解释两部分
- 返回格式易于阅读且突出显示解决方案

### 实现步骤
1. 在 ai_explainer.rs 中添加新的 `explain_git_error` 函数:
   ```rust
   /// 解释 Git 错误并提供帮助信息
   ///
   /// 此函数使用专门的 git-master-prompt 系统提示词来解析 Git 错误
   /// 并提供更清晰的解释和可能的解决方案
   ///
   /// # Arguments
   ///
   /// * `config` - 应用配置，包含 AI 参数和提示词
   /// * `error_output` - Git 命令执行的错误输出
   /// * `command` - 用户执行的命令字符串，用于上下文
   ///
   /// # Returns
   ///
   /// * `Result<String, AIError>` - 格式化的错误解释或错误
   pub async fn explain_git_error(
       config: &AppConfig,
       error_output: &str,
       command: &str,
   ) -> Result<String, AIError> {
       // 验证输入
       if error_output.trim().is_empty() {
           return Ok("Git 命令未产生错误输出，但执行失败。这可能是权限问题或者其它系统级别的错误。".to_string());
       }

       tracing::debug!(
           "请求 AI 分析 Git 错误 (命令: {}): {:?}",
           command,
           error_output.chars().take(200).collect::<String>()
       );

       // 获取 git-master-prompt 系统提示词
       let system_prompt_content = config
           .prompts
           .get("git-master")
           .cloned()
           .unwrap_or_else(|| {
               tracing::warn!("Git master prompt not found in config, using empty string");
               "".to_string()
           });

       // 构建用户消息，包含命令和错误输出
       let user_message = format!(
           "在执行以下 Git 命令时遇到错误：\n\n命令: {}\n\n错误输出:\n{}\n\n请分析这个错误，解释原因并提供解决方案。",
           command, error_output
       );

       let messages = vec![
           ChatMessage {
               role: "system".to_string(),
               content: system_prompt_content,
           },
           ChatMessage {
               role: "user".to_string(),
               content: user_message,
           },
       ];

       // 发送 AI 请求并获取解释
       match execute_ai_request(config, messages).await {
           Ok(ai_explanation) => {
               // 格式化输出，包含原始错误和 AI 解释
               let formatted_output = format!(
                   "【原始 Git 错误】\n{}\n\n【Gitie AI 帮助】\n{}",
                   error_output, ai_explanation
               );
               Ok(formatted_output)
           }
           Err(e) => Err(e),
       }
   }
   ```

2. 添加必要的导入和更新函数文档

### 相关文件
- `gitie/src/ai_explainer.rs`

## 技术故事 3: 重构 passthrough_to_git 函数

**作为** Gitie 开发者  
**我想要** 重构 passthrough_to_git 函数，使其能捕获 Git 输出并判断是否需要进行错误解释  
**以便于** 保持成功命令的原有体验，同时为失败命令提供增强的错误处理

### 验收标准
- 重构后的函数能捕获 Git 命令的标准输出和错误输出
- 函数能根据命令执行状态决定返回原始输出或错误解释
- 函数保持与原始 API 兼容，但内部实现有所改变
- 添加新的函数 `passthrough_to_git_with_error_handling`，接受一个额外的 `handle_error` 参数

### 实现步骤
1. 创建新的 `passthrough_to_git_with_error_handling` 函数:
   ```rust
   /// 执行 Git 命令并可选地处理错误
   ///
   /// 执行 Git 命令，捕获输出，并根据执行状态决定是
   /// 直接输出结果还是进行错误处理
   ///
   /// # Arguments
   ///
   /// * `args` - 传递给 Git 的参数
   /// * `handle_error` - 是否处理错误 (若为 false，行为与原始函数一致)
   ///
   /// # Returns
   ///
   /// * `Result<CommandOutput, AppError>` - 命令输出或错误
   pub fn passthrough_to_git_with_error_handling(
       args: &[String],
       handle_error: bool,
   ) -> Result<CommandOutput, AppError> {
       let command_to_run = if args.is_empty() {
           vec!["--help".to_string()]
       } else {
           args.to_vec()
       };
       let cmd_str_log = command_to_run.join(" ");
       tracing::debug!("Executing system git: git {}", cmd_str_log);

       // 直接执行并获取输出，而不是只获取状态
       let output = Command::new("git")
           .args(&command_to_run)
           .output()
           .map_err(|e| {
               AppError::IO(
                   format!("Failed to execute system git: git {}", cmd_str_log),
                   e,
               )
           })?;

       let stdout = String::from_utf8_lossy(&output.stdout).to_string();
       let stderr = String::from_utf8_lossy(&output.stderr).to_string();

       // 如果不需要错误处理或命令成功执行，则直接打印输出
       if !handle_error || output.status.success() {
           // 打印标准输出和错误输出，模拟原始命令的行为
           if !stdout.is_empty() {
               print!("{}", stdout);
           }
           if !stderr.is_empty() {
               eprint!("{}", stderr);
           }
       }

       if !output.status.success() {
           tracing::warn!("Git command 'git {}' failed: {}", cmd_str_log, output.status);
           
           if !handle_error {
               // 如果不处理错误，则直接返回原始错误
               return Err(AppError::Git(GitError::PassthroughFailed {
                   command: format!("git: {}", cmd_str_log),
                   status_code: output.status.code(),
               }));
           }
       }

       Ok(CommandOutput {
           stdout,
           stderr,
           status: output.status,
       })
   }
   ```

2. 修改原始的 `passthrough_to_git` 函数，调用新函数:
   ```rust
   pub fn passthrough_to_git(args: &[String]) -> Result<(), AppError> {
       // 调用新函数，指定不处理错误
       let result = passthrough_to_git_with_error_handling(args, false)?;
       
       // 检查状态以保持原始行为一致
       if !result.status.success() {
           return Err(AppError::Git(GitError::PassthroughFailed {
               command: format!("git: {}", args.join(" ")),
               status_code: result.status.code(),
           }));
       }
       
       Ok(())
   }
   ```

### 相关文件
- `gitie/src/git_commands.rs`

## 技术故事 4: 集成错误处理到主流程

**作为** Gitie 开发者  
**我想要** 将 Git 错误 AI 解释功能集成到主程序流程中  
**以便于** 用户在使用 Gitie 时能自动获得增强的错误解释

### 验收标准
- main.rs 中更新了处理流程，支持错误智能解释
- 当用户使用 `--noai` 标志时，不进行 AI 错误解释
- 程序能正确处理命令执行成功和失败的情况
- 保持与现有功能的兼容性

### 实现步骤
1. 在 main.rs 中修改 `run_app` 函数中处理普通 Git 命令的部分:
   ```rust
   // 将这部分:
   let mut filtered_args = raw_cli_args.clone();
   filtered_args.retain(|arg| arg != "--noai");
   passthrough_to_git(&filtered_args)?;
   
   // 替换为:
   let mut filtered_args = raw_cli_args.clone();
   filtered_args.retain(|arg| arg != "--noai");
   
   // 确定是否使用 AI 解释错误
   let use_ai = should_use_ai(&raw_cli_args);
   
   // 执行 Git 命令并捕获输出
   let output = passthrough_to_git_with_error_handling(&filtered_args, use_ai)?;
   
   // 如果启用了 AI 且命令执行失败，提供 AI 错误解释
   if use_ai && !output.status.success() {
       tracing::info!("Git 命令执行失败，提供 AI 错误解释");
       
       // 合并标准错误和输出用于分析
       let mut error_text = String::new();
       if !output.stderr.is_empty() {
           error_text.push_str(&output.stderr);
       }
       if !output.stdout.is_empty() {
           if !error_text.is_empty() {
               error_text.push_str("\n");
           }
           error_text.push_str(&output.stdout);
       }
       
       // 调用错误解释函数
       match explain_git_error(&config, &error_text, &format!("git {}", filtered_args.join(" "))).await {
           Ok(explanation) => println!("{}", explanation),
           Err(e) => {
               tracing::error!("无法生成 AI 错误解释: {}", e);
               // 已经输出了原始错误，这里不需要重复输出
           }
       }
       
       // 返回适当的错误码
       return Err(AppError::Git(GitError::CommandFailed {
           command: format!("git {}", filtered_args.join(" ")),
           status_code: output.status.code(),
           stdout: output.stdout,
           stderr: output.stderr,
       }));
   }
   ```

2. 更新必要的导入和函数引用

### 相关文件
- `gitie/src/main.rs`

## 技术故事 5: 添加单元测试

**作为** Gitie 开发者  
**我想要** 为新增的错误处理功能添加单元测试  
**以便于** 确保功能稳定可靠，并在未来更改时避免回归

### 验收标准
- 为 `explain_git_error` 函数添加单元测试
- 为重构后的 `passthrough_to_git_with_error_handling` 函数添加测试
- 测试覆盖成功场景和失败场景
- 测试覆盖有 AI 和无 AI 两种模式

### 实现步骤
1. 在 ai_explainer.rs 中添加测试:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use std::collections::HashMap;

       #[tokio::test]
       async fn test_explain_git_error() {
           // 创建测试配置
           let mut config = AppConfig {
               ai: crate::config::AIConfig::default(),
               prompts: HashMap::new(),
           };
           config.prompts.insert("git-master".to_string(), "测试提示词".to_string());
           
           // 模拟错误输出
           let error_output = "git: 'comit' is not a git command. See 'git --help'.";
           let command = "git comit -m 'test'";
           
           // 实际测试需要模拟 AI 响应，此处仅为结构示例
           // TODO: 添加完整测试实现
       }
   }
   ```

2. 在 git_commands.rs 中添加测试:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_passthrough_with_error_handling_success() {
           // TODO: 实现测试逻辑
       }
       
       #[test]
       fn test_passthrough_with_error_handling_failure() {
           // TODO: 实现测试逻辑
       }
   }
   ```

### 相关文件
- `gitie/src/ai_explainer.rs`
- `gitie/src/git_commands.rs`