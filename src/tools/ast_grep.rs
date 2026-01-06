//! AST-Grep Tool
//!
//! Provides structural code search using tree-sitter AST parsing.
//! Unlike text-based grep, this tool understands code structure and
//! can match patterns like function definitions, class methods, etc.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

use super::{Tool, ToolContext};

/// AST-Grep tool for structural code search
pub struct AstGrepTool;

#[async_trait]
impl Tool for AstGrepTool {
    fn name(&self) -> &str {
        "ast_grep"
    }

    fn description(&self) -> &str {
        "Search code using AST (Abstract Syntax Tree) patterns. Unlike text-based grep, this understands code structure and can find function definitions, class declarations, imports, etc. Supports Rust, TypeScript, JavaScript, Python, and Go."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "AST node type (e.g., 'function_item', 'class_definition') or a tree-sitter query pattern. Common patterns: function_item (Rust), function_definition (Python), function_declaration (JS/Go), class_definition (Python), class_declaration (JS), struct_item (Rust), impl_item (Rust)"
                },
                "language": {
                    "type": "string",
                    "enum": ["rust", "typescript", "javascript", "python", "go"],
                    "description": "Language to search. If not specified, searches all supported languages."
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search. Defaults to current directory."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Defaults to 50."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: AstGrepParams = serde_json::from_value(params)?;

        // Create a context with project_path
        let tool_ctx = ToolContextWithPath {
            project_path: ctx.working_dir.to_path_buf(),
        };

        execute(params, &tool_ctx).await
    }
}

/// Simplified context for AST grep execution
struct ToolContextWithPath {
    project_path: std::path::PathBuf,
}

/// Supported languages for AST parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AstLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
}

impl AstLanguage {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(AstLanguage::Rust),
            "ts" | "tsx" => Some(AstLanguage::TypeScript),
            "js" | "jsx" | "mjs" | "cjs" => Some(AstLanguage::JavaScript),
            "py" | "pyi" => Some(AstLanguage::Python),
            "go" => Some(AstLanguage::Go),
            _ => None,
        }
    }

    /// Get the tree-sitter language for this language
    fn get_language(&self) -> tree_sitter::Language {
        match self {
            AstLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
            AstLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            AstLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            AstLanguage::Python => tree_sitter_python::LANGUAGE.into(),
            AstLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }

    /// Get file extensions for this language
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            AstLanguage::Rust => &["rs"],
            AstLanguage::TypeScript => &["ts", "tsx"],
            AstLanguage::JavaScript => &["js", "jsx", "mjs", "cjs"],
            AstLanguage::Python => &["py", "pyi"],
            AstLanguage::Go => &["go"],
        }
    }
}

/// A match found by the AST search
#[derive(Debug, Clone, Serialize)]
pub struct AstMatch {
    /// File path where the match was found
    pub file: String,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (0-indexed)
    pub column: usize,
    /// The matched text
    pub text: String,
    /// Node type (e.g., "function_definition", "class_definition")
    pub node_type: String,
    /// Context: a few lines around the match
    pub context: String,
}

/// Parameters for ast_grep tool
#[derive(Debug, Deserialize)]
pub struct AstGrepParams {
    /// The pattern to search for. Can be:
    /// - A node type like "function_definition" or "class_definition"
    /// - A tree-sitter query like "(function_item name: (identifier) @name)"
    pub pattern: String,
    /// Language to search (rust, typescript, javascript, python, go)
    /// If not specified, will search all supported languages
    pub language: Option<AstLanguage>,
    /// Directory or file to search (defaults to current directory)
    pub path: Option<String>,
    /// Maximum number of results to return
    pub max_results: Option<usize>,
}

/// Common AST patterns for quick searching
pub mod patterns {
    /// Match all function definitions
    pub const FUNCTIONS: &str = "(function_item) @match";
    pub const FUNCTIONS_PYTHON: &str = "(function_definition) @match";
    pub const FUNCTIONS_JS: &str = "(function_declaration) @match";
    pub const FUNCTIONS_GO: &str = "(function_declaration) @match";

    /// Match all struct/class definitions
    pub const STRUCTS_RUST: &str = "(struct_item) @match";
    pub const CLASSES_PYTHON: &str = "(class_definition) @match";
    pub const CLASSES_JS: &str = "(class_declaration) @match";
    pub const STRUCTS_GO: &str = "(type_declaration) @match";

    /// Match all impl blocks (Rust)
    pub const IMPL_BLOCKS: &str = "(impl_item) @match";

    /// Match all imports
    pub const IMPORTS_RUST: &str = "(use_declaration) @match";
    pub const IMPORTS_PYTHON: &str = "(import_statement) @match";
    pub const IMPORTS_JS: &str = "(import_statement) @match";
    pub const IMPORTS_GO: &str = "(import_declaration) @match";
}

/// Execute an AST search
pub async fn execute(params: AstGrepParams, ctx: &ToolContextWithPath) -> Result<String> {
    let search_path = params
        .path
        .map(|p| ctx.project_path.join(p))
        .unwrap_or_else(|| ctx.project_path.clone());

    let max_results = params.max_results.unwrap_or(50);
    let mut all_matches = Vec::new();

    // Determine which languages to search
    let languages: Vec<AstLanguage> = if let Some(lang) = params.language {
        vec![lang]
    } else {
        vec![
            AstLanguage::Rust,
            AstLanguage::TypeScript,
            AstLanguage::JavaScript,
            AstLanguage::Python,
            AstLanguage::Go,
        ]
    };

    // Collect files to search
    let files = collect_files(&search_path, &languages)?;

    for file_path in files {
        if all_matches.len() >= max_results {
            break;
        }

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = match AstLanguage::from_extension(ext) {
            Some(l) => l,
            None => continue,
        };

        // Search this file
        match search_file(&file_path, &params.pattern, lang).await {
            Ok(matches) => {
                for m in matches {
                    if all_matches.len() >= max_results {
                        break;
                    }
                    all_matches.push(m);
                }
            }
            Err(e) => {
                tracing::debug!("Error searching {:?}: {}", file_path, e);
            }
        }
    }

    // Format results
    if all_matches.is_empty() {
        Ok(format!(
            "No matches found for pattern '{}' in {:?}",
            params.pattern, search_path
        ))
    } else {
        let mut output = format!(
            "Found {} matches for pattern '{}':\n\n",
            all_matches.len(),
            params.pattern
        );

        for m in &all_matches {
            output.push_str(&format!(
                "{}:{}:{} [{}]\n",
                m.file, m.line, m.column, m.node_type
            ));
            // Add context with line numbers
            for (i, line) in m.context.lines().enumerate() {
                let line_num = m.line.saturating_sub(1) + i;
                let marker = if i == 1 { ">" } else { " " };
                output.push_str(&format!("{} {:4} | {}\n", marker, line_num, line));
            }
            output.push('\n');
        }

        if all_matches.len() >= max_results {
            output.push_str(&format!(
                "\n(Results limited to {} matches. Use max_results to see more.)\n",
                max_results
            ));
        }

        Ok(output)
    }
}

/// Collect files matching the given languages
fn collect_files(path: &Path, languages: &[AstLanguage]) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.to_path_buf());
        return Ok(files);
    }

    let extensions: Vec<&str> = languages
        .iter()
        .flat_map(|l| l.extensions().iter().copied())
        .collect();

    // Use walkdir to traverse, respecting .gitignore
    let walker = ignore::WalkBuilder::new(path)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        let entry_path = entry.path();
        if entry_path.is_file() {
            if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(entry_path.to_path_buf());
                }
            }
        }
    }

    Ok(files)
}

/// Search a single file for AST matches
pub async fn search_file(
    file_path: &Path,
    pattern: &str,
    language: AstLanguage,
) -> Result<Vec<AstMatch>> {
    let source = tokio::fs::read_to_string(file_path)
        .await
        .context("Failed to read file")?;

    let ts_language = language.get_language();
    let mut parser = Parser::new();
    parser
        .set_language(&ts_language)
        .context("Failed to set language")?;

    let tree = parser
        .parse(&source, None)
        .context("Failed to parse file")?;

    let mut matches = Vec::new();

    // Determine if pattern is a simple node type or a query
    let query_str = if pattern.starts_with('(') || pattern.contains('@') {
        // It's already a query
        pattern.to_string()
    } else {
        // Treat as a simple node type to find
        format!("({}) @match", pattern)
    };

    // Try to create and run the query
    match Query::new(&ts_language, &query_str) {
        Ok(query) => {
            let mut cursor = QueryCursor::new();

            // In tree-sitter 0.24, we need to use the iterator properly
            let mut query_matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

            while let Some(query_match) = query_matches.next() {
                for capture in query_match.captures {
                    let node = capture.node;
                    let start = node.start_position();
                    let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();

                    // Get context (3 lines around the match)
                    let lines: Vec<&str> = source.lines().collect();
                    let start_line = start.row.saturating_sub(1);
                    let end_line = (start.row + 2).min(lines.len());
                    let context = lines[start_line..end_line].join("\n");

                    matches.push(AstMatch {
                        file: file_path.display().to_string(),
                        line: start.row + 1,
                        column: start.column,
                        text: truncate_text(&text, 100),
                        node_type: node.kind().to_string(),
                        context,
                    });
                }
            }
        }
        Err(e) => {
            // If query parsing fails, fall back to simple node type matching
            tracing::debug!("Query parse failed: {}, trying simple node match", e);
            find_nodes_by_type(&tree.root_node(), pattern, &source, file_path, &mut matches);
        }
    }

    Ok(matches)
}

/// Recursively find nodes by type
fn find_nodes_by_type(
    node: &tree_sitter::Node,
    node_type: &str,
    source: &str,
    file_path: &Path,
    matches: &mut Vec<AstMatch>,
) {
    if node.kind() == node_type {
        let start = node.start_position();
        let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();

        let lines: Vec<&str> = source.lines().collect();
        let start_line = start.row.saturating_sub(1);
        let end_line = (start.row + 2).min(lines.len());
        let context = lines[start_line..end_line].join("\n");

        matches.push(AstMatch {
            file: file_path.display().to_string(),
            line: start.row + 1,
            column: start.column,
            text: truncate_text(&text, 100),
            node_type: node.kind().to_string(),
            context,
        });
    }

    // Recursively check children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_nodes_by_type(&child, node_type, source, file_path, matches);
    }
}

/// Truncate text to max length, adding ellipsis if needed
fn truncate_text(text: &str, max_len: usize) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    if first_line.len() > max_len {
        format!("{}...", &first_line[..max_len])
    } else {
        first_line.to_string()
    }
}

/// Get the JSON schema for the ast_grep tool
pub fn get_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "ast_grep",
        "description": "Search code using AST (Abstract Syntax Tree) patterns. Unlike text-based grep, this understands code structure and can find function definitions, class declarations, imports, etc. Supports Rust, TypeScript, JavaScript, Python, and Go.\n\nExamples:\n- pattern: 'function_item' - Find all Rust functions\n- pattern: 'class_definition' - Find all Python classes\n- pattern: '(function_item name: (identifier) @name)' - Tree-sitter query for function names",
        "input_schema": {
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "AST node type (e.g., 'function_item', 'class_definition') or a tree-sitter query pattern"
                },
                "language": {
                    "type": "string",
                    "enum": ["rust", "typescript", "javascript", "python", "go"],
                    "description": "Language to search. If not specified, searches all supported languages."
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search. Defaults to current directory."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Defaults to 50."
                }
            },
            "required": ["pattern"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_find_rust_functions() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            r#"
fn hello() {
    println!("Hello");
}

fn world(x: i32) -> i32 {
    x + 1
}
"#,
        )
        .unwrap();

        let matches = search_file(&file_path, "function_item", AstLanguage::Rust)
            .await
            .unwrap();
        assert_eq!(matches.len(), 2);
        assert!(matches[0].text.contains("fn hello"));
        assert!(matches[1].text.contains("fn world"));
    }

    #[tokio::test]
    async fn test_find_python_classes() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        std::fs::write(
            &file_path,
            r#"
class MyClass:
    def __init__(self):
        pass

class AnotherClass:
    pass
"#,
        )
        .unwrap();

        let matches = search_file(&file_path, "class_definition", AstLanguage::Python)
            .await
            .unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(AstLanguage::from_extension("rs"), Some(AstLanguage::Rust));
        assert_eq!(
            AstLanguage::from_extension("ts"),
            Some(AstLanguage::TypeScript)
        );
        assert_eq!(AstLanguage::from_extension("py"), Some(AstLanguage::Python));
        assert_eq!(AstLanguage::from_extension("go"), Some(AstLanguage::Go));
        assert_eq!(AstLanguage::from_extension("txt"), None);
    }
}
