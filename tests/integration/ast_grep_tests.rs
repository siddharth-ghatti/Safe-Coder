//! Integration tests for the AST-Grep tool
//!
//! Tests the structural code search functionality using tree-sitter AST parsing.

use anyhow::Result;
use serial_test::serial;
use safe_coder::tools::{AstLanguage, AstGrepParams, AstMatch, search_file, patterns};
use assert_fs::prelude::*;
use assert_fs::TempDir;

#[test]
fn test_language_detection_rust() {
    assert_eq!(AstLanguage::from_extension("rs"), Some(AstLanguage::Rust));
}

#[test]
fn test_language_detection_typescript() {
    assert_eq!(AstLanguage::from_extension("ts"), Some(AstLanguage::TypeScript));
    assert_eq!(AstLanguage::from_extension("tsx"), Some(AstLanguage::TypeScript));
}

#[test]
fn test_language_detection_javascript() {
    assert_eq!(AstLanguage::from_extension("js"), Some(AstLanguage::JavaScript));
    assert_eq!(AstLanguage::from_extension("jsx"), Some(AstLanguage::JavaScript));
    assert_eq!(AstLanguage::from_extension("mjs"), Some(AstLanguage::JavaScript));
    assert_eq!(AstLanguage::from_extension("cjs"), Some(AstLanguage::JavaScript));
}

#[test]
fn test_language_detection_python() {
    assert_eq!(AstLanguage::from_extension("py"), Some(AstLanguage::Python));
    assert_eq!(AstLanguage::from_extension("pyi"), Some(AstLanguage::Python));
}

#[test]
fn test_language_detection_go() {
    assert_eq!(AstLanguage::from_extension("go"), Some(AstLanguage::Go));
}

#[test]
fn test_language_detection_unknown() {
    assert_eq!(AstLanguage::from_extension("txt"), None);
    assert_eq!(AstLanguage::from_extension("md"), None);
    assert_eq!(AstLanguage::from_extension("json"), None);
    assert_eq!(AstLanguage::from_extension("yaml"), None);
}

#[test]
fn test_language_detection_case_insensitive() {
    assert_eq!(AstLanguage::from_extension("RS"), Some(AstLanguage::Rust));
    assert_eq!(AstLanguage::from_extension("Py"), Some(AstLanguage::Python));
    assert_eq!(AstLanguage::from_extension("TS"), Some(AstLanguage::TypeScript));
}

#[test]
fn test_language_extensions_rust() {
    let exts = AstLanguage::Rust.extensions();
    assert!(exts.contains(&"rs"));
}

#[test]
fn test_language_extensions_typescript() {
    let exts = AstLanguage::TypeScript.extensions();
    assert!(exts.contains(&"ts"));
    assert!(exts.contains(&"tsx"));
}

#[test]
fn test_language_extensions_javascript() {
    let exts = AstLanguage::JavaScript.extensions();
    assert!(exts.contains(&"js"));
    assert!(exts.contains(&"jsx"));
    assert!(exts.contains(&"mjs"));
    assert!(exts.contains(&"cjs"));
}

#[test]
fn test_language_extensions_python() {
    let exts = AstLanguage::Python.extensions();
    assert!(exts.contains(&"py"));
    assert!(exts.contains(&"pyi"));
}

#[test]
fn test_language_extensions_go() {
    let exts = AstLanguage::Go.extensions();
    assert!(exts.contains(&"go"));
}

#[test]
fn test_ast_grep_params_deserialization() -> Result<()> {
    let json = serde_json::json!({
        "pattern": "function_item",
        "language": "rust",
        "path": "src/",
        "max_results": 10
    });

    let params: AstGrepParams = serde_json::from_value(json)?;

    assert_eq!(params.pattern, "function_item");
    assert_eq!(params.language, Some(AstLanguage::Rust));
    assert_eq!(params.path, Some("src/".to_string()));
    assert_eq!(params.max_results, Some(10));

    Ok(())
}

#[test]
fn test_ast_grep_params_minimal() -> Result<()> {
    let json = serde_json::json!({
        "pattern": "class_definition"
    });

    let params: AstGrepParams = serde_json::from_value(json)?;

    assert_eq!(params.pattern, "class_definition");
    assert!(params.language.is_none());
    assert!(params.path.is_none());
    assert!(params.max_results.is_none());

    Ok(())
}

#[test]
fn test_ast_match_structure() -> Result<()> {
    let match_result = AstMatch {
        file: "src/main.rs".to_string(),
        line: 10,
        column: 0,
        text: "fn main()".to_string(),
        node_type: "function_item".to_string(),
        context: "fn main() {\n    println!(\"Hello\");\n}".to_string(),
    };

    assert_eq!(match_result.file, "src/main.rs");
    assert_eq!(match_result.line, 10);
    assert_eq!(match_result.column, 0);
    assert_eq!(match_result.node_type, "function_item");
    assert!(match_result.context.contains("fn main"));

    Ok(())
}

#[test]
fn test_get_schema() -> Result<()> {
    let schema = safe_coder::tools::ast_grep::get_schema();

    // Verify schema structure
    assert_eq!(schema["name"], "ast_grep");
    assert!(schema["description"].as_str().unwrap().contains("AST"));

    let input_schema = &schema["input_schema"];
    assert_eq!(input_schema["type"], "object");

    let properties = &input_schema["properties"];
    assert!(properties["pattern"].is_object());
    assert!(properties["language"].is_object());
    assert!(properties["path"].is_object());
    assert!(properties["max_results"].is_object());

    // Verify required fields
    let required = input_schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("pattern")));

    Ok(())
}

#[test]
fn test_patterns_constants() {

    // Verify pattern constants exist and are valid tree-sitter patterns
    assert!(patterns::FUNCTIONS.contains("function_item"));
    assert!(patterns::FUNCTIONS_PYTHON.contains("function_definition"));
    assert!(patterns::FUNCTIONS_JS.contains("function_declaration"));
    assert!(patterns::FUNCTIONS_GO.contains("function_declaration"));

    assert!(patterns::STRUCTS_RUST.contains("struct_item"));
    assert!(patterns::CLASSES_PYTHON.contains("class_definition"));
    assert!(patterns::CLASSES_JS.contains("class_declaration"));
    assert!(patterns::STRUCTS_GO.contains("type_declaration"));

    assert!(patterns::IMPL_BLOCKS.contains("impl_item"));

    assert!(patterns::IMPORTS_RUST.contains("use_declaration"));
    assert!(patterns::IMPORTS_PYTHON.contains("import_statement"));
    assert!(patterns::IMPORTS_JS.contains("import_statement"));
    assert!(patterns::IMPORTS_GO.contains("import_declaration"));
}

// The following tests require the search_file function which is async
// and uses the tool context. We test them through the module's unit tests
// which are already defined in ast_grep.rs

#[tokio::test]
#[serial]
async fn test_ast_grep_tool_basic_properties() -> Result<()> {
    use safe_coder::tools::Tool;
    use safe_coder::tools::AstGrepTool;

    let tool = AstGrepTool;

    assert_eq!(tool.name(), "ast_grep");
    assert!(tool.description().contains("AST"));
    // Description mentions structural/code search functionality
    assert!(tool.description().contains("code") || tool.description().contains("structure"));

    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["pattern"].is_object());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_rust_functions() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.rs");

    std::fs::write(&file_path, r#"
fn hello() {
    println!("Hello");
}

fn world(x: i32) -> i32 {
    x + 1
}

pub fn public_fn() -> String {
    "public".to_string()
}
"#)?;

    let matches = search_file(&file_path, "function_item", AstLanguage::Rust).await?;

    assert_eq!(matches.len(), 3);
    assert!(matches.iter().any(|m| m.text.contains("fn hello")));
    assert!(matches.iter().any(|m| m.text.contains("fn world")));
    assert!(matches.iter().any(|m| m.text.contains("pub fn public_fn")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_python_classes() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.py");

    std::fs::write(&file_path, r#"
class MyClass:
    def __init__(self):
        self.value = 0

    def method(self):
        pass

class AnotherClass:
    pass
"#)?;

    let matches = search_file(&file_path, "class_definition", AstLanguage::Python).await?;

    assert_eq!(matches.len(), 2);
    assert!(matches.iter().any(|m| m.text.contains("class MyClass")));
    assert!(matches.iter().any(|m| m.text.contains("class AnotherClass")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_python_functions() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.py");

    std::fs::write(&file_path, r#"
def hello():
    print("Hello")

def world(x):
    return x + 1

def greet(name):
    return f"Hello, {name}"
"#)?;

    let matches = search_file(&file_path, "function_definition", AstLanguage::Python).await?;

    assert_eq!(matches.len(), 3);
    assert!(matches.iter().any(|m| m.text.contains("def hello")));
    assert!(matches.iter().any(|m| m.text.contains("def world")));
    assert!(matches.iter().any(|m| m.text.contains("def greet")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_javascript_functions() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.js");

    std::fs::write(&file_path, r#"
function hello() {
    console.log("Hello");
}

function world(x) {
    return x + 1;
}
"#)?;

    let matches = search_file(&file_path, "function_declaration", AstLanguage::JavaScript).await?;

    assert_eq!(matches.len(), 2);
    assert!(matches.iter().any(|m| m.text.contains("function hello")));
    assert!(matches.iter().any(|m| m.text.contains("function world")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_typescript_interfaces() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.ts");

    std::fs::write(&file_path, r#"
interface User {
    name: string;
    age: number;
}

interface Product {
    id: number;
    title: string;
}

function getUser(): User {
    return { name: "John", age: 30 };
}
"#)?;

    let matches = search_file(&file_path, "interface_declaration", AstLanguage::TypeScript).await?;

    assert_eq!(matches.len(), 2);
    assert!(matches.iter().any(|m| m.text.contains("interface User")));
    assert!(matches.iter().any(|m| m.text.contains("interface Product")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_rust_structs() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.rs");

    std::fs::write(&file_path, r#"
struct Point {
    x: i32,
    y: i32,
}

pub struct User {
    name: String,
    age: u32,
}

#[derive(Debug)]
struct Config {
    enabled: bool,
}
"#)?;

    let matches = search_file(&file_path, "struct_item", AstLanguage::Rust).await?;

    assert_eq!(matches.len(), 3);
    assert!(matches.iter().any(|m| m.text.contains("struct Point")));
    assert!(matches.iter().any(|m| m.text.contains("pub struct User")));
    assert!(matches.iter().any(|m| m.text.contains("struct Config")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_rust_impl_blocks() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.rs");

    std::fs::write(&file_path, r#"
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl Default for Point {
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}
"#)?;

    let matches = search_file(&file_path, "impl_item", AstLanguage::Rust).await?;

    assert_eq!(matches.len(), 2);
    assert!(matches.iter().any(|m| m.text.contains("impl Point")));
    assert!(matches.iter().any(|m| m.text.contains("impl Default for Point")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_with_query_pattern() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.rs");

    std::fs::write(&file_path, r#"
fn hello() {}
fn world() {}
"#)?;

    // Use a full tree-sitter query
    let matches = search_file(&file_path, "(function_item) @match", AstLanguage::Rust).await?;

    assert_eq!(matches.len(), 2);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_match_has_context() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.rs");

    std::fs::write(&file_path, r#"// Previous line
fn hello() {
    println!("Hello");
}
// After line
"#)?;

    let matches = search_file(&file_path, "function_item", AstLanguage::Rust).await?;

    assert_eq!(matches.len(), 1);
    let m = &matches[0];

    // Context should include surrounding lines
    assert!(m.context.len() > m.text.len());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_line_numbers_correct() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.rs");

    std::fs::write(&file_path, r#"// Line 1

// Line 3

fn hello() {
    println!("Hello");
}
"#)?;

    let matches = search_file(&file_path, "function_item", AstLanguage::Rust).await?;

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].line, 5); // 1-indexed, function starts at line 5

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ast_grep_find_go_functions() -> Result<()> {

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.go");

    std::fs::write(&file_path, r#"package main

func hello() {
    fmt.Println("Hello")
}

func world(x int) int {
    return x + 1
}
"#)?;

    let matches = search_file(&file_path, "function_declaration", AstLanguage::Go).await?;

    assert_eq!(matches.len(), 2);
    assert!(matches.iter().any(|m| m.text.contains("func hello")));
    assert!(matches.iter().any(|m| m.text.contains("func world")));

    Ok(())
}
