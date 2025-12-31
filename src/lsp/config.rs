//! LSP Configuration
//!
//! Configuration for language servers, including built-in defaults
//! for common languages.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// LSP configuration for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// Whether LSP is enabled globally
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Per-language server configurations
    #[serde(default)]
    pub servers: HashMap<String, LspServerConfig>,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            enabled: true, // LSP enabled by default
            servers: HashMap::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

/// Configuration for a specific language server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// Whether this server is disabled
    #[serde(default)]
    pub disabled: bool,
    /// Command to start the server
    pub command: String,
    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// File extensions this server handles
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Root markers (files that indicate project root)
    #[serde(default)]
    pub root_markers: Vec<String>,
    /// Initialization options to pass to the server
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            extensions: Vec::new(),
            root_markers: Vec::new(),
            initialization_options: None,
        }
    }
}

/// Get default LSP configurations for common languages
pub fn default_lsp_configs() -> HashMap<String, LspServerConfig> {
    let mut configs = HashMap::new();

    // Rust - rust-analyzer
    configs.insert(
        "rust".to_string(),
        LspServerConfig {
            command: "rust-analyzer".to_string(),
            extensions: vec!["rs".to_string()],
            root_markers: vec!["Cargo.toml".to_string(), "Cargo.lock".to_string()],
            ..Default::default()
        },
    );

    // Go - gopls
    configs.insert(
        "go".to_string(),
        LspServerConfig {
            command: "gopls".to_string(),
            extensions: vec!["go".to_string(), "mod".to_string()],
            root_markers: vec!["go.mod".to_string(), "go.sum".to_string()],
            ..Default::default()
        },
    );

    // TypeScript/JavaScript - typescript-language-server
    configs.insert(
        "typescript".to_string(),
        LspServerConfig {
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec![
                "ts".to_string(),
                "tsx".to_string(),
                "js".to_string(),
                "jsx".to_string(),
                "mjs".to_string(),
                "cjs".to_string(),
            ],
            root_markers: vec![
                "tsconfig.json".to_string(),
                "jsconfig.json".to_string(),
                "package.json".to_string(),
            ],
            ..Default::default()
        },
    );

    // Python - pyright or pylsp
    configs.insert(
        "python".to_string(),
        LspServerConfig {
            command: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["py".to_string(), "pyi".to_string()],
            root_markers: vec![
                "pyproject.toml".to_string(),
                "setup.py".to_string(),
                "requirements.txt".to_string(),
                "pyrightconfig.json".to_string(),
            ],
            ..Default::default()
        },
    );

    // C/C++ - clangd
    configs.insert(
        "c".to_string(),
        LspServerConfig {
            command: "clangd".to_string(),
            extensions: vec![
                "c".to_string(),
                "h".to_string(),
                "cpp".to_string(),
                "hpp".to_string(),
                "cc".to_string(),
                "cxx".to_string(),
            ],
            root_markers: vec![
                "compile_commands.json".to_string(),
                "CMakeLists.txt".to_string(),
                ".clangd".to_string(),
            ],
            ..Default::default()
        },
    );

    // Java - jdtls
    configs.insert(
        "java".to_string(),
        LspServerConfig {
            command: "jdtls".to_string(),
            extensions: vec!["java".to_string()],
            root_markers: vec![
                "pom.xml".to_string(),
                "build.gradle".to_string(),
                "build.gradle.kts".to_string(),
                ".project".to_string(),
            ],
            ..Default::default()
        },
    );

    // Ruby - solargraph
    configs.insert(
        "ruby".to_string(),
        LspServerConfig {
            command: "solargraph".to_string(),
            args: vec!["stdio".to_string()],
            extensions: vec!["rb".to_string(), "rake".to_string()],
            root_markers: vec!["Gemfile".to_string(), ".ruby-version".to_string()],
            ..Default::default()
        },
    );

    // Elixir - elixir-ls
    configs.insert(
        "elixir".to_string(),
        LspServerConfig {
            command: "elixir-ls".to_string(),
            extensions: vec!["ex".to_string(), "exs".to_string()],
            root_markers: vec!["mix.exs".to_string()],
            ..Default::default()
        },
    );

    // Lua - lua-language-server
    configs.insert(
        "lua".to_string(),
        LspServerConfig {
            command: "lua-language-server".to_string(),
            extensions: vec!["lua".to_string()],
            root_markers: vec![".luarc.json".to_string(), ".luacheckrc".to_string()],
            ..Default::default()
        },
    );

    // Zig - zls
    configs.insert(
        "zig".to_string(),
        LspServerConfig {
            command: "zls".to_string(),
            extensions: vec!["zig".to_string()],
            root_markers: vec!["build.zig".to_string()],
            ..Default::default()
        },
    );

    // Bash - bash-language-server
    configs.insert(
        "bash".to_string(),
        LspServerConfig {
            command: "bash-language-server".to_string(),
            args: vec!["start".to_string()],
            extensions: vec!["sh".to_string(), "bash".to_string()],
            root_markers: vec![],
            ..Default::default()
        },
    );

    // YAML - yaml-language-server
    configs.insert(
        "yaml".to_string(),
        LspServerConfig {
            command: "yaml-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["yaml".to_string(), "yml".to_string()],
            root_markers: vec![],
            ..Default::default()
        },
    );

    // JSON - vscode-json-language-server
    configs.insert(
        "json".to_string(),
        LspServerConfig {
            command: "vscode-json-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["json".to_string(), "jsonc".to_string()],
            root_markers: vec![],
            ..Default::default()
        },
    );

    // HTML - vscode-html-language-server
    configs.insert(
        "html".to_string(),
        LspServerConfig {
            command: "vscode-html-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["html".to_string(), "htm".to_string()],
            root_markers: vec![],
            ..Default::default()
        },
    );

    // CSS - vscode-css-language-server
    configs.insert(
        "css".to_string(),
        LspServerConfig {
            command: "vscode-css-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["css".to_string(), "scss".to_string(), "less".to_string()],
            root_markers: vec![],
            ..Default::default()
        },
    );

    // Svelte - svelte-language-server
    configs.insert(
        "svelte".to_string(),
        LspServerConfig {
            command: "svelteserver".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["svelte".to_string()],
            root_markers: vec!["svelte.config.js".to_string()],
            ..Default::default()
        },
    );

    // Vue - vue-language-server
    configs.insert(
        "vue".to_string(),
        LspServerConfig {
            command: "vue-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["vue".to_string()],
            root_markers: vec!["vue.config.js".to_string(), "vite.config.ts".to_string()],
            ..Default::default()
        },
    );

    configs
}

/// Detect language ID from file extension
pub fn detect_language_id(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?.to_lowercase();

    match ext.as_str() {
        "rs" => Some("rust".to_string()),
        "go" | "mod" => Some("go".to_string()),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some("typescript".to_string()),
        "py" | "pyi" => Some("python".to_string()),
        "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" => Some("c".to_string()),
        "java" => Some("java".to_string()),
        "rb" | "rake" => Some("ruby".to_string()),
        "ex" | "exs" => Some("elixir".to_string()),
        "lua" => Some("lua".to_string()),
        "zig" => Some("zig".to_string()),
        "sh" | "bash" => Some("bash".to_string()),
        "yaml" | "yml" => Some("yaml".to_string()),
        "json" | "jsonc" => Some("json".to_string()),
        "html" | "htm" => Some("html".to_string()),
        "css" | "scss" | "less" => Some("css".to_string()),
        "svelte" => Some("svelte".to_string()),
        "vue" => Some("vue".to_string()),
        _ => None,
    }
}

/// Get file extension to language ID mapping
pub fn extension_to_language_id(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "go" => Some("go"),
        "mod" => Some("go"),
        "ts" => Some("typescript"),
        "tsx" => Some("typescriptreact"),
        "js" => Some("javascript"),
        "jsx" => Some("javascriptreact"),
        "mjs" | "cjs" => Some("javascript"),
        "py" | "pyi" => Some("python"),
        "c" => Some("c"),
        "h" => Some("c"),
        "cpp" | "cc" | "cxx" => Some("cpp"),
        "hpp" => Some("cpp"),
        "java" => Some("java"),
        "rb" | "rake" => Some("ruby"),
        "ex" | "exs" => Some("elixir"),
        "lua" => Some("lua"),
        "zig" => Some("zig"),
        "sh" | "bash" => Some("shellscript"),
        "yaml" | "yml" => Some("yaml"),
        "json" => Some("json"),
        "jsonc" => Some("jsonc"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "scss" => Some("scss"),
        "less" => Some("less"),
        "svelte" => Some("svelte"),
        "vue" => Some("vue"),
        "md" => Some("markdown"),
        "toml" => Some("toml"),
        _ => None,
    }
}
