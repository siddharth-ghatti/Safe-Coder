//! Subagent Module
//!
//! Provides the ability to spawn specialized subagents for focused tasks.
//! Subagents are autonomous agents that handle specific use cases like
//! code analysis, testing, refactoring, or documentation.

pub mod executor;
pub mod prompts;
pub mod tool;
pub mod types;

pub use executor::SubagentExecutor;
pub use tool::SubagentTool;
pub use types::{SubagentEvent, SubagentKind, SubagentResult, SubagentScope};
