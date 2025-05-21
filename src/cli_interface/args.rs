use clap::{Args, Parser, Subcommand};

/// Defines the command-line arguments specific to `gitie`'s own subcommands.
/// This is typically used after determining that the invocation is not a global AI explanation request.
#[derive(Parser, Debug)]
#[clap(author="Huchen", version="0.1.0", about="Git with AI support (enabled by default)", long_about=None, name="gitie-subcommand-parser")]
pub struct GitieArgs {
    #[command(subcommand)]
    pub command: GitieSubCommand,
}

/// Represents the specific subcommands that `gitie` itself understands.
#[derive(Subcommand, Debug, Clone)]
pub enum GitieSubCommand {
    /// Handle git command operation, potentially with AI assistance for message generation.
    #[clap(alias = "cm")]
    Commit(CommitArgs),
    /// Perform code review with AI assistance.
    #[clap(alias = "rv")]
    Review(ReviewArgs),
    // Future: Add(AddArgs)
    // Future: Config(ConfigArgs)
}

/// Arguments for the `commit` subcommand
#[derive(Args, Debug, Clone)]
pub struct CommitArgs {
    /// Use AI to generate the commit message (specific to the `commit` subcommand).
    /// Note: AI is enabled by default, this flag is kept for backward compatibility.
    #[clap(long)]
    pub ai: bool,

    /// Disable AI functionality and use standard git behavior.
    #[clap(long)]
    pub noai: bool,

    /// Enable Tree-sitter syntax analysis for improved commit messages.
    /// Optional value can specify analysis depth: 'shallow', 'medium' (default), or 'deep'.
    #[clap(short = 't', long = "tree-sitter", value_name = "LEVEL")]
    pub tree_sitter: Option<String>,

    /// Automatically stage all tracked, modified files before commit (like git commit -a).
    #[clap(short = 'a', long = "all")]
    pub auto_stage: bool,

    /// Pass a message directly to the commit
    #[clap(short, long)]
    pub message: Option<String>,

    /// Perform code review before commit
    #[clap(long = "review")]
    pub review: bool,

    /// Allow all other flags and arguments to be passed through to the udnerlying `git commit`.
    #[clap(allow_hyphen_values = true, last = true)]
    pub passthrough_args: Vec<String>,
}

/// Arguments for the `review` subcommand
#[derive(Args, Debug, Clone)]
pub struct ReviewArgs {
    /// Analysis depth level
    #[clap(long, value_name = "LEVEL", default_value = "normal")]
    pub depth: String,

    /// Focus areas for the review
    #[clap(long, value_name = "AREA")]
    pub focus: Option<String>,

    /// Limit analysis to specific language
    #[clap(long, value_name = "LANGUAGE")]
    pub lang: Option<String>,

    /// Output format
    #[clap(long, value_name = "FORMAT", default_value = "text")]
    pub format: String,

    /// Output file
    #[clap(long, value_name = "FILE")]
    pub output: Option<String>,

    /// Use Tree-sitter for enhanced code analysis (enabled by default)
    #[clap(long = "ts")]
    pub tree_sitter: bool,

    /// Disable Tree-sitter analysis
    #[clap(long = "no-ts")]
    pub no_tree_sitter: bool,

    /// Combined review with tree-sitter analysis
    #[clap(long = "review-ts")]
    pub review_ts: bool,

    /// First commit reference
    #[clap(long, value_name = "COMMIT")]
    pub commit1: Option<String>,

    /// Second commit reference (if comparing two commits)
    #[clap(long, value_name = "COMMIT")]
    pub commit2: Option<String>,

    /// Allow all other flags and arguments to be passed through to git.
    #[clap(allow_hyphen_values = true, last = true)]
    pub passthrough_args: Vec<String>,
}

/// Checks if a slice of string arguments contains "-h" or "--help".
#[inline]
pub fn args_contain_help(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "-h" || arg == "--help")
}

#[allow(unused)]
#[inline]
pub fn args_contain_ai(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--ai")
}

/// Checks if a slice of string arguments contains "--noai".
#[inline]
pub fn args_contain_noai(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--noai")
}

/// Determines if AI functionality should be used based on command line arguments.
/// Returns true if AI should be used (default), false if it should be disabled.
/// Logic:
/// - If --noai is present, disable AI (return false), even if --ai is also present
/// - Otherwise, enable AI (return true), regardless of whether --ai is present or not
/// - The --ai flag is kept for backward compatibility, but is not needed as AI is enabled by default
#[inline]
pub fn should_use_ai(args: &[String]) -> bool {
    !args_contain_noai(args)
}

/// Checks if a slice of string arguments contains "--tree-sitter" or "-t".
#[inline]
#[allow(dead_code)]
pub fn args_contain_tree_sitter(args: &[String]) -> bool {
    for (_i, arg) in args.iter().enumerate() {
        if arg == "--tree-sitter" || arg == "-t" {
            return true;
        }
        // Check for combined short options that include 't'
        if arg.starts_with('-') && !arg.starts_with("--") && arg.contains('t') {
            return true;
        }
    }
    false
}

/// Extracts the Tree-sitter analysis level from command line arguments.
/// Returns None if no level is specified, or the level string otherwise.
#[inline]
#[allow(dead_code)]
pub fn get_tree_sitter_level(args: &[String]) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if arg == "--tree-sitter" && i + 1 < args.len() {
            let next = &args[i + 1];
            if !next.starts_with('-') {
                return Some(next.clone());
            }
        }
        if arg.starts_with("--tree-sitter=") {
            return Some(arg.trim_start_matches("--tree-sitter=").to_string());
        }
    }
    None
}

/// Determines if Tree-sitter functionality should be used based on command line arguments.
/// Returns true if Tree-sitter should be used, false if not.
#[inline]
#[allow(dead_code)]
pub fn should_use_tree_sitter(args: &[String]) -> bool {
    args_contain_tree_sitter(args)
}

/// Generates custom help information for gitie, including gitie-specific
/// commands and options not included in standard git help.
pub fn generate_gitie_help() -> String {
    let mut help = String::new();

    // Add header and introduction
    help.push_str("gitie: Git with AI assistance\n");
    help.push_str("============================\n\n");
    help.push_str("gitie 是一个增强型 git 工具，提供 AI 辅助功能来简化 git 使用。\n");
    help.push_str("它可以像标准 git 一样使用，同时提供额外的 AI 功能。\n\n");

    // Global options
    help.push_str("全局选项:\n");
    help.push_str("  --ai                启用 AI 功能（默认已启用）\n");
    help.push_str("  --noai              禁用 AI 功能\n");
    help.push_str("  -t, --tree-sitter   启用 Tree-sitter 语法分析以改进 AI 功能\n");
    help.push_str("                      可选值: shallow, medium (默认), deep\n\n");

    // Subcommands
    help.push_str("Gitie 特有命令:\n");
    help.push_str("  commit (cm)         增强的 git commit 命令，提供 AI 生成提交信息\n");
    help.push_str("    选项:\n");
    help.push_str("      --ai            使用 AI 生成提交信息（默认）\n");
    help.push_str("      --noai          禁用 AI，使用标准 git 行为\n");
    help.push_str("      -t, --tree-sitter=LEVEL\n");
    help.push_str("                      启用 Tree-sitter 语法分析以改进提交信息\n");
    help.push_str("      -a, --all       自动暂存所有已跟踪的修改文件（类似 git commit -a）\n");
    help.push_str("      -m, --message   直接传递消息给提交\n");
    help.push_str("      --review        在提交前执行代码评审\n\n");

    help.push_str("  review (rv)         执行 AI 辅助的代码评审\n");
    help.push_str("    选项:\n");
    help.push_str("      --depth=LEVEL   分析深度级别 (默认: normal)\n");
    help.push_str("      --focus=AREA    评审重点区域\n");
    help.push_str("      --lang=LANGUAGE 限制分析到特定语言\n");
    help.push_str("      --format=FORMAT 输出格式 (默认: text)\n");
    help.push_str("      --output=FILE   输出文件\n");
    help.push_str("      --ts            使用 Tree-sitter 进行增强代码分析（默认）\n");
    help.push_str("      --no-ts         禁用 Tree-sitter 分析\n");
    help.push_str("      --review-ts     结合评审与 tree-sitter 分析\n");
    help.push_str("      --commit1=COMMIT 第一个提交引用\n");
    help.push_str("      --commit2=COMMIT 第二个提交引用（如果比较两个提交）\n\n");

    help.push_str("标准 git 命令:\n");
    help.push_str("  所有标准 git 命令都可以正常使用，例如:\n");
    help.push_str("  gitie status, gitie add, gitie push, 等等\n\n");

    help.push_str("示例:\n");
    help.push_str("  gitie commit        使用 AI 辅助生成提交信息\n");
    help.push_str("  gitie commit --noai 禁用 AI，使用标准 git commit\n");
    help.push_str("  gitie review        对当前更改执行 AI 辅助代码评审\n");
    help.push_str("  gitie review --depth=deep --focus=\"性能问题\"\n");
    help.push_str("                      执行深度代码评审，重点关注性能问题\n");

    help
}
