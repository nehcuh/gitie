// This lib.rs file exposes modules for testing purposes

// Re-export modules needed for tests
pub mod ai_module;
pub mod cli_interface;
pub mod command_processing;
pub mod config_management;
pub mod core;
pub mod git_module;
pub mod tree_sitter_analyzer;

// For tests that might have been using the old paths,
// you might need to update their `use` statements.
// For example, `use crate::ai_explainer;` should become `use crate::ai_module::explainer;`
// and `use crate::commit_commands;` should become `use crate::command_processing::commit;`
// etc.

// If there were specific items re-exported for convenience,
// those re-exports would need to be updated or tests refactored.
// For example, if you had `pub use app_context::args::GitieArgs;`
// it might now be `pub use cli_interface::args::GitieArgs;`
// or tests could directly use `crate::cli_interface::args::GitieArgs`.

// The goal is to make items accessible via their new module paths.
// e.g., crate::ai_module::explainer, crate::config_management::settings, etc.