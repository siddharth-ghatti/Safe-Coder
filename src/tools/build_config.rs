//! Build configuration tool
//!
//! Allows the LLM to detect and set the build command for the current project.

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

use super::{Tool, ToolContext};

/// Tool for detecting and configuring build commands
pub struct BuildConfigTool;

#[derive(Debug, Deserialize)]
struct BuildConfigParams {
    /// Action to perform: "detect", "set", or "get"
    action: String,
    /// Build command to set (only for "set" action)
    command: Option<String>,
}

#[async_trait]
impl Tool for BuildConfigTool {
    fn name(&self) -> &str {
        "build_config"
    }

    fn description(&self) -> &str {
        "Detect, get, or set the build command for the current project. Use 'detect' to auto-detect based on project files, 'get' to see current command, or 'set' to specify a custom command."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["detect", "get", "set"],
                    "description": "Action to perform: 'detect' auto-detects build command, 'get' returns current command, 'set' sets a custom command"
                },
                "command": {
                    "type": "string",
                    "description": "Build command to set (only used with 'set' action). Include '2>&1' to capture stderr."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: BuildConfigParams = serde_json::from_value(params)?;
        execute_build_config(params, ctx).await
    }
}

async fn execute_build_config(params: BuildConfigParams, ctx: &ToolContext<'_>) -> Result<String> {
    let project_path = ctx.working_dir;

    match params.action.as_str() {
        "detect" => detect_build_command(project_path).await,
        "get" => get_current_build_command(project_path),
        "set" => {
            if let Some(cmd) = params.command {
                set_build_command(&cmd)
            } else {
                Ok("Error: 'command' parameter is required for 'set' action".to_string())
            }
        }
        _ => Ok(format!(
            "Unknown action: {}. Use 'detect', 'get', or 'set'.",
            params.action
        )),
    }
}

/// Detect the build command by examining project files
async fn detect_build_command(project_path: &Path) -> Result<String> {
    let mut detected = Vec::new();

    // Check for various project markers and their build commands
    let markers = [
        ("Cargo.toml", "cargo build 2>&1", "Rust"),
        ("tsconfig.json", "npx tsc --noEmit 2>&1", "TypeScript"),
        ("package.json", "npm run build 2>&1", "Node.js"),
        ("go.mod", "go build ./... 2>&1", "Go"),
        ("pyproject.toml", "python -m compileall -q . 2>&1", "Python"),
        (
            "setup.py",
            "python -m compileall -q . 2>&1",
            "Python (legacy)",
        ),
        (
            "CMakeLists.txt",
            "cmake --build build 2>&1",
            "C/C++ (CMake)",
        ),
        ("Makefile", "make 2>&1", "Make"),
        ("build.gradle", "gradle build 2>&1", "Java/Kotlin (Gradle)"),
        ("build.gradle.kts", "gradle build 2>&1", "Kotlin (Gradle)"),
        ("pom.xml", "mvn compile 2>&1", "Java (Maven)"),
        ("mix.exs", "mix compile 2>&1", "Elixir"),
        ("Gemfile", "bundle exec rake build 2>&1", "Ruby"),
        ("composer.json", "composer install 2>&1", "PHP"),
        (
            "pubspec.yaml",
            "dart compile exe lib/main.dart 2>&1",
            "Dart/Flutter",
        ),
        (
            "Project.toml",
            "julia --project -e 'using Pkg; Pkg.instantiate()' 2>&1",
            "Julia",
        ),
        ("dune-project", "dune build 2>&1", "OCaml"),
        ("stack.yaml", "stack build 2>&1", "Haskell (Stack)"),
        ("cabal.project", "cabal build 2>&1", "Haskell (Cabal)"),
        ("build.zig", "zig build 2>&1", "Zig"),
        ("meson.build", "meson compile -C builddir 2>&1", "Meson"),
    ];

    for (marker, command, lang) in markers {
        if project_path.join(marker).exists() {
            detected.push((marker, command, lang));
        }
    }

    if detected.is_empty() {
        return Ok("No recognized project markers found.\n\n\
            To set a custom build command, use:\n\
            build_config(action: \"set\", command: \"your-build-command 2>&1\")\n\n\
            Common examples:\n\
            - C/C++: \"gcc -c *.c 2>&1\" or \"g++ -c *.cpp 2>&1\"\n\
            - Script languages: \"python -m py_compile script.py 2>&1\"\n\
            - Custom: \"./build.sh 2>&1\""
            .to_string());
    }

    let mut output = String::from("Detected project type(s):\n\n");

    for (marker, command, lang) in &detected {
        output.push_str(&format!("  {} ({}):\n", lang, marker));
        output.push_str(&format!("    Command: `{}`\n\n", command));
    }

    // Recommend the primary one (first detected)
    let (_, recommended_cmd, recommended_lang) = &detected[0];
    output.push_str(&format!(
        "Recommended build command for this {} project:\n\
        `{}`\n\n\
        Run this after every file edit to verify your changes compile.",
        recommended_lang, recommended_cmd
    ));

    // Also try running it to verify it works
    output.push_str("\n\nTo verify this command works, run:\n");
    output.push_str(&format!("bash {}", recommended_cmd));

    Ok(output)
}

/// Get the currently configured build command
fn get_current_build_command(project_path: &Path) -> Result<String> {
    // Check what would be detected
    let markers = [
        ("Cargo.toml", "cargo build 2>&1"),
        ("tsconfig.json", "npx tsc --noEmit 2>&1"),
        ("package.json", "npm run build 2>&1"),
        ("go.mod", "go build ./... 2>&1"),
        ("pyproject.toml", "python -m compileall -q . 2>&1"),
        ("CMakeLists.txt", "cmake --build build 2>&1"),
        ("build.gradle", "gradle build 2>&1"),
        ("build.gradle.kts", "gradle build 2>&1"),
        ("pom.xml", "mvn compile 2>&1"),
    ];

    for (marker, command) in markers {
        if project_path.join(marker).exists() {
            return Ok(format!(
                "Current build command (auto-detected from {}):\n`{}`\n\n\
                Use build_config(action: \"set\", command: \"...\") to override.",
                marker, command
            ));
        }
    }

    Ok("No build command configured or detected.\n\
        Use build_config(action: \"detect\") to auto-detect, or\n\
        Use build_config(action: \"set\", command: \"...\") to set manually."
        .to_string())
}

/// Set a custom build command (returns instructions since we can't modify config at runtime)
fn set_build_command(command: &str) -> Result<String> {
    // We can't actually modify the config at runtime, but we can tell the LLM to use this command
    Ok(format!(
        "Build command noted: `{}`\n\n\
        Use this command after every file edit:\n\
        `bash {}`\n\n\
        To make this permanent, add to your safe-coder config:\n\
        ```toml\n\
        [build.commands]\n\
        \"custom\" = \"{}\"\n\
        ```",
        command, command, command
    ))
}
