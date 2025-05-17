// src/tree_sitter_analyzer/mod.rs
pub mod core;
pub mod analyzer;
pub mod java;
pub mod rust;
// Future: pub mod python;
// Future: pub mod go;
// Future: pub mod javascript;

// Re-export key items for easier access from outside this module.
pub use self::analyzer::TreeSitterAnalyzer;
// pub use self::java::JavaProjectStructure; // This is now in core.rs
// Re-export language-specific functions if they are meant to be part of the public API of this module
// For example, if you want to allow direct access to Java-specific parsing outside of the TreeSitterAnalyzer facade:
// pub use self::java::{extract_java_package_name, extract_java_class_name};
// pub use self::rust::{analyze_rust_file_structure_impl};

// Ensure all public items from submodules that are needed externally are re-exported here.
