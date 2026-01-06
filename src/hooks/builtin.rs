//! Built-in Hooks
//!
//! Pre-built hooks that provide common functionality:
//! - CommentChecker: Warns about excessive comments in code
//! - ContextMonitor: Tracks token usage and warns at thresholds
//! - TodoEnforcer: Ensures todos are completed before session end

use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::types::{Hook, HookContext, HookResult, HookType};

/// Hook that checks for excessive comments in written code
pub struct CommentCheckerHook {
    /// Maximum comment-to-code ratio (0.0 to 1.0)
    max_comment_ratio: f32,
    /// Minimum lines of code to trigger check
    min_lines: usize,
}

impl CommentCheckerHook {
    pub fn new() -> Self {
        Self {
            max_comment_ratio: 0.3, // Warn if >30% comments
            min_lines: 10,
        }
    }

    pub fn with_ratio(mut self, ratio: f32) -> Self {
        self.max_comment_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    fn count_comments(&self, content: &str, file_ext: Option<&str>) -> (usize, usize) {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        if total_lines < self.min_lines {
            return (0, total_lines);
        }

        let comment_prefixes = match file_ext {
            Some("rs") | Some("go") | Some("js") | Some("ts") | Some("tsx") | Some("jsx") => {
                vec!["//", "/*", " *", "*/"]
            }
            Some("py") | Some("pyi") => vec!["#", "'''", "\"\"\""],
            Some("rb") => vec!["#"],
            Some("sh") | Some("bash") => vec!["#"],
            _ => vec!["//", "#", "/*"],
        };

        let comment_lines = lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                comment_prefixes
                    .iter()
                    .any(|prefix| trimmed.starts_with(prefix))
            })
            .count();

        (comment_lines, total_lines)
    }
}

impl Default for CommentCheckerHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for CommentCheckerHook {
    fn name(&self) -> &str {
        "comment_checker"
    }

    fn hook_types(&self) -> &[HookType] {
        &[HookType::PreFileWrite]
    }

    fn description(&self) -> &str {
        "Warns when code has excessive comments (>30% of lines)"
    }

    async fn execute(&self, ctx: &HookContext) -> HookResult {
        let content = match &ctx.file_content {
            Some(c) => c,
            None => return HookResult::Continue,
        };

        let file_ext = ctx.file_path.as_ref().and_then(|p| p.rsplit('.').next());

        let (comment_lines, total_lines) = self.count_comments(content, file_ext);

        if total_lines >= self.min_lines {
            let ratio = comment_lines as f32 / total_lines as f32;
            if ratio > self.max_comment_ratio {
                return HookResult::ContinueWithWarning(format!(
                    "⚠️ High comment ratio: {:.0}% of lines are comments ({}/{} lines). Consider reducing comments.",
                    ratio * 100.0,
                    comment_lines,
                    total_lines
                ));
            }
        }

        HookResult::Continue
    }
}

/// Hook that monitors context window usage and warns at thresholds
pub struct ContextMonitorHook {
    /// Warning threshold (percentage)
    warning_threshold: f32,
    /// Critical threshold (percentage)
    critical_threshold: f32,
    /// Track if we've already warned
    warned: AtomicUsize,
}

impl ContextMonitorHook {
    pub fn new() -> Self {
        Self {
            warning_threshold: 70.0,
            critical_threshold: 85.0,
            warned: AtomicUsize::new(0),
        }
    }

    pub fn with_thresholds(mut self, warning: f32, critical: f32) -> Self {
        self.warning_threshold = warning;
        self.critical_threshold = critical;
        self
    }
}

impl Default for ContextMonitorHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for ContextMonitorHook {
    fn name(&self) -> &str {
        "context_monitor"
    }

    fn hook_types(&self) -> &[HookType] {
        &[HookType::PostResponse]
    }

    fn description(&self) -> &str {
        "Monitors context window usage and warns when approaching limits"
    }

    async fn execute(&self, ctx: &HookContext) -> HookResult {
        let usage = match &ctx.token_usage {
            Some(u) => u,
            None => return HookResult::Continue,
        };

        let current_warned = self.warned.load(Ordering::Relaxed);

        if usage.usage_percent >= self.critical_threshold {
            if current_warned < 2 {
                self.warned.store(2, Ordering::Relaxed);
                return HookResult::ContinueWithWarning(format!(
                    "🔴 CRITICAL: Context window at {:.1}% ({}/{} tokens). Use /compact to free up space or start a new session.",
                    usage.usage_percent,
                    usage.total_tokens,
                    usage.max_tokens
                ));
            }
        } else if usage.usage_percent >= self.warning_threshold {
            if current_warned < 1 {
                self.warned.store(1, Ordering::Relaxed);
                return HookResult::ContinueWithWarning(format!(
                    "⚠️ Context window at {:.1}% ({}/{} tokens). Consider using /compact soon.",
                    usage.usage_percent, usage.total_tokens, usage.max_tokens
                ));
            }
        }

        HookResult::Continue
    }
}

/// Hook that ensures todos are completed before session ends
pub struct TodoEnforcerHook {
    /// Whether to block session end if todos remain
    block_on_incomplete: bool,
}

impl TodoEnforcerHook {
    pub fn new() -> Self {
        Self {
            block_on_incomplete: false, // Warn by default, don't block
        }
    }

    pub fn blocking(mut self) -> Self {
        self.block_on_incomplete = true;
        self
    }
}

impl Default for TodoEnforcerHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for TodoEnforcerHook {
    fn name(&self) -> &str {
        "todo_enforcer"
    }

    fn hook_types(&self) -> &[HookType] {
        &[HookType::SessionEnd]
    }

    fn description(&self) -> &str {
        "Reminds about incomplete todos when session ends"
    }

    async fn execute(&self, ctx: &HookContext) -> HookResult {
        // Check if there are incomplete todos in metadata
        let incomplete_count = ctx
            .metadata
            .get("incomplete_todos")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        if incomplete_count > 0 {
            let message = format!(
                "📋 {} incomplete todo(s) remaining. Consider completing them before ending the session.",
                incomplete_count
            );

            if self.block_on_incomplete {
                return HookResult::Block(message);
            } else {
                return HookResult::ContinueWithWarning(message);
            }
        }

        HookResult::Continue
    }
}

/// Hook that validates edit operations won't break syntax
pub struct EditValidatorHook;

impl EditValidatorHook {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Hook for EditValidatorHook {
    fn name(&self) -> &str {
        "edit_validator"
    }

    fn hook_types(&self) -> &[HookType] {
        &[HookType::PreFileWrite]
    }

    fn description(&self) -> &str {
        "Validates that file edits maintain basic syntax validity"
    }

    async fn execute(&self, ctx: &HookContext) -> HookResult {
        let content = match &ctx.file_content {
            Some(c) => c,
            None => return HookResult::Continue,
        };

        let file_ext = ctx.file_path.as_ref().and_then(|p| p.rsplit('.').next());

        // Basic bracket matching check
        let mut brace_count = 0i32;
        let mut bracket_count = 0i32;
        let mut paren_count = 0i32;

        for ch in content.chars() {
            match ch {
                '{' => brace_count += 1,
                '}' => brace_count -= 1,
                '[' => bracket_count += 1,
                ']' => bracket_count -= 1,
                '(' => paren_count += 1,
                ')' => paren_count -= 1,
                _ => {}
            }

            // Early exit on negative counts (closing before opening)
            if brace_count < 0 || bracket_count < 0 || paren_count < 0 {
                return HookResult::ContinueWithWarning(format!(
                    "⚠️ Possible syntax error: unmatched closing bracket in {}",
                    ctx.file_path.as_deref().unwrap_or("file")
                ));
            }
        }

        // Check for unmatched brackets at end
        if brace_count != 0 || bracket_count != 0 || paren_count != 0 {
            let mut issues = Vec::new();
            if brace_count != 0 {
                issues.push(format!(
                    "{} unmatched {}",
                    brace_count.abs(),
                    if brace_count > 0 { "{" } else { "}" }
                ));
            }
            if bracket_count != 0 {
                issues.push(format!(
                    "{} unmatched {}",
                    bracket_count.abs(),
                    if bracket_count > 0 { "[" } else { "]" }
                ));
            }
            if paren_count != 0 {
                issues.push(format!(
                    "{} unmatched {}",
                    paren_count.abs(),
                    if paren_count > 0 { "(" } else { ")" }
                ));
            }

            return HookResult::ContinueWithWarning(format!(
                "⚠️ Possible syntax error in {}: {}",
                ctx.file_path.as_deref().unwrap_or("file"),
                issues.join(", ")
            ));
        }

        // Language-specific checks
        match file_ext {
            Some("rs") => {
                // Check for common Rust issues
                if content.contains("fn main(")
                    && !content.contains("fn main()")
                    && !content.contains("fn main(")
                {
                    // This is fine, main can have args
                }
            }
            Some("py") => {
                // Check for mixed indentation
                let has_tabs = content.contains('\t');
                let has_spaces = content.lines().any(|l| l.starts_with("    "));
                if has_tabs && has_spaces {
                    return HookResult::ContinueWithWarning(
                        "⚠️ Mixed tabs and spaces detected in Python file. This may cause IndentationError.".to_string()
                    );
                }
            }
            _ => {}
        }

        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_comment_checker_high_ratio() {
        let hook = CommentCheckerHook::new();

        let content = r#"
// Comment 1
// Comment 2
// Comment 3
// Comment 4
// Comment 5
fn main() {
    println!("Hello");
}
// More comments
// Even more
"#;

        let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(content));
        let result = hook.execute(&ctx).await;

        assert!(matches!(result, HookResult::ContinueWithWarning(_)));
    }

    #[tokio::test]
    async fn test_comment_checker_low_ratio() {
        let hook = CommentCheckerHook::new();

        let content = r#"
fn main() {
    println!("Hello");
    let x = 1;
    let y = 2;
    let z = x + y;
    println!("{}", z);
    for i in 0..10 {
        println!("{}", i);
    }
}
"#;

        let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(content));
        let result = hook.execute(&ctx).await;

        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_edit_validator_unmatched_braces() {
        let hook = EditValidatorHook::new();

        let content = "fn main() { println!(\"hello\"); "; // Missing closing brace

        let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(content));
        let result = hook.execute(&ctx).await;

        assert!(matches!(result, HookResult::ContinueWithWarning(_)));
    }
}
