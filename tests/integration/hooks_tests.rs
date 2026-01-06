//! Integration tests for the Hooks system
//!
//! Tests the HookManager and built-in hooks functionality.

use anyhow::Result;
use serial_test::serial;
use safe_coder::hooks::{
    Hook, HookContext, HookManager, HookResult, HookType,
    CommentCheckerHook, ContextMonitorHook, TodoEnforcerHook,
};
use safe_coder::hooks::types::TokenUsageInfo;
use std::sync::Arc;

#[tokio::test]
#[serial]
async fn test_hook_manager_creation() -> Result<()> {
    let manager = HookManager::new();
    let hooks = manager.list_hooks().await;

    // New manager should have no hooks
    assert!(hooks.is_empty());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_manager_with_builtins() -> Result<()> {
    let manager = HookManager::with_builtins();
    let hooks = manager.list_hooks().await;

    // Should have built-in hooks registered
    assert!(hooks.len() >= 3);

    // Check for specific built-in hooks
    let hook_names: Vec<&String> = hooks.iter().map(|(name, _, _)| name).collect();
    assert!(hook_names.iter().any(|n| *n == "comment_checker"));
    assert!(hook_names.iter().any(|n| *n == "context_monitor"));
    assert!(hook_names.iter().any(|n| *n == "todo_enforcer"));
    assert!(hook_names.iter().any(|n| *n == "edit_validator"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_disable_enable() -> Result<()> {
    let manager = HookManager::with_builtins();

    // Disable a hook
    manager.disable("comment_checker").await;
    assert!(manager.is_disabled("comment_checker").await);

    // Re-enable the hook
    manager.enable("comment_checker").await;
    assert!(!manager.is_disabled("comment_checker").await);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_comment_checker_high_comment_ratio() -> Result<()> {
    let hook = CommentCheckerHook::new();

    // Create content with >30% comments (threshold)
    let content = r#"// Comment line 1
// Comment line 2
// Comment line 3
// Comment line 4
// Comment line 5
fn main() {
    println!("Hello");
}
// More comments
// Even more
"#;

    let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(content));
    let result = hook.execute(&ctx).await;

    // Should warn about high comment ratio
    assert!(matches!(result, HookResult::ContinueWithWarning(_)));
    if let HookResult::ContinueWithWarning(msg) = result {
        assert!(msg.contains("comment"));
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_comment_checker_acceptable_ratio() -> Result<()> {
    let hook = CommentCheckerHook::new();

    // Content with low comment ratio
    let content = r#"fn main() {
    let x = 1;
    let y = 2;
    let z = x + y;
    println!("{}", z);
    for i in 0..10 {
        println!("{}", i);
    }
    // Just one comment
    let result = compute_something();
}
"#;

    let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(content));
    let result = hook.execute(&ctx).await;

    // Should continue without warning
    assert!(matches!(result, HookResult::Continue));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_context_monitor_warning_threshold() -> Result<()> {
    let hook = ContextMonitorHook::new();

    // Create context at 75% usage (above 70% warning threshold)
    let usage = TokenUsageInfo::new(3000, 4500, 10000);
    let ctx = HookContext::new(HookType::PostResponse)
        .with_token_usage(usage);

    let result = hook.execute(&ctx).await;

    // Should warn about high usage
    assert!(matches!(result, HookResult::ContinueWithWarning(_)));
    if let HookResult::ContinueWithWarning(msg) = result {
        assert!(msg.contains("Context window"));
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_context_monitor_critical_threshold() -> Result<()> {
    let hook = ContextMonitorHook::new();

    // Create context at 90% usage (above 85% critical threshold)
    let usage = TokenUsageInfo::new(4500, 4500, 10000);
    let ctx = HookContext::new(HookType::PostResponse)
        .with_token_usage(usage);

    let result = hook.execute(&ctx).await;

    // Should show critical warning
    assert!(matches!(result, HookResult::ContinueWithWarning(_)));
    if let HookResult::ContinueWithWarning(msg) = result {
        assert!(msg.contains("CRITICAL"));
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_todo_enforcer_with_incomplete_todos() -> Result<()> {
    let hook = TodoEnforcerHook::new();

    // Create context with incomplete todos
    let ctx = HookContext::new(HookType::SessionEnd)
        .with_metadata("incomplete_todos", "3");

    let result = hook.execute(&ctx).await;

    // Should warn about incomplete todos
    assert!(matches!(result, HookResult::ContinueWithWarning(_)));
    if let HookResult::ContinueWithWarning(msg) = result {
        assert!(msg.contains("3 incomplete todo"));
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_todo_enforcer_all_complete() -> Result<()> {
    let hook = TodoEnforcerHook::new();

    // Create context with no incomplete todos
    let ctx = HookContext::new(HookType::SessionEnd)
        .with_metadata("incomplete_todos", "0");

    let result = hook.execute(&ctx).await;

    // Should continue without warning
    assert!(matches!(result, HookResult::Continue));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_manager_disabled_hooks_not_executed() -> Result<()> {
    let manager = HookManager::with_builtins();

    // Disable comment checker
    manager.disable("comment_checker").await;

    // High comment content that would normally trigger warning
    let content = (0..20).map(|_| "// comment\n").collect::<String>() + "fn main() {}";
    let ctx = HookContext::for_file(HookType::PreFileWrite, "test.rs", Some(&content));

    let warnings = manager.execute_and_collect_warnings(&ctx).await;

    // Warning should NOT contain comment_checker messages
    assert!(!warnings.iter().any(|w| w.contains("comment_checker")));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_context_for_tool() -> Result<()> {
    let ctx = HookContext::for_tool(
        HookType::PreToolUse,
        "bash",
        Some(serde_json::json!({"command": "ls -la"})),
    );

    assert_eq!(ctx.hook_type, HookType::PreToolUse);
    assert_eq!(ctx.tool_name, Some("bash".to_string()));
    assert!(ctx.tool_input.is_some());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_context_for_file() -> Result<()> {
    let ctx = HookContext::for_file(
        HookType::PreFileWrite,
        "src/main.rs",
        Some("fn main() {}"),
    );

    assert_eq!(ctx.hook_type, HookType::PreFileWrite);
    assert_eq!(ctx.file_path, Some("src/main.rs".to_string()));
    assert_eq!(ctx.file_content, Some("fn main() {}".to_string()));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_types_all() -> Result<()> {
    let all_types = HookType::all();

    // Should have all 9 hook types
    assert_eq!(all_types.len(), 9);

    // Verify specific types exist
    assert!(all_types.contains(&HookType::PreToolUse));
    assert!(all_types.contains(&HookType::PostToolUse));
    assert!(all_types.contains(&HookType::PrePrompt));
    assert!(all_types.contains(&HookType::PostResponse));
    assert!(all_types.contains(&HookType::SessionStart));
    assert!(all_types.contains(&HookType::SessionEnd));
    assert!(all_types.contains(&HookType::PreFileWrite));
    assert!(all_types.contains(&HookType::PostFileWrite));
    assert!(all_types.contains(&HookType::OnCompaction));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_result_should_continue() -> Result<()> {
    assert!(HookResult::Continue.should_continue());
    assert!(HookResult::ContinueWithWarning("test".to_string()).should_continue());
    assert!(HookResult::Modify {
        content: "test".to_string(),
        message: None
    }.should_continue());

    assert!(!HookResult::Skip("test".to_string()).should_continue());
    assert!(!HookResult::Block("test".to_string()).should_continue());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_hook_result_is_blocked() -> Result<()> {
    assert!(!HookResult::Continue.is_blocked());
    assert!(!HookResult::ContinueWithWarning("test".to_string()).is_blocked());

    assert!(HookResult::Skip("test".to_string()).is_blocked());
    assert!(HookResult::Block("test".to_string()).is_blocked());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_get_hook_description() -> Result<()> {
    let manager = HookManager::with_builtins();

    let desc = manager.get_hook_description("comment_checker").await;
    assert!(desc.is_some());
    assert!(desc.unwrap().contains("comment"));

    let desc = manager.get_hook_description("nonexistent").await;
    assert!(desc.is_none());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_token_usage_info_calculation() -> Result<()> {
    let usage = TokenUsageInfo::new(1000, 500, 10000);

    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.output_tokens, 500);
    assert_eq!(usage.total_tokens, 1500);
    assert_eq!(usage.max_tokens, 10000);
    assert!((usage.usage_percent - 15.0).abs() < 0.01);

    Ok(())
}
