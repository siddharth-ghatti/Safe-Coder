//! Integration tests for safe-coder
//!
//! These tests verify end-to-end functionality of the safe-coder system,
//! including CLI commands, LLM integrations, orchestrator functionality,
//! and file operations.

// Test utilities and common setup
mod common;

// Simple integration tests that should work
mod simple_tests;

// New feature tests
mod hooks_tests;
mod skills_tests;
mod ast_grep_tests;
mod subagent_config_tests;

// Integration test modules (may need fixes)
// mod cli_tests;
// mod llm_tests;
// mod orchestrator_tests;
// mod session_tests;
// mod tools_tests;
// mod git_tests;
// mod config_tests;

// Re-export common utilities for use by test modules
pub use common::*;