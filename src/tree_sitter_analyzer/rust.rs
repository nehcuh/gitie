// src/tree_sitter_analyzer/rust.rs
use tree_sitter::{Node, Query};

use crate::{
    core::errors::TreeSitterError,
    tree_sitter_analyzer::core::{AffectedNode, FileAst, ChangePattern},
    tree_sitter_analyzer::analyzer::TreeSitterAnalyzer, // To access is_node_public
};

// Rust-specific change patterns
pub enum RustChangePattern {
    #[allow(dead_code)]
    TraitImplementation,
    #[allow(dead_code)]
    MacroChange,
    #[allow(dead_code)]
    StructuralChange,
    #[allow(dead_code)]
    VisibilityChange,
    #[allow(dead_code)]
    LifetimeChange,
}

// Function to convert Rust-specific change patterns to generic ChangePattern
pub fn to_generic_change_pattern(rust_pattern: RustChangePattern) -> ChangePattern {
    ChangePattern::LanguageSpecificChange(match rust_pattern {
        RustChangePattern::TraitImplementation => "RustTraitImplementation".to_string(),
        RustChangePattern::MacroChange => "RustMacroChange".to_string(),
        RustChangePattern::StructuralChange => "RustStructuralChange".to_string(),
        RustChangePattern::VisibilityChange => "RustVisibilityChange".to_string(),
        RustChangePattern::LifetimeChange => "RustLifetimeChange".to_string(),
    })
}

#[allow(dead_code)]
pub fn analyze_rust_file_structure_impl(file_ast: &FileAst, analyzer: &TreeSitterAnalyzer) -> Result<Vec<AffectedNode>, TreeSitterError> {
    let mut nodes = Vec::new();
    let root_node = file_ast.tree.root_node();
    let query_source = get_rust_query_pattern_str();

    let query = Query::new(tree_sitter_rust::language(), query_source)
        .map_err(|e| TreeSitterError::QueryError(format!("Failed to create Rust query: {}", e)))?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let matches = cursor.matches(&query, root_node, file_ast.source.as_bytes());

    for m in matches {
        for capture in m.captures {
            let node = capture.node;
            let capture_name = &query.capture_names()[capture.index as usize];
            
            // Helper to extract name from common name fields like "name: (identifier)"
            // Rust structures often have `name: (identifier)` or `name: (type_identifier)`
            let extract_name = |n: Node, field_name: &str| -> Option<String> {
                n.child_by_field_name(field_name)
                    .and_then(|name_node| name_node.utf8_text(file_ast.source.as_bytes()).ok().map(|s| s.to_string()))
            };

            let (node_type, name_opt) = match capture_name.as_str() {
                "function" => ("function".to_string(), extract_name(node, "name")),
                "struct" => ("struct".to_string(), extract_name(node, "name")),
                "enum" => ("enum".to_string(), extract_name(node, "name")),
                "trait" => ("trait".to_string(), extract_name(node, "name")),
                "impl_item" => {
                    // For impl blocks, the "name" is often the type being implemented or the trait for a type.
                    // This might need more sophisticated extraction, e.g., getting the text of the type node.
                    let type_node = node.child_by_field_name("type");
                    let trait_node = node.child_by_field_name("trait");
                    let name = if let Some(tn) = trait_node {
                        format!("impl {} for {}", 
                            tn.utf8_text(file_ast.source.as_bytes()).unwrap_or_default(),
                            type_node.map_or("_".to_string(), |n| n.utf8_text(file_ast.source.as_bytes()).unwrap_or_default().to_string()))
                    } else if let Some(tn) = type_node {
                        format!("impl {}", tn.utf8_text(file_ast.source.as_bytes()).unwrap_or_default())
                    } else {
                        "UnknownImpl".to_string()
                    };
                    ("impl".to_string(), Some(name))
                }
                "module" => ("module".to_string(), extract_name(node, "name")),
                "const" => ("const".to_string(), extract_name(node, "name")),
                "static" => ("static".to_string(), extract_name(node, "name")),
                "type_alias" => ("type_alias".to_string(), extract_name(node, "name")),
                "macro_definition" => ("macro".to_string(), extract_name(node, "name")),
                // "use_declaration" and "attribute" might not need a specific "name" in the same way,
                // or their "name" is the full path/text.
                "use" => ("use_declaration".to_string(), Some(node.utf8_text(file_ast.source.as_bytes()).unwrap_or_default().to_string())),
                _ => continue, // Skip other captures or non-primary captures
            };

            if let Some(name) = name_opt {
                nodes.push(AffectedNode {
                    node_type,
                    name,
                    range: (node.start_position().row, node.end_position().row),
                    is_public: analyzer.is_node_public(&node, file_ast),
                    content: Some(node.utf8_text(file_ast.source.as_bytes()).unwrap_or("").to_string()),
                    line_range: (node.start_position().row, node.end_position().row),
                });
            }
        }
    }
    Ok(nodes)
}

#[allow(dead_code)]
pub fn is_rust_node_public_impl(node: &tree_sitter::Node, file_ast: &FileAst) -> bool {
    // Check for a `visibility_modifier` child node or `pub` keyword directly.
    let mut cursor = node.walk();
    if node.child_count() > 0 && cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "visibility_modifier" {
                // The text of visibility_modifier itself will be like "pub", "pub(crate)", etc.
                if let Ok(vis_text) = child.utf8_text(file_ast.source.as_bytes()) {
                    if vis_text.starts_with("pub") {
                        return true;
                    }
                }
            }
            // Sometimes `pub` can be a direct keyword child without being wrapped in `visibility_modifier`
            // (though less common for items that `visibility_modifier` applies to).
            // This check might be redundant if tree-sitter-rust always uses `visibility_modifier`.
            if child.kind() == "pub" { // Check if there's a node kind named "pub"
                 if let Ok(text) = child.utf8_text(file_ast.source.as_bytes()) {
                    if text == "pub" {
                        return true;
                    }
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    // If the node itself is a `visibility_modifier` starting with `pub` (e.g. in some macro outputs or unusual ASTs)
    if node.kind() == "visibility_modifier" {
        if let Ok(vis_text) = node.utf8_text(file_ast.source.as_bytes()) {
            if vis_text.starts_with("pub") {
                return true;
            }
        }
    }

    // Default to not public if no explicit `pub` modifier found.
    // In Rust, items are private by default.
    false
}

#[allow(dead_code)]
pub fn get_rust_query_pattern_str() -> &'static str {
    r#"
    (function_item) @function
    (struct_item) @struct
    (enum_item) @enum
    (trait_item) @trait
    (impl_item) @impl_item
    (mod_item) @module
    (const_item) @const
    (static_item) @static
    (type_item) @type_alias  ; type alias
    (macro_definition) @macro_definition
    (use_declaration) @use
    ; (attribute_item) @attribute ; Attributes might be too noisy if captured this way
    "#
}
