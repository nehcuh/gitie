## Git 增强工具产品需求文档

### 1. 引言

#### 1.1 项目概述
本项目旨在开发一款基于 Rust 的 Git 扩展工具，通过集成人工智能（AI）和代码扫描功能，增强现有的 Git 命令行体验，提高开发效率和代码质量。

#### 1.2 项目目标
*   通过 `gitie` 的 `--ai` 标志激活 AI 功能，实现：
    *   自动生成规范的 Git Commit Message。
    *   对用户指定的 Git 命令进行 AI 解释。
*   [未来功能] 在 `git add` 阶段集成代码扫描能力，提前发现潜在问题。
*   [未来功能] 优化提交流程，将代码扫描结果与 Commit 信息关联。
*   提供灵活的配置选项，并确保工具的易用性和可扩展性。

#### 1.3 目标用户
所有使用 Git 进行版本控制的开发者。

### 2. 功能需求

#### 2.1 AI 辅助 Git Commit

*   **需求描述**：用户在使用 `gitie commit` 执行 Git Commit 相关操作时，可以通过添加 `--ai` 参数 (此为 `commit` 子命令特有的标志，区别于全局 `--ai` 命令解释标志)，让工具自动生成 Commit Message。此外，用户还可以通过添加 `-a` 或 `--all` 标志，实现提交前自动暂存所有已跟踪且已修改的文件，类似于 `git commit -a` 的行为。

*   **实现逻辑**：
    1.  **AI Commit Message 生成 (`gitie commit --ai`)**：
        1.  当用户执行 `gitie commit --ai [其他git commit参数]` 时，工具解析到 `commit` 子命令及其 `--ai` 标志。
        2.  **自动暂存 (如果使用 `-a` 或 `--all` 标志)**：
            *   如果用户同时提供了 `-a` 或 `--all` 标志 (例如 `gitie commit --ai -a`)，工具首先执行 `git add -u` 来暂存所有已跟踪文件中被修改或删除的内容。
            *   如果 `git add -u` 执行失败，则报错并终止操作。
        3.  **获取变更**：工具执行 `git diff --staged` 获取已暂存 (包括上一步自动暂存的) 的代码变更内容。
        4.  **处理无变更情况**：
            *   如果 `git diff --staged` 输出为空：
                *   若用户在 `[其他git commit参数]` 中包含了 `--allow-empty`，则 `gitie` 将会尝试执行一个标准的空提交 (例如 `git commit --allow-empty [其他git commit参数]`)，此时不调用 AI。该操作会通过 `handle_commit_passthrough` 处理，确保 `-a` (如果原始命令包含) 和 `--allow-empty` 等参数被正确传递。
                *   否则，提示用户“没有暂存的更改可供提交” (或者，如果使用了 `-a` 或 `--all`，则提示“没有已跟踪的修改文件可供提交”)，并正常退出，不产生错误码。
        5.  **调用 AI 服务**：将获取到的代码变更信息发送给配置好的人工智能服务，并附带一个用于生成 commit message 的系统提示。
        6.  **生成并执行 Commit**：AI 服务返回符合规范的 Commit Message。工具使用此 AI 生成的 Message，并结合用户提供的 `[其他git commit参数]` (除了 `-a` 或 `--all`，因为它们已经被处理)，执行 `git commit -m \"<generated_message>\" [其他git commit参数]`。
    2.  **标准 Commit (非 AI，`gitie commit`)**：
        1.  当用户执行 `gitie commit [其他git commit参数]` (不带 `commit` 子命令的 `--ai` 标志) 时，`gitie` 的行为应尽可能接近原生的 `git commit`。
        2.  **处理 `-a` 或 `--all` 标志**：如果用户提供了 `-a` 或 `--all` 标志 (例如 `gitie commit -am \"message\"` 或 `gitie commit -a`)，`gitie` 在通过 `handle_commit_passthrough` 执行最终的 `git commit` 命令时，需要确保 `-a` (或 `--all`) 标志被正确传递给原生 Git。
        3.  其他所有参数 (`[其他git commit参数]`) 也应透传给原生 `git commit`。

*   **相关配置**：AI 服务地址、API Key、模型选择、Commit Message 生成提示等（详见配置管理章节）。
*   **命令行示例**：
    *   `gitie commit --ai` (基于已暂存的更改生成 AI commit message)
    *   `gitie commit --ai -a` (自动暂存所有已跟踪的修改文件，然后生成 AI commit message)
    *   `gitie commit --ai --all -S` (自动暂存并 GPG 签名，使用 AI commit message)
    *   `gitie commit -m \"我的提交信息\"` (标准提交)
    *   `gitie commit -am \"我的提交信息\"` (自动暂存所有已跟踪的修改文件，并使用提供的 message 提交)
    *   `gitie commit -a` (自动暂存所有已跟踪的修改文件，然后 Git 可能会打开编辑器)
    *   `gitie commit --ai -a --allow-empty` (自动暂存，即使无变更也允许空提交，但不调用AI)
    *   `gitie commit --ai --allow-empty` (基于已暂存更改，若为空则允许空提交，不调用AI)

#### 2.2 Git Add 集成代码扫描 [未来功能]

*   **需求描述**：用户在执行 `git add` 命令时，可以通过添加 `--scan` 参数，触发对本次改动代码的自动扫描。
*   **实现逻辑**：
    1.  当用户执行 `git add <files> --scan` 时，工具在执行标准的 `git add` 操作后，启动代码扫描流程。
    2.  工具将当前仓库的代码（或根据 `<files>` 确定扫描范围）打包压缩。
    3.  通过 HTTP POST 请求将压缩包发送到用户配置的代码扫描系统。
    4.  在本地（如项目根目录下的 `.git/.scan_info` 或用户指定位置）创建一个跟踪文件，记录以下信息：
        *   扫描任务 ID（如果扫描系统返回）。
        *   扫描状态（例如：`pending`, `in_progress`, `completed`, `failed`）。
        *   发起扫描的时间戳。
        *   关联的 Git Commit 哈希（初始为空，在 Commit 后更新）。
*   **相关配置**：代码扫描系统地址、认证方式、打包范围配置等。
*   **命令行示例**：`git add . --scan`

#### 2.3 Commit 流程与代码扫描结果联动 [未来功能]

*   **需求描述**：用户执行 `git commit` 时，如果检测到存在由 `--scan` 触发且尚未完成或未关联到 Commit 的扫描任务，应提示用户并允许用户等待扫描结果。
*   **实现逻辑**：
    1.  当用户执行 `git commit`（无论是否带其子命令的 `--ai` 参数）时，工具检查本地是否存在活动的扫描跟踪文件。
    2.  **如果存在活动的扫描任务**（例如状态为 `pending` 或 `in_progress`）：
        *   向用户提示：“检测到正在进行的代码扫描任务。是否等待扫描结果并将其信息包含在 Commit Message 中？(yes/no/details)”。
        *   如果用户选择 `yes`：工具将轮询代码扫描系统获取最新状态。获取成功后，可以将扫描结果摘要或链接提供给用户，或（如果使用 `commit --ai`）作为附加上下文信息提供给 AI 生成 Commit Message。
        *   如果用户选择 `no`：继续执行 Commit 流程，不等待扫描结果。
        *   如果用户选择 `details`：显示当前扫描任务的详细状态。
    3.  **如果扫描任务已完成但未关联 Commit**：
        *   提示用户可以将扫描结果（如问题摘要、通过状态）包含在 Commit Message 中。
    4.  Commit 成功后，更新扫描跟踪文件，将当前 Commit 的哈希值与对应的扫描任务关联起来。

#### 2.4 配置文件管理

*   **需求描述**：工具应支持通过配置文件进行个性化设置，并在首次使用时自动创建和配置。
*   **实现逻辑**：
    1.  **配置文件位置**：在代码仓库的根目录下，例如 `.gitie_config.toml`。
    2.  **自动创建**：当工具首次在项目中执行或未找到配置文件时，自动创建一个包含默认设置的配置文件。
    3.  **.gitignore 集成**：首次创建配置文件时，自动将该配置文件名添加到项目的 `.gitignore` 文件中，以避免将本地配置（尤其是 API 密钥等敏感信息）提交到仓库。
    4.  **配置内容示例**：
        ```toml
        [ai]
        # AI 服务提供商，例如 "openai", "gemini" 等
        provider = "openai"
        api_key = "YOUR_OPENAI_API_KEY" # 提示用户填写
        model = "gpt-3.5-turbo"
        # 生成 commit message 的 prompt 模板等
        commit_message_prompt = "Generate a concise git commit message in conventional commit format for the following changes:"

        # [未来功能] 代码扫描相关配置
        [code_scan]
        # 代码扫描系统地址
        endpoint = "https://your.codescan.server/api/scan"
        # 认证 Token 或其他凭证
        auth_token = "YOUR_SCAN_AUTH_TOKEN" # 提示用户填写
        # 扫描历史/跟踪文件路径，相对于 .git 目录
        tracking_file = ".scan_info"

        # [未来功能] 历史记录相关配置 (主要配合代码扫描)
        [history]
        # 用于记录扫描历史的定位点等，例如上次扫描的commit
        last_scanned_commit = ""
        ```
    5.  **配置加载**：工具启动时加载配置文件，优先使用项目级配置。未来可考虑支持用户级的全局配置。

#### 2.5 AI 驱动的 Git 命令解释与辅助 (全局 --ai 标志)

*   **需求描述**：
    `gitie` 将通过一个全局性的 `--ai` 标志提供 AI 辅助功能。此标志的行为取决于它在参数列表中的位置以及其他参数。

*   **核心逻辑与行为**：
    1.  **参数处理概述**：
        *   工具扫描所有原始命令行参数。
        *   识别**第一个出现**的 `--ai` 标志。这个 `--ai` 作为“全局 AI 模式”开关。
        *   第一个全局 `--ai` 之后出现的任何 `--ai` 标志都被视为后续命令的普通参数。

    2.  **场景化行为**：
        *   **场景 A：全局 `--ai` 标志存在**
            *   用于后续处理的参数 (称作 `effective_command_args`) 是指除第一个全局 `--ai` 之外的所有原始参数，并保持它们原有的相对顺序。
            *   **子场景 A.1：`effective_command_args` 包含 `-h` 或 `--help`**
                *   **操作**：`gitie` 构建并执行由 `git` 和 `effective_command_args` 组成的 Git 命令。捕获该命令的标准输出（即帮助文本），然后将其发送给 AI 服务进行解释。AI 生成的解释将显示给用户。
                *   **示例**：`gitie status --ai --short --help` (全局 `--ai` 被检测到，`effective_command_args` = `[\"status\", \"--short\", \"--help\"]`。AI 解释 `git status --short --help` 的输出。)
            *   **子场景 A.2：`effective_command_args` 不包含 `-h` 或 `--help`**
                *   `gitie` 尝试将 `effective_command_args` 解析为其自身的特定子命令 (例如 `commit`)。
                *   **若 `effective_command_args` 成功解析为 `gitie` 的特定子命令，并且该子命令实例本身指示了 AI 操作** (例如 `commit` 子命令的 `ai` 标志被激活)：
                    *   **操作**：执行该 `gitie` 子命令的特定 AI 功能 (例如 AI 生成提交信息)。
                    *   **示例**：`gitie --ai commit --ai -m \"Initial commit\"` (全局 `--ai` 被检测到。`effective_command_args` = `[\"commit\", \"--ai\", \"-m\", \"Initial commit\"]`。这被解析为 `gitie commit` 且其自身的 `--ai` 标志被激活，从而触发 AI 提交信息生成功能。)
                *   **否则** (若 `effective_command_args` 未解析为 `gitie` 的特定 AI 子命令，或解析成功但该子命令实例未激活其内部 AI 功能)：
                    *   **操作**：全局 `--ai` 标志的存在意味着用户希望 AI 解释由 `git` 加上 `effective_command_args` 所代表的 Git 命令。这些 `effective_command_args` 被发送给 AI 服务以解释命令的用途和行为。AI 生成的解释将被显示。
                    *   **示例 1**：`gitie --ai status --short` (全局 `--ai` 被检测到。`effective_command_args` = `[\"status\", \"--short\"]`。AI 解释 `git status --short` 命令的作用。)
                    *   **示例 2**：`gitie --ai commit -m \"A normal commit\"` (全局 `--ai` 被检测到。`effective_command_args` = `[\"commit\", \"-m\", \"A normal commit\"]`。这被解析为 `gitie commit`，但其用于生成消息的特定 `ai` 标志未激活。因此，AI 解释 `git commit -m \"A normal commit\"` 命令的作用。)

        *   **场景 B：全局 `--ai` 标志不存在**
            *   `gitie` 尝试将所有提供的参数解析为其自身的特定子命令 (例如 `commit`)。
            *   **若参数成功解析为 `gitie` 的特定子命令**：
                *   **操作**：执行为该子命令定义的逻辑。这包括处理其自身的标志 (例如 `gitie commit --ai` 会触发 AI 信息生成；`gitie commit -m \"msg\"` 会执行标准的提交透传)。
            *   **若参数未解析为 `gitie` 的特定子命令**：
                *   **操作**：所有参数将直接传递给系统的原生 `git` 命令执行。
                *   **示例**：`gitie status -s` 执行 `git status -s`。

*   **相关配置**：AI 服务地址、API Key、模型选择、命令解释系统提示等 (可复用或扩展 `[ai]` 配置节)。
*   **命令行示例汇总**：
    *   解释 Git 帮助输出: `gitie --ai status --short --help` 或 `gitie status --ai --help -s`
    *   解释 Git 命令功能: `gitie --ai log --oneline -n 5`
    *   通过全局 AI 触发 `commit` 子命令的 AI 功能: `gitie --ai commit --ai --all`
    *   直接触发 `commit` 子命令的 AI 功能: `gitie commit --ai -a` (此处的 `-a` 会被 `commit` 子命令处理)
    *   标准 `commit` 子命令用法: `gitie commit -am \"我的消息\"` (此处的 `-a` 会被 `commit` 子命令处理)
    *   标准 Git 命令透传: `gitie status -s`
    *   处理多个 `--ai` 标志: `gitie --ai status --ai --short --help` (第一个 `--ai` 作为全局标志，第二个 `--ai` 作为 `status` 命令的参数参与解释)

### 3. 技术选型与架构

#### 3.1 核心技术
*   **编程语言**：Rust。
*   **命令行参数解析**：Clap 库。

#### 3.2 架构设计
*   **模块化**：代码结构应清晰划分，例如分为命令行处理模块、Git 交互模块、AI 服务模块、代码扫描模块、配置管理模块。
*   **可扩展性**：设计时应考虑到未来功能的扩展，例如支持新的 AI 服务、新的代码扫描工具或新的 Git 子命令增强。Clap 允许通过组合不同的参数结构体来方便地扩展命令行接口。

### 4. 工作流程图 (Mermaid)

#### 4.1 AI 辅助 Commit (`gitie commit --ai [-a]`) 流程 [L173-174]

```mermaid
graph TD
    A[用户执行: gitie commit --ai [-a] <args>] --> B{工具解析 commit 子命令, --ai 标志, 及可选的 -a/--all 标志};
    B -- 带 -a/--all --> Ba[执行 'git add -u'];
    Ba -- 成功 --> C;
    Ba -- 失败 --> Bb[报错并退出];
    B --不带 -a/--all --> C;
    C --> D[执行 'git diff --staged'];
    D --> E{捕获暂存区代码变更};
    E -- 无变更 --> Ea{包含 --allow-empty 参数?};
    Ea -- 是 --> Eb[执行 git commit --allow-empty [-a] <args>];
    Eb --> I[Commit 完成];
    Ea -- 否 --> Ec[提示无变更并退出];
    E -- 有变更 --> F[将变更信息和 Commit 提示发送至配置的 AI 服务];
    F --> G[AI 服务生成 Commit Message];
    G --> H[工具接收 Commit Message];
    H --> Ha[执行 'git commit -m \"AI 生成的 Message\" <args不含-a>'];
    Ha --> I;
```

#### 4.2 代码扫描与 Commit 联动流程 (`git add --scan` 后接 `git commit`) [未来功能]

```mermaid
graph TD
    subgraph Git Add 阶段
        AA[用户执行: git add <files> --scan] --> AB{工具拦截命令};
        AB --> AC[执行 'git add <files>'];
        AC --> AD[打包仓库代码 (或指定文件)];
        AD --> AE[通过 POST 请求发送至代码扫描系统];
        AE --> AF[创建/更新本地扫描跟踪文件 (状态: pending, 扫描ID)];
    end

    subgraph Git Commit 阶段
        CA[用户执行: git commit] --> CB{工具拦截命令};
        CB --> CC{检查是否存在活动的扫描跟踪文件?};
        CC -- 是 --> CD{扫描任务进行中?};
        CD -- 是 --> CE[提示用户: 是否等待扫描结果?];
        CE -- 是 --> CF[轮询扫描系统获取状态/结果];
        CF -- 扫描完成 --> CG[获取扫描结果];
        CG --> CH[可选: 将扫描结果纳入 Commit Message (手动或通过AI)];
        CH --> CI[执行实际的 'git commit'];
        CD -- 否/已完成 --> CG;
        CE -- 否 --> CI;
        CC -- 否 --> CI;
        CI -- Commit 成功 --> CJ[更新扫描跟踪文件, 关联 Commit 哈希];
        CJ --> CK[Commit 完成];
    end
```

#### 4.3 AI 驱动的 Git 命令解释与辅助流程 (全局 `--ai`)

```mermaid
graph TD
    A[用户输入命令 (含或不含全局 --ai)] --> B{解析: 是否存在全局 --ai? (第一个 --ai)};
    B -- 是 (全局 --ai 存在) --> C{提取 effective_command_args (移除第一个 --ai)};
    C --> D{effective_command_args 含 -h 或 --help?};
    D -- 是 --> E[执行 git effective_command_args, 捕获输出];
    E --> F[AI 解释捕获的输出];
    F --> G[显示AI解释];
    G --> Z[结束];
    D -- 否 --> H{尝试解析 effective_command_args 为 gitie AI 子命令? (如 commit --ai)};
    H -- 是 (特定AI子命令) --> I[执行 gitie 子命令的 AI 功能];
    I --> Z;
    H -- 否 --> J[AI 解释 'git effective_command_args' 的功能];
    J --> G;
    B -- 否 (无全局 --ai) --> K{尝试解析原始参数为 gitie 子命令?};\
    K -- 是 --> L[执行 gitie 子命令逻辑 (可能含其内部 --ai, 或透传)];
    L --> Z;\
    K -- 否 --> M[直接透传原始参数给系统 git 执行];\
    M --> Z;
```

### 5. 非功能性需求

*   **性能**：工具的执行不应显著拖慢正常的 Git 操作。
*   **易用性**：清晰的命令行提示和错误信息。
*   **安全性**：妥善处理 API Key 等敏感信息，避免硬编码，并通过 `.gitignore` 防止泄露。
*   **兼容性**：尽可能兼容主流操作系统（Windows, macOS, Linux）。

### 6. 未来展望（本次迭代范围外）
*   支持更多的 AI 服务提供商。
*   集成多种代码扫描工具。
*   提供更丰富的扫描结果展示和交互。
*   通过 Git Hooks 实现更自动化的流程（例如 pre-commit 自动扫描）。
*   支持更细致的扫描范围配置。

希望这份产品需求文档能够帮助您清晰地规划和开发这款 Git 增强工具。
