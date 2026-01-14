// API client for safe-coder server

const DEFAULT_BASE_URL = "http://127.0.0.1:9876";

let baseUrl = DEFAULT_BASE_URL;

export function setBaseUrl(url: string) {
  baseUrl = url;
}

export function getBaseUrl() {
  return baseUrl;
}

// Generic fetch wrapper
async function apiFetch<T>(
  endpoint: string,
  options: RequestInit = {}
): Promise<T> {
  const url = `${baseUrl}${endpoint}`;
  const response = await fetch(url, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: "Unknown error" }));
    throw new Error(error.error || `HTTP ${response.status}`);
  }

  return response.json();
}

// Health check
export async function checkHealth(): Promise<{ status: string; version: string }> {
  return apiFetch("/api/health");
}

// Config
export async function getConfig(): Promise<{ provider: string; model: string; mode: string }> {
  return apiFetch("/api/config");
}

// Sessions
export async function listSessions(): Promise<{ sessions: Array<{
  id: string;
  project_path: string;
  created_at: string;
  message_count: number;
  file_changes: { total_files: number; additions: number; deletions: number };
}> }> {
  return apiFetch("/api/sessions");
}

export async function createSession(projectPath: string, mode?: string): Promise<{
  id: string;
  project_path: string;
  created_at: string;
  mode: string;
}> {
  return apiFetch("/api/sessions", {
    method: "POST",
    body: JSON.stringify({ project_path: projectPath, mode }),
  });
}

export async function getSession(sessionId: string): Promise<{
  id: string;
  project_path: string;
  created_at: string;
  mode: string;
}> {
  return apiFetch(`/api/sessions/${sessionId}`);
}

export async function deleteSession(sessionId: string): Promise<void> {
  await fetch(`${baseUrl}/api/sessions/${sessionId}`, { method: "DELETE" });
}

export async function setSessionMode(sessionId: string, mode: string): Promise<{ status: string; mode: string }> {
  return apiFetch(`/api/sessions/${sessionId}/mode`, {
    method: "PUT",
    body: JSON.stringify({ mode }),
  });
}

// Messages
export async function getMessages(sessionId: string): Promise<Array<{
  id: string;
  role: string;
  content: string;
  timestamp: string;
  tool_calls?: Array<{ id: string; name: string; input: unknown; output?: string }>;
}>> {
  return apiFetch(`/api/sessions/${sessionId}/messages`);
}

export async function sendMessage(
  sessionId: string,
  content: string,
  attachments: Array<{ path: string; content?: string }> = []
): Promise<{ status: string }> {
  return apiFetch(`/api/sessions/${sessionId}/messages`, {
    method: "POST",
    body: JSON.stringify({ content, attachments }),
  });
}

export async function cancelOperation(sessionId: string): Promise<{ status: string }> {
  return apiFetch(`/api/sessions/${sessionId}/cancel`, { method: "POST" });
}

// Project files (for @ mentions)
export async function listProjectFiles(
  sessionId: string,
  query?: string,
  limit: number = 50
): Promise<{ files: Array<{ path: string; name: string; is_dir: boolean }> }> {
  const params = new URLSearchParams();
  if (query) params.set("query", query);
  params.set("limit", limit.toString());
  return apiFetch(`/api/sessions/${sessionId}/files?${params.toString()}`);
}

// File changes
export async function getSessionChanges(sessionId: string): Promise<{
  session_id: string;
  changes: Array<{
    path: string;
    change_type: string;
    additions: number;
    deletions: number;
    timestamp: string;
    diff?: string;
  }>;
  stats: { total_files: number; additions: number; deletions: number };
}> {
  return apiFetch(`/api/sessions/${sessionId}/changes`);
}

// SSE event stream
export function subscribeToEvents(
  sessionId: string,
  onEvent: (event: { type: string; data: unknown }) => void,
  onError?: (error: Error) => void
): () => void {
  const url = `${baseUrl}/api/sessions/${sessionId}/events`;
  const eventSource = new EventSource(url);

  // Handle different event types
  const eventTypes = [
    "Connected",
    "Thinking",
    "Reasoning",
    "ToolStart",
    "ToolOutput",
    "BashOutputLine",
    "ToolComplete",
    "FileDiff",
    "DiagnosticUpdate",
    "TextChunk",
    "SubagentStarted",
    "SubagentProgress",
    "SubagentCompleted",
    "PlanCreated",
    "PlanStepStarted",
    "PlanStepCompleted",
    "PlanAwaitingApproval",
    "PlanApproved",
    "PlanRejected",
    "TokenUsage",
    "ContextCompressed",
    "Error",
    "Completed",
  ];

  eventTypes.forEach((type) => {
    eventSource.addEventListener(type, (event) => {
      try {
        const data = JSON.parse(event.data);
        console.log(`SSE event received: ${type}`, data);
        onEvent({ type, data });
      } catch {
        console.log(`SSE event received (raw): ${type}`, event.data);
        onEvent({ type, data: event.data });
      }
    });
  });

  // Log connection state
  eventSource.onopen = () => {
    console.log("SSE connection opened");
  };

  eventSource.onerror = (event) => {
    console.error("SSE error:", event);
    onError?.(new Error("SSE connection error"));
  };

  // Return cleanup function
  return () => {
    eventSource.close();
  };
}
