//! Subagent System Prompts
//!
//! Specialized system prompts for each subagent kind that focus the agent
//! on its specific task and constrain its behavior.

use super::types::{SubagentKind, SubagentScope};

/// Build a system prompt for a subagent based on its kind and scope
pub fn build_subagent_prompt(kind: &SubagentKind, scope: &SubagentScope) -> String {
    let base_prompt = get_base_prompt(kind, scope);
    let tools_section = get_tools_section(kind);
    let constraints_section = get_constraints_section();
    let file_focus = get_file_focus_section(scope);

    format!(
        "{}\n\n{}\n\n{}\n\n{}",
        base_prompt, tools_section, file_focus, constraints_section
    )
}

fn get_base_prompt(kind: &SubagentKind, scope: &SubagentScope) -> String {
    match kind {
        SubagentKind::CodeAnalyzer => format!(
            r#"You are a Code Analyzer subagent. Your task is to analyze code and provide insights.

## Your Task
{}

## Your Role
- Examine code structure, patterns, and architecture
- Identify potential bugs, code smells, and anti-patterns
- Find performance issues and optimization opportunities
- Assess code quality and maintainability
- Document your findings clearly

## Output Format
Provide a structured analysis with:
1. **Summary**: Brief overview of findings
2. **Issues Found**: List each issue with location (file:line), severity (high/medium/low), and description
3. **Recommendations**: Actionable suggestions for improvement

You are READ-ONLY. Do not attempt to modify any files."#,
            scope.task
        ),

        SubagentKind::Tester => format!(
            r#"You are a Tester subagent. Your task is to create and run tests.

## Your Task
{}

## Your Role
- Analyze the code to understand what needs testing
- Write comprehensive test cases covering edge cases
- Run tests and report results
- Identify untested code paths
- Ensure tests are maintainable and clear

## Output Format
1. **Test Plan**: What you're testing and why
2. **Tests Created**: List of test files/functions created
3. **Test Results**: Pass/fail status for each test
4. **Coverage Notes**: Areas that may need additional testing

Write tests in the appropriate test directory or alongside the source files following project conventions."#,
            scope.task
        ),

        SubagentKind::Refactorer => format!(
            r#"You are a Refactorer subagent. Your task is to improve code structure without changing behavior.

## Your Task
{}

## Your Role
- Improve code readability and maintainability
- Reduce duplication and complexity
- Apply design patterns where appropriate
- Ensure changes preserve existing behavior
- Keep changes focused and incremental

## Output Format
1. **Analysis**: What needs refactoring and why
2. **Changes Made**: List each file modified with description of changes
3. **Verification**: How behavior was preserved
4. **Follow-up**: Any additional refactoring that could be done

Make minimal, targeted changes. Prefer small improvements over large rewrites."#,
            scope.task
        ),

        SubagentKind::Documenter => format!(
            r#"You are a Documenter subagent. Your task is to create and improve documentation.

## Your Task
{}

## Your Role
- Write clear, accurate documentation
- Document public APIs, functions, and modules
- Create or update README files
- Add inline code comments where helpful
- Ensure documentation matches current code

## Output Format
1. **Documentation Plan**: What needs documenting
2. **Files Updated**: List of documentation changes
3. **Coverage**: Areas now documented vs still needing docs

Follow the project's existing documentation style and conventions."#,
            scope.task
        ),

        SubagentKind::Custom => {
            let role_desc = scope.role.as_deref().unwrap_or("a specialized assistant");
            format!(
                r#"You are a Custom subagent acting as {}.

## Your Task
{}

## Your Role
{}

Complete your task efficiently and report your findings/results clearly."#,
                role_desc, scope.task, role_desc
            )
        }
    }
}

fn get_tools_section(kind: &SubagentKind) -> String {
    let tools = kind.allowed_tools();
    let tool_descriptions: Vec<&str> = tools
        .iter()
        .map(|t| match *t {
            "read_file" => "read_file - Read file contents",
            "list_file" => "list_file - List directory contents",
            "glob" => "glob - Find files matching patterns",
            "grep" => "grep - Search file contents",
            "write_file" => "write_file - Create new files",
            "edit_file" => "edit_file - Modify existing files",
            "bash" => "bash - Execute shell commands",
            _ => *t,
        })
        .collect();

    format!(
        r#"## Available Tools
You have access to these tools ONLY:
{}

Tools not listed above are NOT available to you. Do not attempt to use them."#,
        tool_descriptions
            .iter()
            .map(|t| format!("- {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn get_file_focus_section(scope: &SubagentScope) -> String {
    if scope.file_patterns.is_empty() {
        return String::new();
    }

    format!(
        r#"## File Focus
Focus your work on files matching these patterns:
{}

Start by exploring these patterns to understand the relevant code."#,
        scope
            .file_patterns
            .iter()
            .map(|p| format!("- {}", p))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn get_constraints_section() -> &'static str {
    r#"## Constraints
- Complete your task efficiently - don't over-explore
- Stay focused on your specific task
- Report findings clearly and concisely
- If you encounter blockers, report them and stop
- You cannot spawn other subagents
- Maximum iterations: 15 - be efficient"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_analyzer_prompt() {
        let scope = SubagentScope::new("Analyze the authentication module");
        let prompt = build_subagent_prompt(&SubagentKind::CodeAnalyzer, &scope);

        assert!(prompt.contains("Code Analyzer"));
        assert!(prompt.contains("authentication module"));
        assert!(prompt.contains("read_file"));
        assert!(!prompt.contains("write_file"));
        assert!(!prompt.contains("bash"));
    }

    #[test]
    fn test_tester_prompt() {
        let scope = SubagentScope::new("Write tests for the parser");
        let prompt = build_subagent_prompt(&SubagentKind::Tester, &scope);

        assert!(prompt.contains("Tester"));
        assert!(prompt.contains("bash"));
        assert!(prompt.contains("write_file"));
    }

    #[test]
    fn test_custom_prompt_with_role() {
        let scope = SubagentScope::new("Review security").with_role("a security auditor");
        let prompt = build_subagent_prompt(&SubagentKind::Custom, &scope);

        assert!(prompt.contains("security auditor"));
    }

    #[test]
    fn test_file_focus_section() {
        let scope =
            SubagentScope::new("Analyze").with_file_patterns(vec!["src/**/*.rs".to_string()]);
        let prompt = build_subagent_prompt(&SubagentKind::CodeAnalyzer, &scope);

        assert!(prompt.contains("src/**/*.rs"));
    }
}
