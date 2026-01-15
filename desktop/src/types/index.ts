// Session types
export interface Session {
  id: string;
  project_path: string;
  created_at: string;
  mode: string;
}

export interface SessionSummary {
  id: string;
  project_path: string;
  created_at: string;
  message_count: number;
  file_changes: FileChangeStats;
}

export interface FileChangeStats {
  total_files: number;
  additions: number;
  deletions: number;
}

// Message types
export interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: string;
  tool_calls?: ToolCall[];
  // For displaying tool executions in chat history
  toolExecutions?: ToolExecution[];
}

export interface ToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
  output?: string;
}

// File change types
export interface FileChange {
  path: string;
  change_type: "created" | "modified" | "deleted";
  additions: number;
  deletions: number;
  timestamp: string;
  diff?: string;
}

// Server event types
export type ServerEvent =
  | { type: "Connected" }
  | { type: "Thinking"; message: string }
  | { type: "Reasoning"; text: string }
  | { type: "ToolStart"; name: string; description: string }
  | { type: "ToolOutput"; name: string; output: string }
  | { type: "BashOutputLine"; name: string; line: string }
  | { type: "ToolComplete"; name: string; success: boolean }
  | { type: "FileDiff"; path: string; additions: number; deletions: number; diff: string }
  | { type: "DiagnosticUpdate"; errors: number; warnings: number }
  | { type: "TextChunk"; text: string }
  | { type: "SubagentStarted"; id: string; kind: string; task: string }
  | { type: "SubagentProgress"; id: string; message: string }
  | { type: "SubagentCompleted"; id: string; success: boolean; summary: string }
  | { type: "PlanCreated"; title: string; steps: PlanStep[] }
  | { type: "PlanStepStarted"; plan_id: string; step_id: string }
  | { type: "PlanStepCompleted"; plan_id: string; step_id: string; success: boolean }
  | { type: "PlanAwaitingApproval"; plan_id: string }
  | { type: "PlanApproved"; plan_id: string }
  | { type: "PlanRejected"; plan_id: string }
  | { type: "TokenUsage"; input_tokens: number; output_tokens: number; cache_read_tokens?: number; cache_creation_tokens?: number }
  | { type: "ContextCompressed"; tokens_compressed: number }
  | { type: "DoomLoopPrompt"; prompt_id: string; message: string }
  | { type: "Error"; message: string }
  | { type: "Completed" }
  | { type: "TodoList"; todos: TodoItem[] };

export interface TodoItem {
  content: string;
  status: string;
  active_form: string;
  priority: number;
}

export interface PlanStep {
  id: string;
  description: string;
  status: string;
}

// Config types
export interface Config {
  provider: string;
  model: string;
  mode: string;
}

// UI state types
export type AgentMode = "plan" | "build";

export interface ToolExecution {
  id: string;
  name: string;
  description: string;
  output: string;
  success?: boolean;
  startTime: number;
  endTime?: number;
  // Input parameters for the tool (for showing file content, etc.)
  input?: Record<string, unknown>;
  // Reasoning text that appeared before this tool call
  reasoning?: string;
}

export interface StreamingMessage {
  role: "assistant";
  content: string;
  isStreaming: boolean;
  toolExecutions: ToolExecution[];
  // Buffer for reasoning that hasn't been attached to a tool yet
  pendingReasoning?: string;
}

export interface DoomLoopPrompt {
  id: string;
  message: string;
}
