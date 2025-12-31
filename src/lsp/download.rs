//! LSP Server Auto-Download
//!
//! Automatically downloads and installs language servers like Zed does.
//! Supports:
//! - GitHub releases (rust-analyzer, lua-language-server, etc.)
//! - npm packages (typescript-language-server, yaml-language-server, etc.)
//! - pip packages (python-lsp-server, etc.)
//! - cargo install (some Rust-based servers)

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;

/// Installation method for a language server
#[derive(Debug, Clone)]
pub enum InstallMethod {
    /// Download from GitHub releases
    GitHubRelease {
        repo: String,          // e.g., "rust-lang/rust-analyzer"
        asset_pattern: String, // e.g., "rust-analyzer-{arch}-{os}.gz"
        binary_name: String,   // e.g., "rust-analyzer"
    },
    /// Install via npm
    Npm {
        package: String,     // e.g., "typescript-language-server"
        binary_name: String, // e.g., "typescript-language-server"
    },
    /// Install via pip
    Pip {
        package: String,     // e.g., "python-lsp-server"
        binary_name: String, // e.g., "pylsp"
    },
    /// Install via cargo
    Cargo {
        crate_name: String,  // e.g., "taplo-cli"
        binary_name: String, // e.g., "taplo"
    },
    /// Direct URL download
    DirectDownload { url: String, binary_name: String },
    /// No auto-install available (must be installed manually)
    Manual,
}

/// LSP server download/install information
#[derive(Debug, Clone)]
pub struct LspInstallInfo {
    pub language: String,
    pub server_name: String,
    pub install_method: InstallMethod,
}

/// Get the LSP servers directory
pub fn lsp_servers_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .context("Could not determine data directory")?;

    Ok(data_dir.join("safe-coder").join("lsp-servers"))
}

/// Get the binary path for an installed LSP server
pub fn get_lsp_binary_path(binary_name: &str) -> Result<PathBuf> {
    let servers_dir = lsp_servers_dir()?;
    let bin_dir = servers_dir.join("bin");

    #[cfg(target_os = "windows")]
    let binary = format!("{}.exe", binary_name);
    #[cfg(not(target_os = "windows"))]
    let binary = binary_name.to_string();

    Ok(bin_dir.join(binary))
}

/// Check if an LSP server is installed (either in PATH or in our directory)
pub fn is_lsp_installed(binary_name: &str) -> bool {
    // Check PATH first
    if which::which(binary_name).is_ok() {
        return true;
    }

    // Check our installation directory
    if let Ok(path) = get_lsp_binary_path(binary_name) {
        if path.exists() {
            return true;
        }
    }

    false
}

/// Check if a binary path is valid and executable
/// This runs the binary with --version or --help to verify it works
fn is_binary_valid(path: &std::path::Path) -> bool {
    use std::process::Command;

    // Try running with --version first (most common)
    if let Ok(output) = Command::new(path)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        if output.success() {
            return true;
        }
    }

    // Some binaries don't support --version, try --help
    if let Ok(output) = Command::new(path)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        if output.success() {
            return true;
        }
    }

    false
}

/// Get the effective binary path (prefers PATH, falls back to our install)
/// Validates that the binary actually works before returning it
pub fn get_effective_binary_path(binary_name: &str) -> Option<PathBuf> {
    // Check PATH first (like Zed does)
    if let Ok(path) = which::which(binary_name) {
        // Verify the binary actually works (handles broken rustup shims, etc.)
        if is_binary_valid(&path) {
            return Some(path);
        }
    }

    // Check our installation directory
    if let Ok(path) = get_lsp_binary_path(binary_name) {
        if path.exists() && is_binary_valid(&path) {
            return Some(path);
        }
    }

    None
}

/// Download and install an LSP server
pub async fn install_lsp_server(info: &LspInstallInfo) -> Result<PathBuf> {
    let servers_dir = lsp_servers_dir()?;
    fs::create_dir_all(&servers_dir).await?;

    match &info.install_method {
        InstallMethod::GitHubRelease {
            repo,
            asset_pattern,
            binary_name,
        } => install_from_github(repo, asset_pattern, binary_name, &servers_dir).await,
        InstallMethod::Npm {
            package,
            binary_name,
        } => install_from_npm(package, binary_name, &servers_dir).await,
        InstallMethod::Pip {
            package,
            binary_name,
        } => install_from_pip(package, binary_name, &servers_dir).await,
        InstallMethod::Cargo {
            crate_name,
            binary_name,
        } => install_from_cargo(crate_name, binary_name, &servers_dir).await,
        InstallMethod::DirectDownload { url, binary_name } => {
            install_from_url(url, binary_name, &servers_dir).await
        }
        InstallMethod::Manual => Err(anyhow::anyhow!(
            "Server {} must be installed manually",
            info.server_name
        )),
    }
}

/// Create a reqwest client with reasonable timeouts for LSP downloads
fn create_download_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("Failed to create HTTP client")
}

/// Install from GitHub releases
async fn install_from_github(
    repo: &str,
    asset_pattern: &str,
    binary_name: &str,
    servers_dir: &Path,
) -> Result<PathBuf> {
    let client = create_download_client()?;

    // Get latest release info
    let release_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let response = client
        .get(&release_url)
        .header("User-Agent", "safe-coder")
        .send()
        .await
        .context("Failed to fetch release info")?;

    let release: serde_json::Value = response.json().await?;

    // Determine platform
    let (os, arch) = get_platform_info();

    // Find matching asset
    let asset_name = asset_pattern.replace("{os}", os).replace("{arch}", arch);

    let assets = release["assets"]
        .as_array()
        .context("No assets in release")?;

    let asset = assets
        .iter()
        .find(|a| {
            let name = a["name"].as_str().unwrap_or("");
            name.contains(&asset_name) || name.contains(os) && name.contains(arch)
        })
        .context(format!("No matching asset found for {}", asset_name))?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .context("No download URL")?;

    // Download the asset
    let bin_dir = servers_dir.join("bin");
    fs::create_dir_all(&bin_dir).await?;

    let response = client
        .get(download_url)
        .header("User-Agent", "safe-coder")
        .send()
        .await?;

    let bytes = response.bytes().await?;

    // Determine output path
    #[cfg(target_os = "windows")]
    let binary_path = bin_dir.join(format!("{}.exe", binary_name));
    #[cfg(not(target_os = "windows"))]
    let binary_path = bin_dir.join(binary_name);

    // Handle different archive types
    let asset_name = asset["name"].as_str().unwrap_or("");
    if asset_name.ends_with(".gz") && !asset_name.ends_with(".tar.gz") {
        // Gzip compressed single file
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        fs::write(&binary_path, decompressed).await?;
    } else if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
        // Tar.gz archive
        extract_tar_gz(&bytes, &bin_dir, binary_name).await?;
    } else if asset_name.ends_with(".zip") {
        // Zip archive
        extract_zip(&bytes, &bin_dir, binary_name).await?;
    } else {
        // Assume it's a raw binary
        fs::write(&binary_path, bytes).await?;
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary_path, perms)?;
    }

    Ok(binary_path)
}

/// Install from npm
async fn install_from_npm(package: &str, binary_name: &str, servers_dir: &Path) -> Result<PathBuf> {
    // Check if npm is available
    if which::which("npm").is_err() {
        return Err(anyhow::anyhow!(
            "npm is not installed. Please install Node.js first."
        ));
    }

    let npm_dir = servers_dir.join("npm");
    fs::create_dir_all(&npm_dir).await?;

    // Install package locally
    let status = Command::new("npm")
        .args(["install", "--prefix", npm_dir.to_str().unwrap(), package])
        .status()
        .context("Failed to run npm install")?;

    if !status.success() {
        return Err(anyhow::anyhow!("npm install failed for {}", package));
    }

    // Find the binary
    let bin_path = npm_dir.join("node_modules").join(".bin").join(binary_name);

    if !bin_path.exists() {
        return Err(anyhow::anyhow!(
            "Binary {} not found after npm install",
            binary_name
        ));
    }

    // Create symlink in our bin directory
    let bin_dir = servers_dir.join("bin");
    fs::create_dir_all(&bin_dir).await?;

    #[cfg(target_os = "windows")]
    let link_path = bin_dir.join(format!("{}.cmd", binary_name));
    #[cfg(not(target_os = "windows"))]
    let link_path = bin_dir.join(binary_name);

    if link_path.exists() {
        fs::remove_file(&link_path).await?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(&bin_path, &link_path)?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&bin_path, &link_path)?;

    Ok(link_path)
}

/// Install from pip
async fn install_from_pip(package: &str, binary_name: &str, servers_dir: &Path) -> Result<PathBuf> {
    // Check if pip is available
    let pip_cmd = if which::which("pip3").is_ok() {
        "pip3"
    } else {
        "pip"
    };
    if which::which(pip_cmd).is_err() {
        return Err(anyhow::anyhow!(
            "pip is not installed. Please install Python first."
        ));
    }

    let pip_dir = servers_dir.join("pip");
    fs::create_dir_all(&pip_dir).await?;

    // Install package to our directory
    let status = Command::new(pip_cmd)
        .args(["install", "--target", pip_dir.to_str().unwrap(), package])
        .status()
        .context("Failed to run pip install")?;

    if !status.success() {
        return Err(anyhow::anyhow!("pip install failed for {}", package));
    }

    // Find the binary in pip's bin directory
    let bin_path = pip_dir.join("bin").join(binary_name);

    if bin_path.exists() {
        return Ok(bin_path);
    }

    // On some systems, scripts are in Scripts (Windows) or directly in the package
    let scripts_path = pip_dir.join("Scripts").join(binary_name);
    if scripts_path.exists() {
        return Ok(scripts_path);
    }

    Err(anyhow::anyhow!(
        "Binary {} not found after pip install",
        binary_name
    ))
}

/// Install from cargo
async fn install_from_cargo(
    crate_name: &str,
    binary_name: &str,
    servers_dir: &Path,
) -> Result<PathBuf> {
    // Check if cargo is available
    if which::which("cargo").is_err() {
        return Err(anyhow::anyhow!(
            "cargo is not installed. Please install Rust first."
        ));
    }

    let cargo_dir = servers_dir.join("cargo");
    fs::create_dir_all(&cargo_dir).await?;

    // Install to our directory
    let status = Command::new("cargo")
        .args(["install", "--root", cargo_dir.to_str().unwrap(), crate_name])
        .status()
        .context("Failed to run cargo install")?;

    if !status.success() {
        return Err(anyhow::anyhow!("cargo install failed for {}", crate_name));
    }

    let bin_path = cargo_dir.join("bin").join(binary_name);

    if !bin_path.exists() {
        return Err(anyhow::anyhow!(
            "Binary {} not found after cargo install",
            binary_name
        ));
    }

    Ok(bin_path)
}

/// Install from direct URL
async fn install_from_url(url: &str, binary_name: &str, servers_dir: &Path) -> Result<PathBuf> {
    let client = create_download_client()?;

    let response = client
        .get(url)
        .header("User-Agent", "safe-coder")
        .send()
        .await?;

    let bytes = response.bytes().await?;

    let bin_dir = servers_dir.join("bin");
    fs::create_dir_all(&bin_dir).await?;

    #[cfg(target_os = "windows")]
    let binary_path = bin_dir.join(format!("{}.exe", binary_name));
    #[cfg(not(target_os = "windows"))]
    let binary_path = bin_dir.join(binary_name);

    fs::write(&binary_path, bytes).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary_path, perms)?;
    }

    Ok(binary_path)
}

/// Extract tar.gz archive
async fn extract_tar_gz(data: &[u8], dest_dir: &Path, binary_name: &str) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);

    // Extract all files, looking for the binary
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        // Check if this is the binary we're looking for
        if let Some(file_name) = path.file_name() {
            let file_name_str = file_name.to_string_lossy();
            if file_name_str == binary_name || file_name_str.starts_with(binary_name) {
                let dest_path = dest_dir.join(binary_name);
                entry.unpack(&dest_path)?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&dest_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dest_path, perms)?;
                }

                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!(
        "Binary {} not found in archive",
        binary_name
    ))
}

/// Extract zip archive
async fn extract_zip(data: &[u8], dest_dir: &Path, binary_name: &str) -> Result<()> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let reader = Cursor::new(data);
    let mut archive = ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let path = file.name();

        // Check if this is the binary we're looking for
        if let Some(file_name) = Path::new(path).file_name() {
            let file_name_str = file_name.to_string_lossy();
            if file_name_str == binary_name || file_name_str.starts_with(binary_name) {
                let dest_path = dest_dir.join(binary_name);
                let mut out = std::fs::File::create(&dest_path)?;
                std::io::copy(&mut file, &mut out)?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&dest_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dest_path, perms)?;
                }

                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!(
        "Binary {} not found in archive",
        binary_name
    ))
}

/// Get platform info for downloading correct binary
fn get_platform_info() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    };

    (os, arch)
}

/// Get all supported LSP servers with their installation info
pub fn get_lsp_install_info() -> Vec<LspInstallInfo> {
    vec![
        // Rust - rust-analyzer (GitHub releases)
        LspInstallInfo {
            language: "rust".to_string(),
            server_name: "rust-analyzer".to_string(),
            install_method: InstallMethod::GitHubRelease {
                repo: "rust-lang/rust-analyzer".to_string(),
                asset_pattern: "rust-analyzer-{arch}-apple-{os}".to_string(),
                binary_name: "rust-analyzer".to_string(),
            },
        },
        // TypeScript - typescript-language-server (npm)
        LspInstallInfo {
            language: "typescript".to_string(),
            server_name: "typescript-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "typescript-language-server typescript".to_string(),
                binary_name: "typescript-language-server".to_string(),
            },
        },
        // Python - pyright (npm)
        LspInstallInfo {
            language: "python".to_string(),
            server_name: "pyright".to_string(),
            install_method: InstallMethod::Npm {
                package: "pyright".to_string(),
                binary_name: "pyright-langserver".to_string(),
            },
        },
        // Go - gopls (go install, treated as manual for now)
        LspInstallInfo {
            language: "go".to_string(),
            server_name: "gopls".to_string(),
            install_method: InstallMethod::Manual, // Requires: go install golang.org/x/tools/gopls@latest
        },
        // Lua - lua-language-server (GitHub releases)
        LspInstallInfo {
            language: "lua".to_string(),
            server_name: "lua-language-server".to_string(),
            install_method: InstallMethod::GitHubRelease {
                repo: "LuaLS/lua-language-server".to_string(),
                asset_pattern: "lua-language-server-".to_string(),
                binary_name: "lua-language-server".to_string(),
            },
        },
        // YAML - yaml-language-server (npm)
        LspInstallInfo {
            language: "yaml".to_string(),
            server_name: "yaml-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "yaml-language-server".to_string(),
                binary_name: "yaml-language-server".to_string(),
            },
        },
        // JSON - vscode-json-language-server (npm)
        LspInstallInfo {
            language: "json".to_string(),
            server_name: "vscode-json-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "vscode-langservers-extracted".to_string(),
                binary_name: "vscode-json-language-server".to_string(),
            },
        },
        // HTML - vscode-html-language-server (npm)
        LspInstallInfo {
            language: "html".to_string(),
            server_name: "vscode-html-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "vscode-langservers-extracted".to_string(),
                binary_name: "vscode-html-language-server".to_string(),
            },
        },
        // CSS - vscode-css-language-server (npm)
        LspInstallInfo {
            language: "css".to_string(),
            server_name: "vscode-css-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "vscode-langservers-extracted".to_string(),
                binary_name: "vscode-css-language-server".to_string(),
            },
        },
        // Bash - bash-language-server (npm)
        LspInstallInfo {
            language: "bash".to_string(),
            server_name: "bash-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "bash-language-server".to_string(),
                binary_name: "bash-language-server".to_string(),
            },
        },
        // TOML - taplo (cargo)
        LspInstallInfo {
            language: "toml".to_string(),
            server_name: "taplo".to_string(),
            install_method: InstallMethod::Cargo {
                crate_name: "taplo-cli".to_string(),
                binary_name: "taplo".to_string(),
            },
        },
        // Svelte - svelte-language-server (npm)
        LspInstallInfo {
            language: "svelte".to_string(),
            server_name: "svelte-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "svelte-language-server".to_string(),
                binary_name: "svelteserver".to_string(),
            },
        },
        // Vue - vue-language-server (npm)
        LspInstallInfo {
            language: "vue".to_string(),
            server_name: "vue-language-server".to_string(),
            install_method: InstallMethod::Npm {
                package: "@vue/language-server".to_string(),
                binary_name: "vue-language-server".to_string(),
            },
        },
        // C/C++ - clangd (manual - part of LLVM)
        LspInstallInfo {
            language: "c".to_string(),
            server_name: "clangd".to_string(),
            install_method: InstallMethod::Manual, // Part of LLVM, install via package manager
        },
        // Java - jdtls (manual - complex setup)
        LspInstallInfo {
            language: "java".to_string(),
            server_name: "jdtls".to_string(),
            install_method: InstallMethod::Manual, // Requires Eclipse JDT.LS setup
        },
        // Ruby - solargraph (gem, treated as manual)
        LspInstallInfo {
            language: "ruby".to_string(),
            server_name: "solargraph".to_string(),
            install_method: InstallMethod::Manual, // Requires: gem install solargraph
        },
        // Zig - zls (GitHub releases)
        LspInstallInfo {
            language: "zig".to_string(),
            server_name: "zls".to_string(),
            install_method: InstallMethod::GitHubRelease {
                repo: "zigtools/zls".to_string(),
                asset_pattern: "zls-".to_string(),
                binary_name: "zls".to_string(),
            },
        },
    ]
}

/// Get install info for a specific language
pub fn get_install_info_for_language(language: &str) -> Option<LspInstallInfo> {
    get_lsp_install_info()
        .into_iter()
        .find(|info| info.language == language)
}
