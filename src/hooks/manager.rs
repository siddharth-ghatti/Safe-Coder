//! Hook Manager
//!
//! Manages registration and execution of hooks.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::builtin::{CommentCheckerHook, ContextMonitorHook, EditValidatorHook, TodoEnforcerHook};
use super::types::{Hook, HookContext, HookResult, HookType};

/// Manages lifecycle hooks
pub struct HookManager {
    /// Registered hooks by type
    hooks: RwLock<HashMap<HookType, Vec<Arc<dyn Hook>>>>,
    /// Disabled hook names
    disabled: RwLock<Vec<String>>,
}

impl HookManager {
    /// Create a new hook manager with no hooks
    pub fn new() -> Self {
        Self {
            hooks: RwLock::new(HashMap::new()),
            disabled: RwLock::new(Vec::new()),
        }
    }

    /// Create a new hook manager with built-in hooks registered
    pub fn with_builtins() -> Self {
        let manager = Self::new();

        // We'll register in a blocking way since this is typically called at startup
        let hooks: Vec<Arc<dyn Hook>> = vec![
            Arc::new(CommentCheckerHook::new()),
            Arc::new(ContextMonitorHook::new()),
            Arc::new(TodoEnforcerHook::new()),
            Arc::new(EditValidatorHook::new()),
        ];

        // Pre-populate the hooks map
        let mut hooks_map: HashMap<HookType, Vec<Arc<dyn Hook>>> = HashMap::new();

        for hook in hooks {
            for hook_type in hook.hook_types() {
                hooks_map.entry(*hook_type).or_default().push(hook.clone());
            }
        }

        Self {
            hooks: RwLock::new(hooks_map),
            disabled: RwLock::new(Vec::new()),
        }
    }

    /// Register a hook
    pub async fn register(&self, hook: Arc<dyn Hook>) {
        let mut hooks = self.hooks.write().await;
        for hook_type in hook.hook_types() {
            hooks.entry(*hook_type).or_default().push(hook.clone());
        }
    }

    /// Disable a hook by name
    pub async fn disable(&self, name: &str) {
        let mut disabled = self.disabled.write().await;
        if !disabled.contains(&name.to_string()) {
            disabled.push(name.to_string());
        }
    }

    /// Enable a hook by name
    pub async fn enable(&self, name: &str) {
        let mut disabled = self.disabled.write().await;
        disabled.retain(|n| n != name);
    }

    /// Check if a hook is disabled
    pub async fn is_disabled(&self, name: &str) -> bool {
        let disabled = self.disabled.read().await;
        disabled.contains(&name.to_string())
    }

    /// Execute all hooks for a given type
    /// Returns the combined result (most restrictive wins)
    pub async fn execute(&self, ctx: &HookContext) -> HookResult {
        let hooks = self.hooks.read().await;
        let disabled = self.disabled.read().await;

        let type_hooks = match hooks.get(&ctx.hook_type) {
            Some(h) => h,
            None => return HookResult::Continue,
        };

        let mut warnings = Vec::new();
        let mut final_result = HookResult::Continue;

        for hook in type_hooks {
            // Skip disabled hooks
            if disabled.contains(&hook.name().to_string()) {
                continue;
            }

            // Skip hooks that report themselves as disabled
            if !hook.is_enabled() {
                continue;
            }

            let result = hook.execute(ctx).await;

            match &result {
                HookResult::Continue => {
                    // Continue checking other hooks
                }
                HookResult::ContinueWithWarning(msg) => {
                    warnings.push(format!("[{}] {}", hook.name(), msg));
                }
                HookResult::Skip(msg) => {
                    // Skip is less severe than Block, but still stops execution
                    if !matches!(final_result, HookResult::Block(_)) {
                        final_result = HookResult::Skip(format!("[{}] {}", hook.name(), msg));
                    }
                }
                HookResult::Block(msg) => {
                    // Block is the most severe - stop immediately
                    return HookResult::Block(format!("[{}] {}", hook.name(), msg));
                }
                HookResult::Modify { content, message } => {
                    // Modification - pass through with message
                    if let Some(msg) = message {
                        warnings.push(format!("[{}] {}", hook.name(), msg));
                    }
                    final_result = HookResult::Modify {
                        content: content.clone(),
                        message: Some(warnings.join("\n")),
                    };
                }
            }
        }

        // If we collected warnings but no blocking result, return them
        if !warnings.is_empty() && matches!(final_result, HookResult::Continue) {
            return HookResult::ContinueWithWarning(warnings.join("\n"));
        }

        final_result
    }

    /// Execute hooks and get just warnings (for non-blocking scenarios)
    pub async fn execute_and_collect_warnings(&self, ctx: &HookContext) -> Vec<String> {
        let result = self.execute(ctx).await;
        match result {
            HookResult::ContinueWithWarning(msg) => msg.lines().map(|s| s.to_string()).collect(),
            HookResult::Skip(msg) | HookResult::Block(msg) => vec![msg],
            HookResult::Modify { message, .. } => message.map(|m| vec![m]).unwrap_or_default(),
            HookResult::Continue => Vec::new(),
        }
    }

    /// List all registered hooks
    pub async fn list_hooks(&self) -> Vec<(String, Vec<HookType>, bool)> {
        let hooks = self.hooks.read().await;
        let disabled = self.disabled.read().await;

        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for hooks_list in hooks.values() {
            for hook in hooks_list {
                let name = hook.name().to_string();
                if seen.insert(name.clone()) {
                    let is_enabled = !disabled.contains(&name);
                    result.push((name, hook.hook_types().to_vec(), is_enabled));
                }
            }
        }

        result
    }

    /// Get description of a hook by name
    pub async fn get_hook_description(&self, name: &str) -> Option<String> {
        let hooks = self.hooks.read().await;

        for hooks_list in hooks.values() {
            for hook in hooks_list {
                if hook.name() == name {
                    return Some(hook.description().to_string());
                }
            }
        }

        None
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hook_manager_execution() {
        let manager = HookManager::with_builtins();

        // Test with a file that has too many comments
        let content = (0..20).map(|_| "// comment\n").collect::<String>() + "fn main() {}";
        let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(&content));

        let result = manager.execute(&ctx).await;
        assert!(matches!(result, HookResult::ContinueWithWarning(_)));
    }

    #[tokio::test]
    async fn test_disable_hook() {
        let manager = HookManager::with_builtins();

        // Disable comment checker
        manager.disable("comment_checker").await;

        // Now the same file should pass
        let content = (0..20).map(|_| "// comment\n").collect::<String>() + "fn main() {}";
        let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(&content));

        let result = manager.execute(&ctx).await;
        // Should still get edit_validator warnings but not comment_checker
        let warnings = manager.execute_and_collect_warnings(&ctx).await;
        assert!(!warnings.iter().any(|w| w.contains("comment")));
    }

    #[tokio::test]
    async fn test_list_hooks() {
        let manager = HookManager::with_builtins();
        let hooks = manager.list_hooks().await;

        assert!(hooks.len() >= 3);
        assert!(hooks.iter().any(|(name, _, _)| name == "comment_checker"));
        assert!(hooks.iter().any(|(name, _, _)| name == "context_monitor"));
        assert!(hooks.iter().any(|(name, _, _)| name == "todo_enforcer"));
    }
}
