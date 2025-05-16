# Gitie 开发者快速入门指南

欢迎加入 Gitie 项目开发！本指南将帮助你快速设置开发环境，理解项目架构，并学习如何为项目添加新功能。

## 1. 环境准备

### 必备工具

- **Rust 工具链**: 安装最新版本的 Rust (推荐使用 [rustup](https://rustup.rs/))
- **Git**: 版本控制系统
- **编辑器**: 推荐 VS Code + Rust Analyzer 插件

### 项目获取与初始化

```bash
# 克隆仓库
git clone https://github.com/yourusername/gitie.git
cd gitie

# 构建项目
cargo build

# 运行单元测试
cargo test
```

## 2. 项目结构概览

Gitie 是一个 Rust 项目，主要结构如下：

```
gitie/
├── assets/                # 提示词模板和配置示例
│   ├── commit-prompt      # 提交信息生成的提示词
│   ├── explanation-prompt # 命令解释的提示词
│   ├── git-master-prompt  # Git错误解释的提示词
│   └── config.example.toml # 配置示例文件
├── src/                   # 源代码
│   ├── main.rs            # 程序入口点
│   ├── lib.rs             # 库入口点，公开核心模块
│   ├── ai_explainer.rs    # AI 解释功能
│   ├── ai_utils.rs        # AI 工具函数
│   ├── cli.rs             # 命令行参数处理
│   ├── commit_commands.rs # 提交相关命令
│   ├── config.rs          # 配置加载与处理
│   ├── errors.rs          # 错误类型定义
│   ├── git_commands.rs    # Git 命令执行
│   └── types.rs           # 通用类型定义
├── tests/                 # 集成测试
└── doc/                   # 项目文档
```

## 3. 核心模块说明

### 程序流程

1. **入口点**: `main.rs` 处理命令行参数，初始化日志，并调用相应的功能模块
2. **命令解析**: `cli.rs` 使用 clap 定义命令行结构和参数
3. **配置管理**: `config.rs` 负责加载和管理配置
4. **命令执行**: `git_commands.rs` 包含 Git 命令执行函数
5. **AI 交互**: `ai_explainer.rs` 和 `ai_utils.rs` 处理与 AI 服务的交互

### 主要功能模块

- **AI 辅助提交**: `commit_commands.rs` 中的 `handle_commit` 函数
- **命令解释**: `ai_explainer.rs` 中的 `explain_git_command` 函数
- **错误解释**: `ai_explainer.rs` 中的 `explain_git_error` 函数
- **Git命令执行**: `git_commands.rs` 中的各种执行函数

## 4. 添加新功能步骤

### 步骤一：理解需求

1. 明确功能定义和范围
2. 检查 `doc/requirements` 目录了解需求文档格式
3. 创建新的需求文档和用户故事

### 步骤二：设计解决方案

1. 确定修改哪些模块或创建新模块
2. 设计函数和数据流
3. 创建设计文档，保存在 `doc/design` 目录

### 步骤三：实现功能

1. **添加命令行选项** (如果需要)
   ```rust
   // 在 cli.rs 中:
   pub struct YourNewCommand {
       #[clap(long, help = "功能描述")]
       pub your_option: bool,
   }
   ```

2. **实现核心功能**
   ```rust
   // 在你创建的新模块 your_feature.rs 或现有模块中:
   pub async fn handle_your_feature(args: YourNewCommand, config: &AppConfig) -> Result<(), AppError> {
       // 实现你的功能
       Ok(())
   }
   ```

3. **集成到主流程**
   ```rust
   // 在 main.rs 的 run_app 函数中:
   match parsed_args.command {
       GitieSubCommand::YourFeature(args) => {
           handle_your_feature(args, &config).await?;
       }
       // 其他命令...
   }
   ```

### 步骤四：添加测试

1. **单元测试**: 在模块内部添加测试
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_your_function() {
           // 测试逻辑
           assert_eq!(your_function(), expected_result);
       }
   }
   ```

2. **集成测试**: 在 `tests` 目录添加端到端测试
   ```rust
   // 在 tests/your_feature_test.rs:
   
   #[test]
   fn test_your_feature_integration() {
       // 测试逻辑
   }
   ```

### 步骤五：文档与提交

1. 更新文档，记录新功能
2. 使用有意义的提交消息
3. 提交前确保通过所有测试: `cargo test`

## 5. 具体示例：添加新的 Git 辅助命令

假设我们要添加一个新命令 `gitie branch-info` 来显示分支信息。

### 1. 更新命令行定义 (cli.rs)

```rust
// 在 GitieSubCommand 枚举中添加新命令
pub enum GitieSubCommand {
    Commit(CommitArgs),
    BranchInfo(BranchInfoArgs), // 新命令
}

// 定义命令参数结构体
#[derive(Args, Debug)]
pub struct BranchInfoArgs {
    #[clap(long, help = "显示详细信息")]
    pub detailed: bool,
    
    #[clap(long, help = "是否使用 AI 解释")]
    pub ai: bool,
    
    #[clap(long, help = "禁用 AI 功能")]
    pub noai: bool,
}
```

### 2. 创建功能模块 (branch_commands.rs)

```rust
use crate::{
    cli::BranchInfoArgs,
    config::AppConfig,
    errors::AppError,
};

pub async fn handle_branch_info(args: BranchInfoArgs, config: &AppConfig) -> Result<(), AppError> {
    // 实现分支信息获取和显示逻辑
    // 如果需要AI解释，调用ai_explainer中的函数
    
    Ok(())
}
```

### 3. 集成到主流程 (main.rs)

```rust
// 添加模块引用
mod branch_commands;
use crate::branch_commands::handle_branch_info;

// 在 run_app 函数中更新 match 逻辑
match parsed_args.command {
    GitieSubCommand::Commit(commit_args) => {
        handle_commit(commit_args, &config).await?;
    },
    GitieSubCommand::BranchInfo(branch_info_args) => {
        handle_branch_info(branch_info_args, &config).await?;
    },
}
```

### 4. 添加测试 (branch_commands_test.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_branch_info_basic() {
        // 测试逻辑
    }
}
```

## 6. 常见问题

### AI 相关问题

- **配置问题**: 确保 `config.toml` 中正确配置了 AI 服务
- **API 连接**: 检查网络和 API key 是否正确
- **提示词调试**: 修改 `assets` 目录中的提示词模板

### Rust 相关问题

- **依赖问题**: 使用 `cargo update` 更新依赖
- **编译错误**: 仔细阅读编译器提示信息
- **生命周期问题**: 考虑使用 `clone()` 或重构代码结构

## 7. 资源链接

- [Rust 官方文档](https://doc.rust-lang.org/book/)
- [Clap 命令行参数库](https://docs.rs/clap/latest/clap/)
- [Tokio 异步运行时](https://tokio.rs/tokio/tutorial)
- [Reqwest HTTP 客户端](https://docs.rs/reqwest/latest/reqwest/)

## 8. 贡献流程

1. 创建新分支: `git checkout -b feature/your-feature-name`
2. 提交更改: `git commit -m "feat: add your feature"`
3. 推送分支: `git push origin feature/your-feature-name`
4. 创建 Pull Request

祝你在 Gitie 项目中有愉快的开发体验！