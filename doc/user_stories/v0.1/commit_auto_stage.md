## 用户故事：AI Commit 结合自动暂存已跟踪文件

**作为一名** 使用 `gitie` 进行 AI 辅助提交的开发者，
**我希望** 能够在 `gitie commit --ai` 命令中使用一个选项（例如 `-a` 或 `--all`），
**以便** 在 AI 分析变更并生成 Commit Message 之前，所有当前已跟踪文件中被修改或删除的内容都能被自动暂存。
**这将允许我** 快速地为所有已跟踪文件上的当前工作创建一个包含 AI 生成信息的提交，而无需预先手动执行 `git add -u` 或 `git add <specific_files>`，从而获得类似于 `git commit -a` 的便捷体验。

### 验收标准 (Acceptance Criteria):

1.  当执行 `gitie commit --ai -a` (或 `gitie commit --ai --all`) 时：
    *   工具应首先尝试暂存所有已跟踪文件中被修改或删除的内容 (类似于 `git add -u` 的效果)。
    *   如果暂存操作失败（例如，`git add -u` 返回错误），应显示相应的错误信息，并终止后续操作。
    *   如果暂存操作成功，工具接着获取已暂存的 diff (此时应包含自动暂存的更改)。
    *   如果在自动暂存后，`git diff --staged` 仍然为空（即没有已跟踪的文件被修改或删除），并且用户没有提供 `--allow-empty` 标志：
        *   工具应提示用户“没有已跟踪的修改文件可供提交”或类似信息，并正常退出（不应视为错误）。
    *   如果在自动暂存后，`git diff --staged` 为空，但用户提供了 `--allow-empty` 标志：
        *   工具应透传执行一个标准的空提交，例如 `git commit --allow-empty -a [其他原始的passthrough参数]`，此时不调用 AI。
    *   如果存在已暂存的变更，AI 应基于这些变更生成 Commit Message。
    *   工具应使用 AI 生成的 Commit Message 创建提交。最终执行的 `git commit` 命令**不应**再次包含 `-a` 或 `--all` 标志，因为暂存步骤已经独立处理完毕。其他透传参数（如 `-S`）应被保留。
2.  当执行 `gitie commit -a` (即**不带** `commit` 子命令的 `--ai` 标志) 时：
    *   工具的行为应等同于原生的 `git commit -a`。这意味着 `-a` 标志应被正确传递给底层的 `git commit` 命令，由 Git 自行处理暂存和提交的逻辑。
3.  新的 `-a`/`--all` 标志应在 `gitie commit` 命令的帮助文档中得到清晰说明。
4.  相关的 README 文件应更新以反映此新功能。 (此条目前面的步骤已处理PRD的更新)