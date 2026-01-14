# Safe Coder Architecture

## Overview

Safe Coder uses a client-server architecture where the core AI functionality runs as a server, and multiple clients (TUI, Desktop app) communicate with it via HTTP REST and Server-Sent Events (SSE).

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLIENTS                                  │
├─────────────────────┬─────────────────────┬─────────────────────┤
│   Shell TUI         │   Desktop App       │   CLI (Future)      │
│   (Rust/Ratatui)    │   (React/Electron)  │                     │
└─────────┬───────────┴─────────┬───────────┴─────────────────────┘
          │                     │
          │  HTTP REST + SSE    │
          │                     │
┌─────────▼─────────────────────▼─────────────────────────────────┐
│                         SERVER                                   │
│                    (Axum HTTP Server)                            │
├─────────────────────────────────────────────────────────────────┤
│  Routes:                                                         │
│  - POST   /api/sessions              (create session)            │
│  - DELETE /api/sessions/:id          (close session)             │
│  - POST   /api/sessions/:id/messages (send message)              │
│  - PUT    /api/sessions/:id/mode     (set mode)                  │
│  - GET    /api/sessions/:id/events   (SSE stream)                │
│  - POST   /api/sessions/:id/doom-loop-response                   │
│  - GET    /health                    (health check)              │
└─────────────────────────────────────────────────────────────────┘
          │
          │
┌─────────▼─────────────────────────────────────────────────────────┐
│                         CORE                                       │
├─────────────────────────────────────────────────────────────────────┤
│  Session        │  LLM Client    │  Tools         │  Planning      │
│  - Context      │  - Anthropic   │  - Read/Write  │  - TaskPlan    │
│  - Messages     │  - OpenAI      │  - Bash        │  - Execution   │
│  - Events       │  - Copilot     │  - Grep/Glob   │  - Approval    │
└─────────────────┴────────────────┴────────────────┴────────────────┘
```

## Components

### 1. Server (`src/server/`)

The HTTP server is the central hub that manages sessions and coordinates AI operations.

**Key Files:**
- `mod.rs` - Server startup and configuration
- `routes/` - HTTP route handlers
  - `sessions.rs` - Session CRUD operations
  - `messages.rs` - Message handling and SSE
  - `files.rs` - File change tracking
- `types.rs` - DTOs and event types
- `state.rs` - Shared server state

**Starting the Server:**
```bash
safe-coder serve --port 9876
```

### 2. HTTP Client (`src/client/mod.rs`)

The Rust HTTP client used by the TUI to communicate with the server.

```rust
pub struct SafeCoderClient {
    base_url: String,
    client: Client,
    session_id: Option<String>,
}

impl SafeCoderClient {
    // Session management
    pub async fn create_session(&mut self, project_path: &str, mode: Option<&str>) -> Result<SessionResponse>;
    pub async fn close_session(&mut self) -> Result<()>;

    // Messaging
    pub async fn send_message(&self, content: &str) -> Result<()>;
    pub async fn send_message_with_attachments(&self, content: &str, attachments: Vec<AttachmentInput>) -> Result<()>;

    // Configuration
    pub async fn set_mode(&self, mode: &str) -> Result<()>;

    // Events
    pub async fn subscribe_events(&self) -> Result<mpsc::UnboundedReceiver<ServerEvent>>;

    // Doom loop
    pub async fn respond_to_doom_loop(&self, prompt_id: &str, continue_anyway: bool) -> Result<()>;
}
```

### 3. Server Manager (`src/client/mod.rs`)

Manages the server process lifecycle for the TUI.

```rust
pub struct ServerManager {
    process: Option<Child>,
    port: u16,
}

impl ServerManager {
    pub async fn ensure_running(&mut self) -> Result<()>;  // Start if not running
    pub async fn is_running(&self) -> bool;                 // Health check
    pub async fn stop(&mut self);                           // Stop server
}
```

### 4. Server Events (`src/server/types.rs`)

Events sent from server to clients via SSE:

```rust
pub enum ServerEvent {
    // Connection
    Connected,
    Completed,

    // AI Processing
    Thinking { message: String },
    Reasoning { text: String },
    TextChunk { text: String },

    // Tool Execution
    ToolStart { name: String, description: String },
    ToolOutput { name: String, output: String },
    BashOutputLine { name: String, line: String },
    ToolComplete { name: String, success: bool },

    // File Changes
    FileDiff { path: String, additions: i32, deletions: i32, diff: String },
    DiagnosticUpdate { errors: usize, warnings: usize },

    // Subagents
    SubagentStarted { id: String, kind: String, task: String },
    SubagentProgress { id: String, message: String },
    SubagentCompleted { id: String, success: bool, summary: String },

    // Planning
    PlanCreated { title: String, steps: Vec<PlanStepDto> },
    PlanStepStarted { plan_id: String, step_id: String },
    PlanStepCompleted { plan_id: String, step_id: String, success: bool },
    PlanAwaitingApproval { plan_id: String },
    PlanApproved { plan_id: String },
    PlanRejected { plan_id: String },

    // Token Usage
    TokenUsage { input_tokens: usize, output_tokens: usize, ... },
    ContextCompressed { tokens_compressed: usize },

    // User Prompts
    DoomLoopPrompt { prompt_id: String, message: String },

    // Errors
    Error { message: String },
}
```

## Data Flow

### 1. Session Creation

```
TUI                          Server                         Core
 │                              │                             │
 │──POST /api/sessions─────────▶│                             │
 │   {project_path, mode}       │                             │
 │                              │──create Session────────────▶│
 │                              │◀─────────────────────────────│
 │◀─────{id, project_path}──────│                             │
 │                              │                             │
```

### 2. Sending a Message

```
TUI                          Server                         Core
 │                              │                             │
 │──GET /api/sessions/:id/events (SSE)──────────────────────▶│
 │   (subscribe to events)      │                             │
 │                              │                             │
 │──POST /api/sessions/:id/messages───────────────────────────▶│
 │   {content}                  │                             │
 │                              │──session.send_message()────▶│
 │                              │                             │
 │◀─────SSE: Thinking──────────│◀────SessionEvent::Thinking──│
 │◀─────SSE: ToolStart─────────│◀────SessionEvent::ToolStart─│
 │◀─────SSE: ToolOutput────────│◀────SessionEvent::ToolOutput│
 │◀─────SSE: TextChunk─────────│◀────SessionEvent::TextChunk─│
 │◀─────SSE: Completed─────────│◀────────────────────────────│
 │                              │                             │
```

### 3. Doom Loop Handling

```
TUI                          Server                         Core
 │                              │                             │
 │◀─────SSE: DoomLoopPrompt────│◀──SessionEvent::DoomLoop───│
 │   {prompt_id, message}       │                             │
 │                              │                             │
 │   (user chooses continue)    │                             │
 │                              │                             │
 │──POST /api/sessions/:id/doom-loop-response────────────────▶│
 │   {prompt_id, continue: true}│                             │
 │                              │──resume processing─────────▶│
 │                              │                             │
```

## TUI Architecture (`src/tui/`)

### Shell TUI (New - HTTP-based)

**Files:**
- `shell_app.rs` - Application state (ShellTuiApp)
- `shell_runner.rs` - Event loop and command execution (ShellTuiRunner)
- `shell_ui.rs` - Rendering

**Key Components:**

```rust
pub struct ShellTuiRunner {
    app: ShellTuiApp,           // UI state
    config: Config,              // Configuration
    lsp_manager: Option<LspManager>,  // LSP servers
    server_manager: ServerManager,    // Server process
}

pub struct ShellTuiApp {
    // HTTP client (replaces direct Session)
    pub client: Option<Arc<Mutex<SafeCoderClient>>>,

    // UI state
    pub blocks: Vec<CommandBlock>,
    pub input: String,
    pub sidebar: SidebarState,

    // Doom loop state
    pub doom_loop_visible: bool,
    pub doom_loop_prompt_id: Option<String>,
    pub doom_loop_message: Option<String>,
    // ...
}
```

### Event Flow in TUI

```rust
// 1. Connect to AI
async fn connect_ai(&mut self) -> Result<()> {
    // Start server if needed
    self.server_manager.ensure_running().await?;

    // Create HTTP client and session
    let mut client = SafeCoderClient::new(self.server_manager.port());
    client.create_session(&project_path, Some("build")).await?;

    self.app.client = Some(Arc::new(Mutex::new(client)));
}

// 2. Execute AI query
async fn execute_ai_query(&mut self, input: &str, tx: mpsc::UnboundedSender<AiUpdate>) {
    if let Some(client) = &self.app.client {
        // Subscribe to SSE events
        let event_rx = client.subscribe_events().await?;

        // Send message
        client.send_message(&query).await?;

        // Forward events to UI
        forward_server_events_to_ai_updates(event_rx, block_id, ai_tx).await;
    }
}

// 3. Handle doom loop response
if self.app.has_doom_loop_prompt() {
    if let Some(prompt_id) = self.app.doom_loop_prompt_id.clone() {
        let client = self.app.client.lock().await;
        client.respond_to_doom_loop(&prompt_id, true).await?;
    }
    self.app.clear_doom_loop();
}
```

## Desktop App Architecture (`desktop/`)

The desktop app (React/Electron) follows the same pattern:

```typescript
// Create session
const response = await fetch(`${API_URL}/api/sessions`, {
  method: 'POST',
  body: JSON.stringify({ project_path, mode })
});

// Subscribe to events
const eventSource = new EventSource(`${API_URL}/api/sessions/${sessionId}/events`);
eventSource.onmessage = (event) => {
  const serverEvent = JSON.parse(event.data);
  // Handle event...
};

// Send message
await fetch(`${API_URL}/api/sessions/${sessionId}/messages`, {
  method: 'POST',
  body: JSON.stringify({ content: message })
});
```

## Configuration

### Server Port

Default port: `9876`

Can be changed via:
```bash
safe-coder serve --port 8080
```

### Session Modes

- `build` - Lightweight planning with auto-execution
- `plan` - Deep planning with approval before execution

## Error Handling

### HTTP Errors

All endpoints return standard HTTP status codes:
- `200` - Success
- `400` - Bad request
- `404` - Session not found
- `500` - Internal server error

Error response format:
```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

### SSE Errors

Errors during streaming are sent as `ServerEvent::Error`:
```json
{
  "type": "Error",
  "message": "Error description"
}
```

## Future Improvements

1. **WebSocket Support** - For bidirectional real-time communication
2. **Authentication** - API key or token-based auth for remote access
3. **Multiple Sessions** - Support multiple concurrent sessions per server
4. **Remote Server** - Connect TUI to remote servers (not just localhost)
5. **Session Persistence** - Resume sessions across restarts
