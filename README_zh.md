# Git Enhancer

`gitie` 是一个命令行工具，它通过 AI 功能增强您的 Git 工作流。它可以自动生成提交信息，并为 Git 命令提供 AI 驱动的解释。

## 功能特性

-   **AI 驱动的提交信息**：通过分析您暂存的 diff，使用大型语言模型 (LLM) 自动生成提交信息。
-   **AI 驱动的 Git 命令解释**：直接在您的终端中获取对 Git 命令及其选项的 AI 生成的解释。
-   **标准 Git Commit 传递**：与您现有的 `git commit` 工作流无缝集成。如果您不使用 AI 功能，它的行为与标准 `git commit` 相同。
-   **可配置**：允许自定义 AI 模型、API 端点、temperature (温度) 和系统提示。
-   **追踪/日志**：提供详细的日志用于调试和监控。

## 安装

1.  **先决条件**：
    *   Rust 和 Cargo：[安装 Rust](https://www.rust-lang.org/tools/install)
    *   Git：必须已安装并在您的 PATH 环境变量中。
    *   （可选）一个 OpenAI 兼容的 LLM API 端点（例如，本地运行的 Ollama 模型，或远程服务）。

2.  **从源码构建**：
    ```bash
    git clone <repository_url> # 请替换为实际的仓库 URL
    cd gitie
    cargo build --release
    ```
    可执行文件将位于 `target/release/gitie`。您可以将其复制到您 PATH 环境变量中的目录，例如 `~/.local/bin/` 或 `/usr/local/bin/`。

    ```bash
    # 示例：
    # mkdir -p ~/.local/bin
    # cp target/release/gitie ~/.local/bin/
    # 确保 ~/.local/bin 在您的 PATH 中
    ```

## 配置

`gitie` 在其根目录中使用 `config.toml` 文件进行 AI 相关设置，并使用 `prompts/commit-prompt` 文件作为生成提交信息时使用的系统提示。

1.  **创建 `config.toml`**：
    将示例配置文件 `config.example.toml` 复制到 `gitie` 项目的根目录下，并重命名为 `config.toml`（如果它是全局安装并且期望在那里找到配置文件，则复制到运行可执行文件的目录——这可能需要针对全局安装进行调整）。

    ```bash
    cp config.example.toml config.toml
    ```

    编辑 `config.toml` 并填入您的首选设置：
    ```toml
    [ai]
    api_url = "http://localhost:11434/v1/chat/completions"  # 您的 LLM API 端点
    model_name = "qwen3:32b-q8_0"                           # 要使用的模型
    temperature = 0.7                                        # LLM temperature (温度)
    api_key = "YOUR_API_KEY_IF_NEEDED"                       # API 密钥，如果您的端点需要
    ```
    *   `[ai]`: AI相关配置设置的部分
        *   `api_url`: 您的 OpenAI 兼容的聊天补全端点的 URL。
        *   `model_name`: 您的 API 端点期望的特定模型标识符。
        *   `temperature`: 控制 AI 的创造力。较高的值意味着更具创造性/随机性，较低的值意味着更具确定性。
        *   `api_key`: 您的 API 密钥，如果服务需要。这是可选的。

2.  **自定义 `prompts/commit-prompt`**：
    `prompts/commit-prompt` 文件包含提供给 AI 的系统提示，以指导其生成提交信息。您可以编辑此文件以更改提交信息的风格、语气或特定要求。

    默认提示鼓励使用约定式提交 (conventional commit) 风格的信息。

    *注意：如果找不到 `config.toml`，`gitie` 将使用默认值，但如果缺少 `prompts/commit-prompt`，它将失败。*

## 使用方法

`gitie` 根据提供的参数智能地解释您的命令，特别是 `--ai`、`-h` 和 `--help` 标志。以下是命令处理的详细说明：

**优先级 1：帮助请求 (`-h` 或 `--help`)**

如果您的命令包含帮助标志 (`-h` 或 `--help`)：

*   **使用 `--ai`**：`gitie` 获取该命令的标准 Git 帮助文本（在传递给 `git` 的参数中移除 `--ai`），然后提供该帮助文本的 AI 生成解释。`--ai` 标志可以出现在参数的任何位置。
    ```bash
    # AI 解释 'git commit' 的帮助页面
    gitie commit --help --ai
    gitie --ai commit --ai -S

    # AI 解释 'git status --short' 的帮助页面
    gitie status -s --help --ai
    ```
*   **不使用 `--ai`**：命令直接传递给 Git 以显示其标准帮助信息。
    ```bash
    gitie commit --help  # 显示标准的 'git commit --help'
    gitie status -s --help # 显示标准的 'git status -s --help'
    ```

**优先级 2：`gitie` 特定子命令 (无帮助标志)**

如果没有帮助标志，`gitie` 尝试将命令解析为其自身定义的子命令（目前，只有 `commit` 是功能完整的子命令）。

*   **`gitie commit` 子命令：**
    这是与 `gitie` 自身功能交互的主要方式。
    *   **AI 提交信息生成 (`commit --ai`)**：这是 `commit` 子命令的核心 AI 功能。它分析您的更改并生成提交信息。
        ```bash
        # 如果您已经暂存了文件：
        git add .
        gitie commit --ai
    
        # 自动暂存所有已跟踪的修改文件并生成 AI 提交信息（类似于 git commit -a）：
        gitie commit --ai -a
        # 或者
        gitie commit --ai --all
    
        # 生成 AI 提交信息并使用 GPG 签名提交
        gitie commit --ai -S
    
        # 组合自动暂存与其他选项：
        gitie commit --ai -aS
        ```
    *   **标准提交**：如果在 `commit` 子命令中不使用 `--ai` 进行信息生成，`gitie` 的行为与标准的 `git commit` 一样，直接传递参数。
        ```bash
        gitie commit -m "我的手动提交信息"
        gitie commit --amend # 打开编辑器修改上一次提交
        ```

**优先级 3：通用 Git 命令的全局 AI 解释 (无帮助标志，且未解析为 `gitie` 特定子命令)**

如果命令不包含帮助标志，且 `gitie` 无法将其解析为自身的特定子命令（例如，`gitie status` 或 `gitie --ai status`，因为 `status` 不是 `gitie` 的子命令）：

*   **如果存在 `--ai`**：`gitie` 将提供 Git 命令的 AI 生成解释。它首先从参数中移除所有 `--ai` 出现，然后让 AI 解释剩余的命令。
    ```bash
    # AI 解释 'git status -s' 的功能
    # (raw_cli_args: ["--ai", "status", "-s"] -> AI 解释 "git status -s")
    gitie --ai status -s

    # AI 解释 'git log --oneline -n 5' 的功能
    # (raw_cli_args: ["--ai", "log", "--oneline", "-n", "5"] -> AI 解释 "git log --oneline -n 5")
    gitie --ai log --oneline -n 5

    # AI 解释 'git commit -m "message"' 的功能
    # 这是因为对于 ["--ai", "commit", ...] 的 `GitEnhancerArgs` 解析会由于初始的 "--ai" 而失败，
    # 因此落入全局 AI 解释逻辑。
    # (raw_cli_args: ["--ai", "commit", "-m", "A message"] -> AI 解释 "git commit -m \"A message\"")
    gitie --ai commit -m "一条标准的提交信息"

    # AI 解释 'git commit' 的功能
    # (raw_cli_args: ["--ai", "commit", "--ai"] -> `GitEnhancerArgs` 解析失败。
    # 应用全局 --ai 逻辑。在移除两个 "--ai" 后，AI 解释 "git commit"。)
    gitie --ai commit --ai 
    ```
*   **如果只提供 `--ai`**（例如，`gitie --ai` 没有其他参数）：默认解释 `git --help`。
    ```bash
    gitie --ai # AI 解释 "git --help"
    ```

**优先级 4：传递给 Git (无帮助标志，非 `gitie` 子命令，无全局 `--ai`)**

如果命令不包含帮助标志，不被识别为 `gitie` 子命令，且不包含用于解释的全局 `--ai` 标志，它将直接传递给您系统的 `git` 安装。
```bash
gitie status -s  # 执行 'git status -s'
gitie push origin main # 执行 'git push origin main'
gitie branch my-new-feature # 执行 'git branch my-new-feature'
```

### 4. 日志记录

`gitie` 使用 `tracing` 进行日志记录。默认情况下，日志会打印到标准错误输出。您可以使用 `RUST_LOG` 环境变量来控制日志级别。

示例：
```bash
RUST_LOG=debug gitie commit --ai
```

## 工作流图 (AI 提交)

```mermaid
graph TD
    A[\"用户暂存更改: git add .\"] --> B{\"用户运行: gitie commit --ai\"};
    B --> C{\"gitie 启动\"};
    C --> D[\"加载 config.json 和 prompts/commit-prompt\"];
    D --> E[\"运行: git diff --staged\"];
    E --> F{\"有暂存的更改吗?\"};
    F -- \"否\" --> G[\"通知用户，退出或传递给 git commit\"];
    F -- \"是\" --> H[\"提取 diff 文本\"];
    H --> I[\"准备 AI 请求 (diff + 提示)\"];
    I --> J[\"发送请求到 LLM API\"];
    J --> K[\"接收 AI 生成的提交信息\"];
    K --> L{\"信息有效吗?\"};
    L -- \"否\" --> M[\"记录警告/错误，可能使用回退方案\"];
    L -- \"是\" --> N[\"构造: git commit -m \\\"<AI_MESSAGE>\\\"\"];
    N --> O[\"执行 git commit 命令\"];
    O --> P[\"记录成功/失败\"];
    P --> Q[\"退出\"];
```

## 开发

有关项目结构、贡献指南等更多详细信息，请参阅 `doc/DEVELOPMENT.md`。

### 开发者快速链接
- 构建: `cargo build`
- 运行测试: `cargo test` (添加测试后)
- 格式化: `cargo fmt`
- 代码检查: `cargo clippy`

## 许可证

本项目采用 [MIT 许可证](LICENSE)授权。（假设是 MIT，如果您选择此许可证，请添加一个 LICENSE 文件）
