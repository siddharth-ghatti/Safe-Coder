//! Doom Loop Detection
//!
//! Detects when the AI is stuck in a loop, repeatedly calling the same tool
//! with the same parameters. This prevents infinite retry loops and wasted tokens.
//!
//! Also tracks error TYPES - if the same error pattern keeps occurring even with
//! different fix attempts, we detect that as a loop.

use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use regex::Regex;

/// Action to take when a doom loop is detected
#[derive(Debug, Clone)]
pub enum DoomLoopAction {
    /// Continue normally (no loop detected)
    Continue,
    /// Warn the user but allow continuation
    Warn { message: String },
    /// Ask user whether to continue
    AskUser { message: String },
    /// Block the action entirely
    Block { message: String },
}

/// Represents a tool call for comparison
#[derive(Debug, Clone, PartialEq)]
struct ToolCall {
    tool_name: String,
    /// Normalized parameters (sorted keys for consistent comparison)
    params_hash: String,
}

impl ToolCall {
    fn new(tool_name: &str, params: &Value) -> Self {
        // Create a normalized hash of parameters for comparison
        // This handles cases where JSON key order might differ
        let params_hash = Self::normalize_params(params);
        Self {
            tool_name: tool_name.to_string(),
            params_hash,
        }
    }

    fn normalize_params(params: &Value) -> String {
        // For simple comparison, we'll use the canonical JSON representation
        // This sorts object keys for consistent comparison
        match params {
            Value::Object(map) => {
                let mut pairs: Vec<_> = map.iter().collect();
                pairs.sort_by(|a, b| a.0.cmp(b.0));
                let sorted: serde_json::Map<String, Value> = pairs
                    .into_iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                serde_json::to_string(&Value::Object(sorted)).unwrap_or_default()
            }
            _ => serde_json::to_string(params).unwrap_or_default(),
        }
    }
}

/// Configuration for doom loop detection
#[derive(Debug, Clone)]
pub struct LoopDetectorConfig {
    /// Maximum number of recent calls to track
    pub max_history: usize,
    /// Number of identical calls before warning
    pub warn_threshold: usize,
    /// Number of identical calls before asking user
    pub ask_threshold: usize,
    /// Number of identical calls before blocking
    pub block_threshold: usize,
    /// Whether to include similar (not just identical) calls
    pub detect_similar: bool,
}

impl Default for LoopDetectorConfig {
    fn default() -> Self {
        Self {
            max_history: 15,     // Smaller window for faster detection
            warn_threshold: 1,   // Warn on 1st repeat (2 total)
            ask_threshold: 2,    // Ask on 2nd repeat (3 total) - more aggressive
            block_threshold: 3,  // Block on 3rd repeat (4 total) - more aggressive
            detect_similar: true,
        }
    }
}

/// Represents a build error with extracted type information
#[derive(Debug, Clone)]
pub struct ErrorPattern {
    /// Error code like "E0433", "E0412", etc.
    pub code: Option<String>,
    /// Error category: "cannot_find", "type_mismatch", "borrow", "lifetime", etc.
    pub category: String,
    /// The specific symbol/type that's problematic (if extractable)
    pub target: Option<String>,
    /// Full error message (first line)
    pub message: String,
}

impl ErrorPattern {
    /// Extract error patterns from build output
    pub fn extract_from_output(output: &str) -> Vec<ErrorPattern> {
        let mut patterns = Vec::new();

        // Rust error code pattern: error[E0433]
        let code_re = Regex::new(r"error\[E(\d+)\]").ok();

        // Common error patterns
        for line in output.lines() {
            let line_lower = line.to_lowercase();

            // Skip non-error lines
            if !line_lower.contains("error") {
                continue;
            }

            // Extract error code if present
            let code = code_re.as_ref().and_then(|re| {
                re.captures(line).map(|c| format!("E{}", &c[1]))
            });

            // Categorize the error
            let (category, target) = if line_lower.contains("cannot find") {
                let target = Self::extract_target(line, &["type", "value", "module", "crate"]);
                ("cannot_find".to_string(), target)
            } else if line_lower.contains("not found") {
                let target = Self::extract_target(line, &["type", "value", "module"]);
                ("not_found".to_string(), target)
            } else if line_lower.contains("expected") && line_lower.contains("found") {
                ("type_mismatch".to_string(), None)
            } else if line_lower.contains("borrow") {
                ("borrow_error".to_string(), None)
            } else if line_lower.contains("lifetime") {
                ("lifetime_error".to_string(), None)
            } else if line_lower.contains("missing") {
                let target = Self::extract_target(line, &["field", "argument", "parameter"]);
                ("missing".to_string(), target)
            } else if line_lower.contains("unused") {
                continue; // Skip warnings
            } else {
                ("other".to_string(), None)
            };

            patterns.push(ErrorPattern {
                code,
                category,
                target,
                message: line.chars().take(100).collect(),
            });
        }

        patterns
    }

    /// Try to extract the target symbol from an error message
    fn extract_target(line: &str, keywords: &[&str]) -> Option<String> {
        // Look for patterns like "cannot find type `Foo`" or "value `bar` not found"
        let backtick_re = Regex::new(r"`([^`]+)`").ok()?;

        for keyword in keywords {
            if line.to_lowercase().contains(keyword) {
                if let Some(caps) = backtick_re.captures(line) {
                    return Some(caps[1].to_string());
                }
            }
        }
        None
    }

    /// Check if two error patterns are the same type of error
    pub fn same_type(&self, other: &ErrorPattern) -> bool {
        // Same error code is definitely the same error
        if let (Some(c1), Some(c2)) = (&self.code, &other.code) {
            if c1 == c2 {
                return true;
            }
        }

        // Same category + same target is the same error
        if self.category == other.category {
            if let (Some(t1), Some(t2)) = (&self.target, &other.target) {
                return t1 == t2;
            }
            // Same category without target - might be same error
            return self.category != "other";
        }

        false
    }
}

/// Detects doom loops - repeated tool calls with same parameters
#[derive(Debug)]
pub struct LoopDetector {
    recent_calls: VecDeque<ToolCall>,
    config: LoopDetectorConfig,
    /// Track consecutive failures (tool errors)
    consecutive_failures: usize,
    /// Last error message (for similarity detection)
    last_error: Option<String>,
    /// Track error patterns to detect same-error loops
    error_history: Vec<ErrorPattern>,
    /// Count of times each error type has occurred
    error_type_counts: HashMap<String, usize>,
}

impl LoopDetector {
    /// Create a new loop detector with default configuration
    pub fn new() -> Self {
        Self::with_config(LoopDetectorConfig::default())
    }

    /// Create a new loop detector with custom configuration
    pub fn with_config(config: LoopDetectorConfig) -> Self {
        Self {
            recent_calls: VecDeque::with_capacity(config.max_history),
            config,
            consecutive_failures: 0,
            last_error: None,
            error_history: Vec::new(),
            error_type_counts: HashMap::new(),
        }
    }

    /// Record build errors and check if we're stuck on the same error type
    /// Returns an action if we detect a same-error loop
    pub fn record_build_errors(&mut self, build_output: &str) -> Option<DoomLoopAction> {
        let patterns = ErrorPattern::extract_from_output(build_output);

        if patterns.is_empty() {
            // No errors - clear error tracking
            self.error_history.clear();
            self.error_type_counts.clear();
            return None;
        }

        // Check each error pattern
        for pattern in &patterns {
            // Create a key for this error type
            let key = if let Some(code) = &pattern.code {
                code.clone()
            } else if let Some(target) = &pattern.target {
                format!("{}:{}", pattern.category, target)
            } else {
                pattern.category.clone()
            };

            // Increment count
            let count = self.error_type_counts.entry(key.clone()).or_insert(0);
            *count += 1;

            // Check if we've seen this error type too many times
            if *count >= 3 {
                return Some(DoomLoopAction::AskUser {
                    message: format!(
                        "Same error type '{}' has occurred {} times. The fix attempts aren't working.\n\
                         Error: {}\n\n\
                         The AI should try a different approach. Continue anyway?",
                        key, count, pattern.message
                    ),
                });
            }
        }

        // Store in history
        self.error_history.extend(patterns);

        // Keep history bounded
        if self.error_history.len() > 20 {
            self.error_history.drain(0..10);
        }

        None
    }

    /// Clear error tracking (call when build succeeds)
    pub fn clear_error_tracking(&mut self) {
        self.error_history.clear();
        self.error_type_counts.clear();
    }

    /// Get a summary of repeated errors for display
    pub fn get_error_summary(&self) -> Option<String> {
        let repeated: Vec<_> = self.error_type_counts
            .iter()
            .filter(|(_, count)| **count >= 2)
            .collect();

        if repeated.is_empty() {
            return None;
        }

        let summary: Vec<String> = repeated
            .iter()
            .map(|(key, count)| format!("{}: {}x", key, count))
            .collect();

        Some(format!("Repeated errors: {}", summary.join(", ")))
    }

    /// Check if a tool call would create a doom loop
    /// Call this BEFORE executing the tool
    pub fn check(&mut self, tool_name: &str, params: &Value) -> DoomLoopAction {
        let call = ToolCall::new(tool_name, params);

        // Count identical calls in recent history
        let identical_count = self.recent_calls.iter().filter(|c| *c == &call).count();

        // Determine action based on thresholds
        if identical_count >= self.config.block_threshold {
            return DoomLoopAction::Block {
                message: format!(
                    "🛑 Doom loop detected: '{}' called {} times with identical parameters. \
                     This action has been blocked to prevent infinite loops. \
                     Please try a different approach.",
                    tool_name,
                    identical_count + 1
                ),
            };
        }

        if identical_count >= self.config.ask_threshold {
            return DoomLoopAction::AskUser {
                message: format!(
                    "⚠️ Tool '{}' has been called {} times with the same parameters. \
                     This may indicate a loop. Continue anyway?",
                    tool_name,
                    identical_count + 1
                ),
            };
        }

        if identical_count >= self.config.warn_threshold {
            return DoomLoopAction::Warn {
                message: format!(
                    "⚡ Note: '{}' called {} times with same parameters.",
                    tool_name,
                    identical_count + 1
                ),
            };
        }

        DoomLoopAction::Continue
    }

    /// Record a tool call after it's been executed
    /// Call this AFTER executing the tool
    pub fn record(&mut self, tool_name: &str, params: &Value) {
        let call = ToolCall::new(tool_name, params);

        self.recent_calls.push_back(call);
        if self.recent_calls.len() > self.config.max_history {
            self.recent_calls.pop_front();
        }
    }

    /// Record a tool failure
    pub fn record_failure(&mut self, error: &str) {
        self.consecutive_failures += 1;
        self.last_error = Some(error.to_string());
    }

    /// Record a tool success (resets failure counter)
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_error = None;
    }

    /// Check if we're in a failure loop
    pub fn check_failure_loop(&self) -> Option<DoomLoopAction> {
        if self.consecutive_failures >= 3 {
            Some(DoomLoopAction::AskUser {
                message: format!(
                    "⚠️ {} consecutive tool failures detected. The AI may be stuck. \
                     Last error: {}. Continue?",
                    self.consecutive_failures,
                    self.last_error.as_deref().unwrap_or("Unknown")
                ),
            })
        } else {
            None
        }
    }

    /// Reset the detector (e.g., after user intervention)
    pub fn reset(&mut self) {
        self.recent_calls.clear();
        self.consecutive_failures = 0;
        self.last_error = None;
        self.error_history.clear();
        self.error_type_counts.clear();
    }

    /// Get current loop status for display
    pub fn status(&self) -> String {
        if self.consecutive_failures > 0 {
            format!(
                "Failures: {}, History: {}",
                self.consecutive_failures,
                self.recent_calls.len()
            )
        } else {
            format!("History: {}", self.recent_calls.len())
        }
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_no_loop_on_first_call() {
        let mut detector = LoopDetector::new();
        let action = detector.check("read_file", &json!({"path": "test.rs"}));
        assert!(matches!(action, DoomLoopAction::Continue));
    }

    #[test]
    fn test_no_loop_on_different_params() {
        let mut detector = LoopDetector::new();

        detector.record("read_file", &json!({"path": "test1.rs"}));
        detector.record("read_file", &json!({"path": "test2.rs"}));
        detector.record("read_file", &json!({"path": "test3.rs"}));

        let action = detector.check("read_file", &json!({"path": "test4.rs"}));
        assert!(matches!(action, DoomLoopAction::Continue));
    }

    #[test]
    fn test_warn_on_repeated_calls() {
        let mut detector = LoopDetector::new();
        let params = json!({"path": "test.rs"});

        detector.record("read_file", &params);
        detector.record("read_file", &params);

        let action = detector.check("read_file", &params);
        assert!(matches!(action, DoomLoopAction::Warn { .. }));
    }

    #[test]
    fn test_ask_on_more_repeats() {
        let mut detector = LoopDetector::new();
        let params = json!({"path": "test.rs"});

        detector.record("read_file", &params);
        detector.record("read_file", &params);
        detector.record("read_file", &params);

        let action = detector.check("read_file", &params);
        assert!(matches!(action, DoomLoopAction::AskUser { .. }));
    }

    #[test]
    fn test_block_on_many_repeats() {
        let mut detector = LoopDetector::new();
        let params = json!({"path": "test.rs"});

        for _ in 0..5 {
            detector.record("read_file", &params);
        }

        let action = detector.check("read_file", &params);
        assert!(matches!(action, DoomLoopAction::Block { .. }));
    }

    #[test]
    fn test_failure_loop_detection() {
        let mut detector = LoopDetector::new();

        detector.record_failure("Error 1");
        detector.record_failure("Error 2");
        assert!(detector.check_failure_loop().is_none());

        detector.record_failure("Error 3");
        assert!(detector.check_failure_loop().is_some());
    }

    #[test]
    fn test_success_resets_failures() {
        let mut detector = LoopDetector::new();

        detector.record_failure("Error 1");
        detector.record_failure("Error 2");
        detector.record_success();

        assert_eq!(detector.consecutive_failures, 0);
        assert!(detector.last_error.is_none());
    }

    #[test]
    fn test_reset() {
        let mut detector = LoopDetector::new();
        let params = json!({"path": "test.rs"});

        detector.record("read_file", &params);
        detector.record("read_file", &params);
        detector.record_failure("Error");

        detector.reset();

        assert!(detector.recent_calls.is_empty());
        assert_eq!(detector.consecutive_failures, 0);
    }
}
