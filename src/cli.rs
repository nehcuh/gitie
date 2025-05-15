use clap::Parser;

/// Defines the command-line arguments specific to `gitie`'s own subcommands.
/// This is typically used after determining that the invocation is not a global AI explanation request.
#[derive(Parser, Debug)]
#[clap(author="Huchen", version="0.1.0", about="Git with AI support", long_about=None, name="gitie-subcommand-parser")]
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
    #[clap(long)]
    pub ai: bool,

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

#[inline]
pub fn args_contain_ai(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--ai")
}
