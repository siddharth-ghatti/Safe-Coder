//! Hooks System
//!
//! Provides lifecycle hooks for customizing Safe-Coder behavior.
//! Hooks can be triggered at various points in the execution flow,
//! allowing users to inject custom logic, validation, or transformations.

pub mod builtin;
pub mod manager;
pub mod types;

pub use builtin::{CommentCheckerHook, ContextMonitorHook, TodoEnforcerHook};
pub use manager::HookManager;
pub use types::{Hook, HookContext, HookResult, HookType};
