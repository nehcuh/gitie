# Module Reference Guide for Gitie

This document provides a comprehensive reference for the modules in the Gitie project, detailing their responsibilities, key components, and interactions. Use this guide to navigate the codebase when developing new features or modifying existing functionality.

## Table of Contents

1. [Overview](#overview)
2. [Core Modules](#core-modules)
   - [core](#core)
3. [Functional Modules](#functional-modules)
   - [ai_module](#ai_module)
   - [cli_interface](#cli_interface)
   - [command_processing](#command_processing)
   - [config_management](#config_management)
   - [git_module](#git_module)
   - [tree_sitter_analyzer](#tree_sitter_analyzer)
4. [Application Entry Points](#application-entry-points)
   - [main.rs](#mainrs)
   - [lib.rs](#librs)
5. [Module Interactions](#module-interactions)

## Overview

Gitie is structured with a clean separation of concerns across different modules:

- Core types and errors are isolated in the `core` module
- Functional capabilities are separated into dedicated modules
- The application has a clear entry point in `main.rs`
- Library functionality is exposed through `lib.rs`

This organization allows for better testability, maintainability, and extensibility.

## Core Modules

### core

The `core` module contains fundamental types and error definitions used throughout the application.

**Files:**
- `mod.rs`: Exports the module's components
- `errors.rs`: Error type definitions
- `types.rs`: Common type definitions

**Key Components:**

- `AppError`: The main error type used throughout the application
  - Variants: `ConfigError`, `GitError`, `AIError`, etc.
  
- Common types:
  - `CommandOutput`: Structured representation of command execution results
  - Various result type aliases

**Usage Examples:**
```rust
// Error handling
fn my_function() -> Result<(), core::errors::AppError> {
    // Implementation...
}

// Using common types
let output: core::types::CommandOutput = execute_command();
```

## Functional Modules

### ai_module

Handles interactions with AI services, including constructing prompts, making API calls, and processing responses.

**Files:**
- `mod.rs`: Exports module components
- `explainer.rs`: Functions for generating explanations using AI
- `utils.rs`: Utilities for working with AI, such as formatting output

**Key Components:**

- `explain_git_command`: Generates AI explanations for Git commands
- `explain_git_error`: Provides AI-powered help for Git errors
- `generate_commit_message`: Creates commit messages from diff content
- AI response formatting utilities

**Interactions:**
- Uses `config_management` for AI service configuration
- Used by `command_processing` when AI features are needed

### cli_interface

Defines the command-line interface structure and handles argument parsing.

**Files:**
- `mod.rs`: Exports module components
- `args.rs`: Command-line argument definitions (using clap)
- `ui.rs`: User interface helpers

**Key Components:**

- `GitieArgs`: Main CLI arguments structure
- `GitieSubCommand`: Enum for different commands (commit, etc.)
- `CommitArgs`: Arguments specific to the commit command
- Functions for argument checking:
  - `args_contain_help`
  - `args_contain_ai`
  - `args_contain_noai`
  - `should_use_ai`

**Interactions:**
- Used by `main.rs` to parse command line inputs
- Consumed by `command_processing` modules to determine behavior

### command_processing

Contains handlers for specific commands, implementing the application's core functionality.

**Files:**
- `mod.rs`: Exports module components
- `commit.rs`: Handles the commit command workflow

**Key Components:**

- `handle_commit`: Main function for processing commit commands
- Helper functions for different commit scenarios:
  - AI-assisted commit message generation
  - Standard Git commit passthrough

**Interactions:**
- Uses `cli_interface` to access parsed arguments
- Uses `ai_module` for generating commit messages
- Uses `git_module` to execute Git commands
- Uses `tree_sitter_analyzer` for code structure analysis

### config_management

Manages application configuration, including loading settings from files and environment variables.

**Files:**
- `mod.rs`: Exports module components
- `settings.rs`: Configuration loading and management

**Key Components:**

- `AppConfig`: Main configuration structure
- `AIConfig`: AI-specific configuration
- `TreeSitterConfig`: Code analysis configuration
- Configuration loading functions

**Interactions:**
- Used by almost all other modules that need configuration values
- Interacts with the filesystem to read configuration files

### git_module

Provides an interface for executing Git commands and processing their output.

**Files:**
- `mod.rs`: Contains Git command execution functions

**Key Components:**

- `execute_git_command_and_capture_output`: Runs Git commands and captures results
- `passthrough_to_git`: Delegates commands directly to Git
- `passthrough_to_git_with_error_handling`: Enhanced execution with error handling
- `is_git_available`: Checks if Git is installed
- `is_in_git_repository`: Checks if the current directory is a Git repository

**Interactions:**
- Used by `command_processing` to execute Git operations
- Used by `main.rs` for initialization checks

### tree_sitter_analyzer

Provides code structure analysis using the Tree-sitter parsing library.

**Files:**
- `mod.rs`: Exports module components
- `analyzer.rs`: Main analysis functionality
- `core.rs`: Core data structures for analysis
- `java.rs`: Java language-specific analysis
- `rust.rs`: Rust language-specific analysis

**Key Components:**

- `TreeSitterAnalyzer`: Main analyzer class
- Language-specific parsers and analyzers
- Data structures:
  - `FileAst`: Abstract syntax tree for a file
  - `GitDiff`: Representation of Git diff information
  - `DiffAnalysis`: Analysis results for a diff
  - Language-specific structures (e.g., `JavaProjectStructure`)

**Interactions:**
- Used by `command_processing/commit.rs` to analyze code changes
- Uses `config_management` for analyzer configuration

## Application Entry Points

### main.rs

The main application entry point, responsible for initializing the application, parsing arguments, and delegating to appropriate handlers.

**Key Responsibilities:**
- Initializes logging
- Sets up the Tokio runtime for async operations
- Parses command-line arguments
- Loads configuration
- Delegates to command handlers
- Handles top-level error reporting

**Interactions:**
- Uses most other modules to fulfill its responsibilities

### lib.rs

Exposes the application's modules for use as a library and in tests.

**Key Responsibilities:**
- Re-exports modules needed by external code
- Enables integration testing of internal components

## Module Interactions

The following diagram illustrates the main interactions between modules:

```
main.rs  →  cli_interface  →  command_processing  →  ai_module
   ↓            ↓                    ↓                  ↓
config_management  ←───────────────────────────────────┘
   ↑                                 ↓
   └───────────────────  git_module  ←  tree_sitter_analyzer
```

**Key Flows:**

1. **Command Processing Flow:**
   - `main.rs` parses arguments using `cli_interface`
   - Arguments are passed to appropriate handlers in `command_processing`
   - Handlers use services from other modules to fulfill requests

2. **AI Commit Flow:**
   - `command_processing/commit.rs` gets staged changes via `git_module`
   - Code changes are analyzed using `tree_sitter_analyzer`
   - Analysis results are sent to `ai_module` to generate a commit message
   - The message is used to create a commit via `git_module`

3. **Configuration Flow:**
   - `config_management` loads settings at startup
   - Other modules access configuration as needed

## Guidelines for Extending Modules

When adding functionality to the project:

1. **Respect module boundaries:**
   - Add new functionality to the appropriate existing module
   - Create new modules only for significant new capabilities

2. **Maintain error handling patterns:**
   - Define new error types in `core/errors.rs`
   - Use the `Result<T, AppError>` pattern for error propagation

3. **Testing:**
   - Add unit tests within module files in a `tests` submodule
   - Add integration tests in the `tests/` directory
   - Test new CLI arguments in `tests/cli_args_test.rs`

4. **Documentation:**
   - Update this guide when adding new modules or significant functionality
   - Add function-level documentation comments