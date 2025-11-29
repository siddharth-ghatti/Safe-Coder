//! High-level planner that breaks down user requests into tasks

use anyhow::Result;
use uuid::Uuid;

use crate::orchestrator::{Task, TaskPlan, TaskStatus, WorkerKind};

/// The planner analyzes user requests and creates execution plans
pub struct Planner {
    /// Template for task decomposition (could be enhanced with LLM in future)
    _decomposition_strategy: DecompositionStrategy,
}

/// Strategy for decomposing tasks
#[derive(Debug, Clone, Default)]
enum DecompositionStrategy {
    /// Simple heuristic-based decomposition
    #[default]
    Heuristic,
    // Future: LlmAssisted - use LLM for planning
}

impl Planner {
    /// Create a new planner
    pub fn new() -> Self {
        Self {
            _decomposition_strategy: DecompositionStrategy::default(),
        }
    }
    
    /// Create an execution plan from a user request
    pub async fn create_plan(&self, request: &str) -> Result<TaskPlan> {
        let plan_id = Uuid::new_v4().to_string();
        
        // Analyze the request and decompose into tasks
        let tasks = self.decompose_request(request)?;
        
        // Create the plan
        let summary = self.generate_summary(request, &tasks);
        let mut plan = TaskPlan::new(plan_id, request.to_string(), summary);
        
        for task in tasks {
            plan.add_task(task);
        }
        
        Ok(plan)
    }
    
    /// Decompose a request into individual tasks
    fn decompose_request(&self, request: &str) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
        let request_lower = request.to_lowercase();
        
        // Parse the request to identify distinct work items
        // This is a heuristic approach - could be enhanced with LLM
        
        // Check for common patterns that indicate multiple tasks
        let parts = self.split_into_parts(request);
        
        for (i, part) in parts.iter().enumerate() {
            let task_id = format!("task-{}", i + 1);
            let task = self.create_task_from_part(&task_id, part, &request_lower)?;
            tasks.push(task);
        }
        
        // If no tasks were created, create a single task for the whole request
        if tasks.is_empty() {
            let task = Task::new(
                "task-1".to_string(),
                "Execute user request".to_string(),
                request.to_string(),
            );
            tasks.push(task);
        }
        
        Ok(tasks)
    }
    
    /// Split a request into logical parts
    fn split_into_parts(&self, request: &str) -> Vec<String> {
        let mut parts = Vec::new();
        
        // Split by common separators
        let separators = [
            " and then ",
            " after that ",
            " next ",
            " also ",
            " additionally ",
            ". Then ",
            ". Also ",
            ". Next ",
            "\n- ",
            "\n* ",
            "\n1. ",
            "\n2. ",
            "\n3. ",
        ];
        
        let mut remaining = request.to_string();
        
        for sep in &separators {
            if remaining.contains(sep) {
                let split: Vec<&str> = remaining.split(sep).collect();
                if split.len() > 1 {
                    parts.push(split[0].trim().to_string());
                    remaining = split[1..].join(sep).trim().to_string();
                }
            }
        }
        
        // Add the remaining part
        if !remaining.is_empty() {
            parts.push(remaining);
        }
        
        // If only one part and it's the same as input, return empty to signal single task
        if parts.len() == 1 && parts[0] == request {
            return vec![request.to_string()];
        }
        
        parts
    }
    
    /// Create a task from a part of the request
    fn create_task_from_part(&self, id: &str, part: &str, full_request: &str) -> Result<Task> {
        let description = self.extract_description(part);
        let relevant_files = self.extract_relevant_files(part);
        let preferred_worker = self.suggest_worker(part, full_request);
        
        let mut task = Task::new(
            id.to_string(),
            description,
            part.to_string(),
        );
        
        task.relevant_files = relevant_files;
        task.preferred_worker = preferred_worker;
        task.status = TaskStatus::Pending;
        
        Ok(task)
    }
    
    /// Extract a short description from the task text
    fn extract_description(&self, part: &str) -> String {
        // Take first sentence or first 100 chars
        let first_sentence = part.split(['.', '!', '?'])
            .next()
            .unwrap_or(part);
        
        if first_sentence.len() > 100 {
            format!("{}...", &first_sentence[..97])
        } else {
            first_sentence.to_string()
        }
    }
    
    /// Extract file paths mentioned in the text
    fn extract_relevant_files(&self, part: &str) -> Vec<String> {
        let mut files = Vec::new();
        
        // Look for common file patterns
        for word in part.split_whitespace() {
            let word = word.trim_matches(|c| c == ',' || c == '.' || c == '\'' || c == '"' || c == '`');
            
            // Check if it looks like a file path
            if word.contains('/') || word.contains('.') {
                // Check for common extensions
                let extensions = [
                    ".rs", ".py", ".js", ".ts", ".go", ".java", ".cpp", ".c", ".h",
                    ".json", ".yaml", ".yml", ".toml", ".md", ".txt", ".html", ".css",
                ];
                
                for ext in &extensions {
                    if word.ends_with(ext) {
                        files.push(word.to_string());
                        break;
                    }
                }
                
                // Also check for directory-like patterns
                if word.starts_with("src/") || word.starts_with("./") || word.starts_with("../") {
                    if !files.contains(&word.to_string()) {
                        files.push(word.to_string());
                    }
                }
            }
        }
        
        files
    }
    
    /// Suggest which worker might be best for this task
    fn suggest_worker(&self, part: &str, _full_request: &str) -> Option<WorkerKind> {
        let part_lower = part.to_lowercase();
        
        // Use Claude Code for complex reasoning tasks
        if part_lower.contains("refactor") 
            || part_lower.contains("explain")
            || part_lower.contains("analyze")
            || part_lower.contains("review")
            || part_lower.contains("complex")
        {
            return Some(WorkerKind::ClaudeCode);
        }
        
        // Use Gemini for quick fixes and simple tasks
        if part_lower.contains("fix")
            || part_lower.contains("simple")
            || part_lower.contains("quick")
            || part_lower.contains("typo")
        {
            return Some(WorkerKind::GeminiCli);
        }
        
        // Default: no preference, let orchestrator decide
        None
    }
    
    /// Generate a summary of the plan
    fn generate_summary(&self, request: &str, tasks: &[Task]) -> String {
        let task_count = tasks.len();
        let files: Vec<_> = tasks.iter()
            .flat_map(|t| &t.relevant_files)
            .collect();
        
        let mut summary = format!(
            "Plan to address: \"{}\"\n\n\
             Breaking down into {} task(s):\n",
            if request.len() > 100 { format!("{}...", &request[..97]) } else { request.to_string() },
            task_count
        );
        
        for (i, task) in tasks.iter().enumerate() {
            summary.push_str(&format!(
                "  {}. {}\n",
                i + 1,
                task.description
            ));
        }
        
        if !files.is_empty() {
            summary.push_str(&format!(
                "\nRelevant files: {:?}\n",
                files
            ));
        }
        
        summary
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_simple_request_creates_single_task() {
        let planner = Planner::new();
        let plan = planner.create_plan("Add a hello world function").await.unwrap();
        
        assert_eq!(plan.tasks.len(), 1);
        assert!(plan.tasks[0].description.contains("hello world"));
    }
    
    #[tokio::test]
    async fn test_compound_request_creates_multiple_tasks() {
        let planner = Planner::new();
        let plan = planner.create_plan("First add tests and then refactor the code").await.unwrap();
        
        assert!(plan.tasks.len() >= 2);
    }
    
    #[tokio::test]
    async fn test_extracts_file_paths() {
        let planner = Planner::new();
        let plan = planner.create_plan("Edit src/main.rs to add logging").await.unwrap();
        
        assert!(plan.tasks[0].relevant_files.contains(&"src/main.rs".to_string()));
    }
}
