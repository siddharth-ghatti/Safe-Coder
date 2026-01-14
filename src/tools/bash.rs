use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;

use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::{Tool, ToolContext};

pub struct BashTool;

#[derive(Debug, Deserialize)]
struct BashParams {
    command: String,
    #[serde(default)]
    timeout: Option<u64>,
}

/// Result of checking a command for dangerous patterns
#[derive(Debug)]
struct DangerCheck {
    is_dangerous: bool,
    matched_patterns: Vec<String>,
}

impl BashTool {
    /// Check if a command matches any dangerous patterns
    fn check_dangerous_command(command: &str, patterns: &[String]) -> DangerCheck {
        let mut matched = Vec::new();

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(command) {
                    matched.push(pattern.clone());
                }
            }
        }

        DangerCheck {
            is_dangerous: !matched.is_empty(),
            matched_patterns: matched,
        }
    }

    /// Truncate output if it exceeds the maximum size
    fn truncate_output(output: String, max_bytes: usize) -> String {
        if output.len() <= max_bytes {
            return output;
        }

        // Find a safe truncation point (don't cut in the middle of a UTF-8 character)
        let truncated = &output[..max_bytes];
        let safe_end = truncated
            .char_indices()
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);

        let mut result = output[..safe_end].to_string();
        let truncated_bytes = output.len() - safe_end;
        result.push_str(&format!(
            "\n\n[OUTPUT TRUNCATED: {} bytes omitted. Total output was {} bytes, limit is {} bytes]",
            truncated_bytes,
            output.len(),
            max_bytes
        ));

        result
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Executes a bash command in the project directory and returns the output. Commands have a configurable timeout (default: 120s) and output size limit."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in seconds (overrides default from config)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: BashParams = serde_json::from_value(params)
            .context("Invalid parameters for bash")?;

        // Check for dangerous commands if enabled
        if ctx.config.warn_dangerous_commands {
            let danger_check = Self::check_dangerous_command(
                &params.command,
                &ctx.config.dangerous_patterns
            );

            if danger_check.is_dangerous {
                return Ok(format!(
                    "⚠️  DANGEROUS COMMAND DETECTED\n\n\
                    The command '{}' matches dangerous patterns:\n{}\n\n\
                    This command has been blocked for safety. If you really need to run this command, \
                    you can disable dangerous command warnings in the config:\n\n\
                    [tools]\n\
                    warn_dangerous_commands = false\n\n\
                    Or remove the specific pattern from dangerous_patterns.",
                    params.command,
                    danger_check.matched_patterns
                        .iter()
                        .map(|p| format!("  - {}", p))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            }
        }

        // Use config timeout as default, allow override from params
        let timeout_secs = params.timeout.unwrap_or(ctx.config.bash_timeout_secs);
        let timeout = tokio::time::Duration::from_secs(timeout_secs);

        tracing::debug!(
            "Executing bash command with {}s timeout: {}",
            timeout_secs,
            &params.command
        );

        // Spawn the process with piped stdout/stderr for better control
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&params.command)
            .current_dir(ctx.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn command")?;

        // Get handles to stdout and stderr
        let mut stdout = child.stdout.take().context("Failed to capture stdout")?;
        let mut stderr = child.stderr.take().context("Failed to capture stderr")?;

        let mut combined_output = String::new();

        // Check if we have a callback for streaming output
        let use_streaming = ctx.output_callback.is_some();
        
        if use_streaming {
            // Stream output in real-time
            let callback = ctx.output_callback.as_ref().unwrap();
            
            let mut stdout_reader = BufReader::new(stdout);
            let mut stderr_reader = BufReader::new(stderr);
            
            let mut stdout_line = String::new();
            let mut stderr_line = String::new();
            
            let result = tokio::time::timeout(timeout, async {
                loop {
                    tokio::select! {
                        result = stdout_reader.read_line(&mut stdout_line) => {
                            match result {
                                Ok(0) => break, // EOF
                                Ok(_) => {
                                    let line = stdout_line.trim_end_matches('\n').to_string();
                                    if !line.is_empty() {
                                        combined_output.push_str(&stdout_line);
                                        callback(line);
                                    }
                                    stdout_line.clear();
                                }
                                Err(e) => return Err(e.into()),
                            }
                        }
                        result = stderr_reader.read_line(&mut stderr_line) => {
                            match result {
                                Ok(0) => {}, // EOF on stderr, continue
                                Ok(_) => {
                                    if !stderr_line.trim().is_empty() {
                                        let formatted_line = format!("stderr: {}", stderr_line.trim_end_matches('\n'));
                                        combined_output.push_str(&stderr_line);
                                        callback(formatted_line);
                                    }
                                    stderr_line.clear();
                                }
                                Err(e) => return Err(e.into()),
                            }
                        }
                        _ = child.wait() => {
                            // Process has completed, read any remaining output
                            while let Ok(n) = stdout_reader.read_line(&mut stdout_line).await {
                                if n == 0 { break; }
                                let line = stdout_line.trim_end_matches('\n').to_string();
                                if !line.is_empty() {
                                    combined_output.push_str(&stdout_line);
                                    callback(line);
                                }
                                stdout_line.clear();
                            }
                            while let Ok(n) = stderr_reader.read_line(&mut stderr_line).await {
                                if n == 0 { break; }
                                if !stderr_line.trim().is_empty() {
                                    let formatted_line = format!("stderr: {}", stderr_line.trim_end_matches('\n'));
                                    combined_output.push_str(&stderr_line);
                                    callback(formatted_line);
                                }
                                stderr_line.clear();
                            }
                            break;
                        }
                    }
                }
                
                let status = child.wait().await.context("Failed to wait for process")?;
                Ok::<_, anyhow::Error>(status)
            }).await;
            
            match result {
                Ok(Ok(status)) => {
                    if !status.success() {
                        let exit_msg = format!("[Exit status: {}]", status);
                        combined_output.push_str(&exit_msg);
                        if let Some(ref callback) = ctx.output_callback {
                            callback(exit_msg);
                        }
                    }
                    
                    // Truncate if necessary
                    Ok(Self::truncate_output(combined_output, ctx.config.max_output_bytes))
                }
                Ok(Err(e)) => Err(e),
                Err(_) => {
                    // Timeout occurred
                    tracing::warn!(
                        "Command timed out after {}s, attempting to kill process: {}",
                        timeout_secs,
                        &params.command
                    );

                    if let Err(kill_err) = child.kill().await {
                        tracing::error!("Failed to kill timed-out process: {}", kill_err);
                    }

                    let timeout_msg = format!(
                        "⏱️  COMMAND TIMED OUT\n\n\
                        The command '{}' exceeded the timeout of {} seconds and was terminated.\n\n\
                        Possible reasons:\n\
                        - The command is taking longer than expected\n\
                        - The command is waiting for input\n\
                        - The command entered an infinite loop\n\n\
                        You can:\n\
                        1. Increase the timeout by passing a 'timeout' parameter (e.g., timeout: 300)\n\
                        2. Modify the default timeout in config:\n\
                           [tools]\n\
                           bash_timeout_secs = 300\n\
                        3. Break the command into smaller operations",
                        params.command,
                        timeout_secs
                    );
                    
                    if let Some(ref callback) = ctx.output_callback {
                        callback(timeout_msg.clone());
                    }
                    
                    Ok(timeout_msg)
                }
            }
        } else {
            // Original non-streaming behavior for backward compatibility
            let result = tokio::time::timeout(timeout, async {
                let mut stdout_buf = Vec::new();
                let mut stderr_buf = Vec::new();

                // Read both streams concurrently
                let (stdout_result, stderr_result) = tokio::join!(
                    tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut stdout_buf),
                    tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut stderr_buf)
                );

                stdout_result.context("Failed to read stdout")?;
                stderr_result.context("Failed to read stderr")?;

                // Wait for the process to complete
                let status = child.wait().await.context("Failed to wait for process")?;

                Ok::<_, anyhow::Error>((stdout_buf, stderr_buf, status))
            })
            .await;

            match result {
                Ok(Ok((stdout_buf, stderr_buf, status))) => {
                    // Process completed within timeout
                    let stdout_str = String::from_utf8_lossy(&stdout_buf);
                    let stderr_str = String::from_utf8_lossy(&stderr_buf);

                    let mut output = String::new();

                    if !stdout_str.is_empty() {
                        output.push_str(&stdout_str);
                    }
                    if !stderr_str.is_empty() {
                        if !output.is_empty() {
                            output.push('\n');
                        }
                        output.push_str(&stderr_str);
                    }

                    if !status.success() {
                        output.push_str(&format!("\n[Exit status: {}]", status));
                    }

                    // Truncate if necessary
                    Ok(Self::truncate_output(output, ctx.config.max_output_bytes))
                }
                Ok(Err(e)) => {
                    // Process completed but had an error reading output
                    Err(e)
                }
                Err(_) => {
                    // Timeout occurred - try to kill the process
                    tracing::warn!(
                        "Command timed out after {}s, attempting to kill process: {}",
                        timeout_secs,
                        &params.command
                    );

                    // Try to kill the child process
                    if let Err(kill_err) = child.kill().await {
                        tracing::error!("Failed to kill timed-out process: {}", kill_err);
                    }

                    Ok(format!(
                        "⏱️  COMMAND TIMED OUT\n\n\
                        The command '{}' exceeded the timeout of {} seconds and was terminated.\n\n\
                        Possible reasons:\n\
                        - The command is taking longer than expected\n\
                        - The command is waiting for input\n\
                        - The command entered an infinite loop\n\n\
                        You can:\n\
                        1. Increase the timeout by passing a 'timeout' parameter (e.g., timeout: 300)\n\
                        2. Modify the default timeout in config:\n\
                           [tools]\n\
                           bash_timeout_secs = 300\n\
                        3. Break the command into smaller operations",
                        params.command,
                        timeout_secs
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_command_detection() {
        let patterns = vec![
            r"rm\s+-rf\s+/".to_string(),
            r"rm\s+-rf\s+~".to_string(),
        ];

        // Should detect dangerous commands
        let check = BashTool::check_dangerous_command("rm -rf /", &patterns);
        assert!(check.is_dangerous);

        let check = BashTool::check_dangerous_command("rm -rf ~/", &patterns);
        assert!(check.is_dangerous);

        // Should allow safe commands
        let check = BashTool::check_dangerous_command("rm -rf ./temp", &patterns);
        assert!(!check.is_dangerous);

        let check = BashTool::check_dangerous_command("ls -la", &patterns);
        assert!(!check.is_dangerous);
    }

    #[test]
    fn test_output_truncation() {
        let short_output = "Hello, World!".to_string();
        assert_eq!(
            BashTool::truncate_output(short_output.clone(), 100),
            short_output
        );

        let long_output = "a".repeat(1000);
        let truncated = BashTool::truncate_output(long_output.clone(), 100);
        assert!(truncated.len() < long_output.len());
        assert!(truncated.contains("OUTPUT TRUNCATED"));
    }
}
