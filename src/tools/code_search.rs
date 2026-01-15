//! Code Search Tool
//!
//! A comprehensive search tool designed for efficient codebase exploration.
//! Unlike simple grep or AST search, this tool:
//! - Supports multiple patterns in one query
//! - Extracts symbol definitions and usages
//! - Provides structural summaries of files
//! - Reduces the need to read entire files during exploration

use anyhow::{Context, Result};
use async_trait::async_trait;
use ignore::WalkBuilder;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

use super::ast_grep::AstLanguage;
use super::{Tool, ToolContext};

/// Code Search Tool for comprehensive codebase exploration
pub struct CodeSearchTool;

#[async_trait]
impl Tool for CodeSearchTool {
    fn name(&self) -> &str {
        "code_search"
    }

    fn description(&self) -> &str {
        r#"Advanced code search tool for efficient codebase exploration. Use this instead of multiple grep/read calls.

Capabilities:
1. Multi-pattern search: Search for multiple patterns at once (e.g., ["fn main", "struct Config", "impl Error"])
2. Symbol extraction: Find all definitions of functions, structs, classes, etc.
3. File structure: Get an overview of all symbols in specific files without reading the entire file
4. Usage search: Find where specific symbols are used/imported

This tool is designed to minimize the number of file reads needed during exploration."#
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["patterns", "definitions", "structure", "usages"],
                    "description": "Search mode:\n- patterns: Search for multiple regex patterns at once\n- definitions: Find all definitions of a symbol name\n- structure: Get symbol overview of files\n- usages: Find where a symbol is used/imported"
                },
                "patterns": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "For 'patterns' mode: Array of regex patterns to search for. Each pattern is searched independently and results are grouped."
                },
                "symbol": {
                    "type": "string",
                    "description": "For 'definitions' and 'usages' modes: The symbol name to search for"
                },
                "files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "For 'structure' mode: Array of file paths to analyze"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to working directory."
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., \"*.rs\", \"*.{ts,tsx}\")"
                },
                "max_results_per_pattern": {
                    "type": "integer",
                    "description": "Maximum results per pattern. Defaults to 20."
                }
            },
            "required": ["mode"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: CodeSearchParams = serde_json::from_value(params)?;
        execute_code_search(params, ctx.working_dir).await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SearchMode {
    Patterns,
    Definitions,
    Structure,
    Usages,
}

#[derive(Debug, Deserialize)]
struct CodeSearchParams {
    mode: SearchMode,
    /// Multiple patterns to search for (patterns mode)
    patterns: Option<Vec<String>>,
    /// Symbol name to find (definitions/usages mode)
    symbol: Option<String>,
    /// Files to analyze (structure mode)
    files: Option<Vec<String>>,
    /// Directory to search in
    path: Option<String>,
    /// File filter glob
    include: Option<String>,
    /// Max results per pattern
    #[serde(default = "default_max_results")]
    max_results_per_pattern: usize,
}

fn default_max_results() -> usize {
    20
}

async fn execute_code_search(params: CodeSearchParams, working_dir: &Path) -> Result<String> {
    let search_path = params
        .path
        .map(|p| {
            if p.starts_with('/') {
                PathBuf::from(p)
            } else {
                working_dir.join(p)
            }
        })
        .unwrap_or_else(|| working_dir.to_path_buf());

    match params.mode {
        SearchMode::Patterns => {
            let patterns = params.patterns.unwrap_or_default();
            if patterns.is_empty() {
                return Ok("Error: 'patterns' mode requires the 'patterns' array parameter".to_string());
            }
            multi_pattern_search(&search_path, &patterns, params.include.as_deref(), params.max_results_per_pattern).await
        }
        SearchMode::Definitions => {
            let symbol = params.symbol.ok_or_else(|| anyhow::anyhow!("'definitions' mode requires the 'symbol' parameter"))?;
            find_definitions(&search_path, &symbol, params.include.as_deref(), params.max_results_per_pattern).await
        }
        SearchMode::Structure => {
            let files = params.files.unwrap_or_default();
            if files.is_empty() {
                return Ok("Error: 'structure' mode requires the 'files' array parameter".to_string());
            }
            analyze_file_structure(&search_path, &files).await
        }
        SearchMode::Usages => {
            let symbol = params.symbol.ok_or_else(|| anyhow::anyhow!("'usages' mode requires the 'symbol' parameter"))?;
            find_usages(&search_path, &symbol, params.include.as_deref(), params.max_results_per_pattern).await
        }
    }
}

/// Search for multiple patterns at once and return grouped results
async fn multi_pattern_search(
    search_path: &Path,
    patterns: &[String],
    include: Option<&str>,
    max_per_pattern: usize,
) -> Result<String> {
    use grep::regex::RegexMatcher;
    use grep::searcher::sinks::UTF8;
    use grep::searcher::Searcher;
    use std::sync::{Arc, Mutex};

    let mut results: HashMap<String, Vec<String>> = HashMap::new();

    // Collect all files first
    let files = collect_search_files(search_path, include)?;

    for pattern in patterns {
        let matcher = match RegexMatcher::new(pattern) {
            Ok(m) => m,
            Err(e) => {
                results.insert(pattern.clone(), vec![format!("Invalid pattern: {}", e)]);
                continue;
            }
        };

        let pattern_results: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        for file_path in &files {
            {
                let guard = pattern_results.lock().unwrap();
                if guard.len() >= max_per_pattern {
                    break;
                }
            }

            let results_clone = Arc::clone(&pattern_results);
            let working_dir = search_path.to_path_buf();
            let path_buf = file_path.clone();

            let mut searcher = Searcher::new();
            let _ = searcher.search_path(
                &matcher,
                &path_buf,
                UTF8(|line_num, line| {
                    let mut guard = results_clone.lock().unwrap();
                    if guard.len() < max_per_pattern {
                        let relative = path_buf
                            .strip_prefix(&working_dir)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| path_buf.to_string_lossy().to_string());
                        guard.push(format!("{}:{}: {}", relative, line_num, line.trim()));
                    }
                    Ok(guard.len() < max_per_pattern)
                }),
            );
        }

        let matches = pattern_results.lock().unwrap().clone();
        results.insert(pattern.clone(), matches);
    }

    // Format output
    let mut output = String::new();
    output.push_str(&format!("# Multi-Pattern Search Results\n\n"));
    output.push_str(&format!("Searched {} files in {:?}\n\n", files.len(), search_path));

    for pattern in patterns {
        let matches = results.get(pattern).unwrap();
        output.push_str(&format!("## Pattern: `{}`\n", pattern));
        if matches.is_empty() {
            output.push_str("No matches found\n\n");
        } else {
            output.push_str(&format!("Found {} matches:\n```\n", matches.len()));
            for m in matches {
                output.push_str(m);
                output.push('\n');
            }
            output.push_str("```\n\n");
        }
    }

    Ok(output)
}

/// Find all definitions of a symbol (functions, structs, classes, etc.)
async fn find_definitions(
    search_path: &Path,
    symbol: &str,
    include: Option<&str>,
    max_results: usize,
) -> Result<String> {
    let files = collect_search_files(search_path, include)?;
    let mut definitions = Vec::new();

    for file_path in files {
        if definitions.len() >= max_results {
            break;
        }

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = match AstLanguage::from_extension(ext) {
            Some(l) => l,
            None => continue,
        };

        if let Ok(defs) = find_symbol_definitions(&file_path, symbol, lang).await {
            for def in defs {
                if definitions.len() >= max_results {
                    break;
                }
                definitions.push(def);
            }
        }
    }

    if definitions.is_empty() {
        return Ok(format!("No definitions found for symbol '{}'", symbol));
    }

    let mut output = format!("# Definitions of '{}'\n\n", symbol);
    output.push_str(&format!("Found {} definitions:\n\n", definitions.len()));

    for def in &definitions {
        output.push_str(&format!("## {}:{}\n", def.file, def.line));
        output.push_str(&format!("**Type:** {}\n", def.kind));
        output.push_str(&format!("```\n{}\n```\n\n", def.signature));
    }

    Ok(output)
}

/// Analyze the structure of given files without reading entire contents
async fn analyze_file_structure(base_path: &Path, files: &[String]) -> Result<String> {
    let mut output = String::new();
    output.push_str("# File Structure Analysis\n\n");

    for file in files {
        let file_path = if file.starts_with('/') {
            PathBuf::from(file)
        } else {
            base_path.join(file)
        };

        if !file_path.exists() {
            output.push_str(&format!("## {}\nFile not found\n\n", file));
            continue;
        }

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = match AstLanguage::from_extension(ext) {
            Some(l) => l,
            None => {
                output.push_str(&format!("## {}\nUnsupported language\n\n", file));
                continue;
            }
        };

        match extract_file_symbols(&file_path, lang).await {
            Ok(symbols) => {
                output.push_str(&format!("## {}\n", file));

                // Group by kind
                let mut by_kind: HashMap<&str, Vec<&SymbolInfo>> = HashMap::new();
                for sym in &symbols {
                    by_kind.entry(&sym.kind).or_default().push(sym);
                }

                for (kind, syms) in by_kind {
                    output.push_str(&format!("\n### {} ({})\n", kind, syms.len()));
                    for sym in syms {
                        output.push_str(&format!("- `{}` (line {})\n", sym.name, sym.line));
                    }
                }
                output.push('\n');
            }
            Err(e) => {
                output.push_str(&format!("## {}\nError: {}\n\n", file, e));
            }
        }
    }

    Ok(output)
}

/// Find usages/references of a symbol
async fn find_usages(
    search_path: &Path,
    symbol: &str,
    include: Option<&str>,
    max_results: usize,
) -> Result<String> {
    use grep::regex::RegexMatcher;
    use grep::searcher::sinks::UTF8;
    use grep::searcher::Searcher;
    use std::sync::{Arc, Mutex};

    let files = collect_search_files(search_path, include)?;

    // Search for the symbol as a word boundary match
    let pattern = format!(r"\b{}\b", regex::escape(symbol));
    let matcher = RegexMatcher::new(&pattern)?;

    let results: Arc<Mutex<Vec<UsageInfo>>> = Arc::new(Mutex::new(Vec::new()));

    for file_path in files {
        {
            let guard = results.lock().unwrap();
            if guard.len() >= max_results {
                break;
            }
        }

        let results_clone = Arc::clone(&results);
        let working_dir = search_path.to_path_buf();
        let path_buf = file_path.clone();

        let mut searcher = Searcher::new();
        let _ = searcher.search_path(
            &matcher,
            &path_buf,
            UTF8(|line_num, line| {
                let mut guard = results_clone.lock().unwrap();
                if guard.len() < max_results {
                    let relative = path_buf
                        .strip_prefix(&working_dir)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path_buf.to_string_lossy().to_string());

                    // Categorize the usage
                    let category = categorize_usage(line, symbol);

                    guard.push(UsageInfo {
                        file: relative,
                        line: line_num as usize,
                        text: line.trim().to_string(),
                        category,
                    });
                }
                Ok(guard.len() < max_results)
            }),
        );
    }

    let usages = results.lock().unwrap().clone();

    if usages.is_empty() {
        return Ok(format!("No usages found for symbol '{}'", symbol));
    }

    // Group by category
    let mut by_category: HashMap<&str, Vec<&UsageInfo>> = HashMap::new();
    for usage in &usages {
        by_category.entry(&usage.category).or_default().push(usage);
    }

    let mut output = format!("# Usages of '{}'\n\n", symbol);
    output.push_str(&format!("Found {} usages:\n\n", usages.len()));

    for (category, cat_usages) in by_category {
        output.push_str(&format!("## {} ({})\n", category, cat_usages.len()));
        for usage in cat_usages {
            output.push_str(&format!("- `{}:{}`: `{}`\n", usage.file, usage.line, truncate(&usage.text, 80)));
        }
        output.push('\n');
    }

    Ok(output)
}

// Helper structs

#[derive(Debug, Clone)]
struct DefinitionInfo {
    file: String,
    line: usize,
    kind: String,
    name: String,
    signature: String,
}

#[derive(Debug, Clone)]
struct SymbolInfo {
    name: String,
    kind: String,
    line: usize,
}

#[derive(Debug, Clone)]
struct UsageInfo {
    file: String,
    line: usize,
    text: String,
    category: String,
}

// Helper functions

fn collect_search_files(path: &Path, include: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.to_path_buf());
        return Ok(files);
    }

    let mut walker_builder = WalkBuilder::new(path);
    walker_builder
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);

    if let Some(include_pattern) = include {
        let mut types_builder = ignore::types::TypesBuilder::new();
        types_builder.add("custom", include_pattern)?;
        types_builder.select("custom");
        walker_builder.types(types_builder.build()?);
    }

    for entry in walker_builder.build().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    Ok(files)
}

async fn find_symbol_definitions(
    file_path: &Path,
    symbol: &str,
    language: AstLanguage,
) -> Result<Vec<DefinitionInfo>> {
    let source = tokio::fs::read_to_string(file_path)
        .await
        .context("Failed to read file")?;

    let ts_language = language.get_language();
    let mut parser = Parser::new();
    parser.set_language(&ts_language)?;

    let tree = parser.parse(&source, None).context("Failed to parse")?;
    let mut definitions = Vec::new();

    // Get queries for definition patterns based on language
    let queries = get_definition_queries(language);

    for query_str in queries {
        if let Ok(query) = Query::new(&ts_language, query_str) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

            while let Some(m) = matches.next() {
                for capture in m.captures {
                    let node = capture.node;
                    let text = node.utf8_text(source.as_bytes()).unwrap_or("");

                    // Check if this definition matches the symbol name
                    if text.contains(symbol) || get_definition_name(&node, &source).as_deref() == Some(symbol) {
                        let start = node.start_position();

                        // Get the full signature (first few lines)
                        let signature = get_signature(&node, &source);

                        definitions.push(DefinitionInfo {
                            file: file_path.display().to_string(),
                            line: start.row + 1,
                            kind: node.kind().to_string(),
                            name: get_definition_name(&node, &source).unwrap_or_else(|| text.to_string()),
                            signature,
                        });
                    }
                }
            }
        }
    }

    Ok(definitions)
}

async fn extract_file_symbols(file_path: &Path, language: AstLanguage) -> Result<Vec<SymbolInfo>> {
    let source = tokio::fs::read_to_string(file_path)
        .await
        .context("Failed to read file")?;

    let ts_language = language.get_language();
    let mut parser = Parser::new();
    parser.set_language(&ts_language)?;

    let tree = parser.parse(&source, None).context("Failed to parse")?;
    let mut symbols = Vec::new();

    // Extract all significant symbols
    extract_symbols_recursive(&tree.root_node(), &source, &mut symbols, 0);

    Ok(symbols)
}

fn extract_symbols_recursive(
    node: &tree_sitter::Node,
    source: &str,
    symbols: &mut Vec<SymbolInfo>,
    depth: usize,
) {
    // Only go a few levels deep
    if depth > 3 {
        return;
    }

    // Check if this node is a significant definition
    let kind = node.kind();
    let is_definition = matches!(
        kind,
        "function_item" | "function_definition" | "function_declaration" |
        "struct_item" | "class_definition" | "class_declaration" |
        "impl_item" | "trait_item" | "interface_declaration" |
        "enum_item" | "type_alias" | "const_item" | "static_item" |
        "method_definition" | "field_definition"
    );

    if is_definition {
        if let Some(name) = get_definition_name(node, source) {
            symbols.push(SymbolInfo {
                name,
                kind: kind.to_string(),
                line: node.start_position().row + 1,
            });
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_symbols_recursive(&child, source, symbols, depth + 1);
    }
}

fn get_definition_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // Try to find a name child node
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "name" || child.kind() == "type_identifier" {
            return child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
        }
    }

    // For some languages, the name might be nested
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(name) = get_definition_name(&child, source) {
            return Some(name);
        }
    }

    None
}

fn get_signature(node: &tree_sitter::Node, source: &str) -> String {
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");
    let lines: Vec<&str> = text.lines().take(5).collect();
    lines.join("\n")
}

fn get_definition_queries(language: AstLanguage) -> Vec<&'static str> {
    match language {
        AstLanguage::Rust => vec![
            "(function_item) @def",
            "(struct_item) @def",
            "(enum_item) @def",
            "(impl_item) @def",
            "(trait_item) @def",
            "(const_item) @def",
            "(type_item) @def",
        ],
        AstLanguage::TypeScript | AstLanguage::JavaScript => vec![
            "(function_declaration) @def",
            "(class_declaration) @def",
            "(interface_declaration) @def",
            "(type_alias_declaration) @def",
            "(variable_declaration) @def",
        ],
        AstLanguage::Python => vec![
            "(function_definition) @def",
            "(class_definition) @def",
        ],
        AstLanguage::Go => vec![
            "(function_declaration) @def",
            "(method_declaration) @def",
            "(type_declaration) @def",
        ],
    }
}

fn categorize_usage(line: &str, _symbol: &str) -> String {
    let line_lower = line.to_lowercase();

    if line_lower.contains("import") || line_lower.contains("use ") || line_lower.contains("from ") {
        "Import".to_string()
    } else if line_lower.contains("fn ") || line_lower.contains("def ") || line_lower.contains("func ") || line_lower.contains("function ") {
        "Definition".to_string()
    } else if line.contains("::") || line.contains(".") {
        "Method/Field Access".to_string()
    } else if line.contains("(") {
        "Function Call".to_string()
    } else if line.contains(":") && (line.contains("let") || line.contains("const") || line.contains("var")) {
        "Type Annotation".to_string()
    } else {
        "Reference".to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_multi_pattern_search() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        std::fs::write(
            &file_path,
            r#"
fn hello() {
    println!("Hello");
}

struct Config {
    name: String,
}

impl Config {
    fn new() -> Self {
        Config { name: String::new() }
    }
}
"#,
        )
        .unwrap();

        let result = multi_pattern_search(
            temp_dir.path(),
            &["fn ".to_string(), "struct ".to_string(), "impl ".to_string()],
            Some("*.rs"),
            10,
        )
        .await
        .unwrap();

        assert!(result.contains("fn hello"));
        assert!(result.contains("struct Config"));
        assert!(result.contains("impl Config"));
    }
}
