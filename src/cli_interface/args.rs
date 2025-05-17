use clap::Parser;

/// Defines the command-line arguments specific to `gitie`'s own subcommands.
/// This is typically used after determining that the invocation is not a global AI explanation request.
#[derive(Parser, Debug)]
#[clap(author="Huchen", version="0.1.0", about="Git with AI support (enabled by default)", long_about=None, name="gitie-subcommand-parser")]
pub struct GitieArgs {
    #[clap(subcommand)]
    pub command: GitieSubCommand,
}

/// Represents the specific subcommands that `gitie` itself understands.
#[derive(Parser, Debug, Clone)]
pub enum GitieSubCommand {
    /// Handle git command operation, potentially with AI assistance for message generation.
    #[clap(alias = "cm")]
    Commit(CommitArgs),
    // Future: Add(AddArgs)
    // Future: Config(ConfigArgs)
}

/// Arguments for the `commit` subcommand
#[derive(Parser, Debug, Clone)]
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

    /// Allow all other flags and arguments to be passed through to the udnerlying `git commit`.
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
