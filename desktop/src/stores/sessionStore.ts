import { create } from "zustand";
import type {
  Session,
  SessionSummary,
  Message,
  FileChange,
  StreamingMessage,
  ToolExecution,
  AgentMode,
  ServerEvent,
  DoomLoopPrompt,
  TodoItem,
  OrchestrationTask,
} from "../types";
import * as api from "../api/client";

// Text chunk batching for smoother streaming
let textBuffer = "";
let flushTimeout: ReturnType<typeof setTimeout> | null = null;
let isFirstChunk = true;
const FLUSH_INTERVAL_MS = 16; // ~60fps for smooth updates

interface SessionState {
  // Session data
  sessions: SessionSummary[];
  activeSessionId: string | null;
  activeSession: Session | null;
  messages: Message[];
  fileChanges: FileChange[];

  // Streaming state
  streamingMessage: StreamingMessage | null;
  thinkingMessage: string | null;
  isConnected: boolean;
  isProcessing: boolean;

  // Token usage
  tokenUsage: {
    inputTokens: number;
    outputTokens: number;
    cacheReadTokens: number;
    cacheCreationTokens: number;
  };

  // Mode
  agentMode: AgentMode;

  // Doom loop
  doomLoopPrompt: DoomLoopPrompt | null;

  // Todo list
  todoList: TodoItem[];

  // Orchestration tasks
  orchestrationTasks: OrchestrationTask[];

  // Actions
  loadSessions: () => Promise<void>;
  createSession: (projectPath: string) => Promise<Session>;
  selectSession: (sessionId: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  loadMessages: () => Promise<void>;
  loadFileChanges: () => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  cancelOperation: () => Promise<void>;
  setAgentMode: (mode: AgentMode) => void;
  respondToDoomLoop: (continueAnyway: boolean) => Promise<void>;

  // Event handlers
  handleServerEvent: (event: ServerEvent) => void;
  setIsConnected: (connected: boolean) => void;
  clearStreamingMessage: () => void;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  // Initial state
  sessions: [],
  activeSessionId: null,
  activeSession: null,
  messages: [],
  fileChanges: [],
  streamingMessage: null,
  thinkingMessage: null,
  isConnected: false,
  isProcessing: false,
  tokenUsage: {
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheCreationTokens: 0,
  },
  agentMode: "build",
  doomLoopPrompt: null,
  todoList: [],
  orchestrationTasks: [],

  // Load all sessions
  loadSessions: async () => {
    try {
      const response = await api.listSessions();
      set({ sessions: response.sessions });
    } catch (error) {
      console.error("Failed to load sessions:", error);
    }
  },

  // Create a new session
  createSession: async (projectPath: string) => {
    const session = await api.createSession(projectPath, get().agentMode);
    await get().loadSessions();
    await get().selectSession(session.id);
    return session;
  },

  // Select and load a session
  selectSession: async (sessionId: string) => {
    console.log("[DEBUG] selectSession called with:", sessionId, "current:", get().activeSessionId);
    // Skip if already selected - don't clear state
    if (get().activeSessionId === sessionId) {
      console.log("[DEBUG] selectSession - already selected, skipping");
      return;
    }

    try {
      const session = await api.getSession(sessionId);
      console.log("[DEBUG] selectSession - clearing state for new session");

      // Clear state for new session
      set({
        activeSessionId: sessionId,
        activeSession: session,
        messages: [],
        fileChanges: [],
        streamingMessage: null,
        thinkingMessage: null,
        orchestrationTasks: [],
        tokenUsage: {
          inputTokens: 0,
          outputTokens: 0,
          cacheReadTokens: 0,
          cacheCreationTokens: 0,
        },
      });

      // Load messages and file changes from backend
      await get().loadMessages();
      await get().loadFileChanges();
      console.log("[DEBUG] selectSession - loaded messages:", get().messages.length);
    } catch (error) {
      console.error("Failed to select session:", error);
    }
  },

  // Delete a session
  deleteSession: async (sessionId: string) => {
    await api.deleteSession(sessionId);
    if (get().activeSessionId === sessionId) {
      set({
        activeSessionId: null,
        activeSession: null,
        messages: [],
        fileChanges: [],
      });
    }
    await get().loadSessions();
  },

  // Load messages for active session
  loadMessages: async () => {
    const sessionId = get().activeSessionId;
    if (!sessionId) return;

    try {
      const messages = await api.getMessages(sessionId);
      set({
        messages: messages.map((m) => ({
          id: m.id,
          content: m.content,
          timestamp: m.timestamp,
          role: m.role as "user" | "assistant",
          tool_calls: m.tool_calls?.map((t) => ({
            ...t,
            input: t.input as Record<string, unknown>,
          })),
        })),
      });
    } catch (error) {
      console.error("Failed to load messages:", error);
    }
  },

  // Load file changes for active session
  loadFileChanges: async () => {
    const sessionId = get().activeSessionId;
    if (!sessionId) return;

    try {
      const response = await api.getSessionChanges(sessionId);
      set({
        fileChanges: response.changes.map((c) => ({
          ...c,
          change_type: c.change_type as "created" | "modified" | "deleted",
        })),
      });
    } catch (error) {
      console.error("Failed to load file changes:", error);
    }
  },

  // Send a message
  sendMessage: async (content: string) => {
    const sessionId = get().activeSessionId;
    if (!sessionId) return;

    // Add user message to UI immediately
    const userMessage: Message = {
      id: `user_${Date.now()}`,
      role: "user",
      content,
      timestamp: new Date().toISOString(),
    };
    set((state) => ({
      messages: [...state.messages, userMessage],
      isProcessing: true,
      streamingMessage: {
        role: "assistant",
        content: "",
        isStreaming: true,
        toolExecutions: [],
        streamItems: [],
      },
    }));

    try {
      await api.sendMessage(sessionId, content);

      // Safety timeout in case SSE drops the Completed event
      setTimeout(() => {
        const state = get();
        if (state.isProcessing && state.activeSessionId === sessionId) {
          // Only reset if there's been no activity (no streaming content)
          if (state.streamingMessage && state.streamingMessage.content === "") {
            set({
              isProcessing: false,
              streamingMessage: null,
              thinkingMessage: null
            });
          }
        }
      }, 120000);
    } catch (error) {
      console.error("Failed to send message:", error);
      set({ isProcessing: false, streamingMessage: null });
    }
  },

  // Cancel current operation
  cancelOperation: async () => {
    const sessionId = get().activeSessionId;
    if (!sessionId) return;

    try {
      await api.cancelOperation(sessionId);
      set({ isProcessing: false });
    } catch (error) {
      console.error("Failed to cancel operation:", error);
    }
  },

  // Set agent mode (also updates server if session exists)
  setAgentMode: async (mode: AgentMode) => {
    set({ agentMode: mode });

    // Update the server-side session mode if there's an active session
    const sessionId = get().activeSessionId;
    if (sessionId) {
      try {
        await api.setSessionMode(sessionId, mode);
      } catch (error) {
        console.error("Failed to update session mode:", error);
      }
    }
  },

  // Respond to doom loop prompt
  respondToDoomLoop: async (continueAnyway: boolean) => {
    const sessionId = get().activeSessionId;
    const prompt = get().doomLoopPrompt;
    if (!sessionId || !prompt) return;

    try {
      await api.respondToDoomLoop(sessionId, prompt.id, continueAnyway);
      set({ doomLoopPrompt: null });
    } catch (error) {
      console.error("Failed to respond to doom loop:", error);
    }
  },

  // Handle server events
  handleServerEvent: (event: ServerEvent) => {
    const currentMessages = get().messages;
    console.log("[DEBUG] Event:", event.type, "| Messages count:", currentMessages.length, "| Streaming:", !!get().streamingMessage);
    switch (event.type) {
      case "Connected":
        set({ isConnected: true });
        break;

      case "Thinking":
        set({ thinkingMessage: event.message });
        break;

      case "Reasoning":
        // Buffer reasoning to attach to the next tool (shown before tool in ToolExecutionCard)
        // Don't add to streamItems - the tool card will display it
        if (event.text.trim()) {
          set((state) => {
            const streaming = state.streamingMessage || {
              role: "assistant" as const,
              content: "",
              isStreaming: true,
              toolExecutions: [],
              streamItems: [],
            };
            // Accumulate in pending buffer for tool attachment
            const existingReasoning = streaming.pendingReasoning || "";
            const newReasoning = existingReasoning
              ? `${existingReasoning}\n${event.text}`
              : event.text;
            return {
              thinkingMessage: event.text, // Show in thinking indicator for visual feedback
              streamingMessage: {
                ...streaming,
                pendingReasoning: newReasoning,
              },
            };
          });
        }
        break;

      case "TextChunk":
        // Batch text chunks for smoother rendering
        // Add newline if current buffer ends with punctuation and new text starts with capital
        if (textBuffer.length > 0 && event.text.length > 0) {
          const lastChar = textBuffer[textBuffer.length - 1];
          const firstChar = event.text[0];
          if (/[.!?]/.test(lastChar) && /[A-Z]/.test(firstChar)) {
            textBuffer += '\n\n'; // Add markdown paragraph break
          }
        }
        textBuffer += event.text;

        // Flush immediately on first chunk for responsiveness, then batch
        // Text chunks are accumulated in content (shown at end), not in streamItems
        if (isFirstChunk) {
          isFirstChunk = false;
          const bufferedText = textBuffer;
          textBuffer = "";
          set((state) => {
            const streaming = state.streamingMessage || {
              role: "assistant" as const,
              content: "",
              isStreaming: true,
              toolExecutions: [],
              streamItems: [],
            };
            return {
              thinkingMessage: null,
              streamingMessage: {
                ...streaming,
                content: streaming.content + bufferedText,
              },
            };
          });
        } else if (!flushTimeout) {
          // Schedule flush for subsequent chunks
          flushTimeout = setTimeout(() => {
            const bufferedText = textBuffer;
            textBuffer = "";
            flushTimeout = null;

            if (bufferedText) {
              set((state) => {
                const streaming = state.streamingMessage || {
                  role: "assistant" as const,
                  content: "",
                  isStreaming: true,
                  toolExecutions: [],
                  streamItems: [],
                };
                return {
                  thinkingMessage: null,
                  streamingMessage: {
                    ...streaming,
                    content: streaming.content + bufferedText,
                  },
                };
              });
            }
          }, FLUSH_INTERVAL_MS);
        }
        break;

      case "ToolStart":
        set((state) => {
          // If no streaming message exists, create one
          const streaming = state.streamingMessage || {
            role: "assistant" as const,
            content: "",
            isStreaming: true,
            toolExecutions: [],
            streamItems: [],
          };
          // Create unique ID for this tool
          const toolId = `tool_${Date.now()}`;
          // Attach any pending reasoning to this tool
          const newTool: ToolExecution = {
            id: toolId,
            name: event.name,
            description: event.description,
            output: "",
            startTime: Date.now(),
            reasoning: streaming.pendingReasoning || undefined,
          };
          // Add tool reference to stream items for interleaved display
          const newStreamItems = [...(streaming.streamItems || []), { type: 'tool' as const, toolId }];
          return {
            thinkingMessage: null,
            streamingMessage: {
              ...streaming,
              toolExecutions: [...streaming.toolExecutions, newTool],
              pendingReasoning: undefined, // Clear the buffer
              streamItems: newStreamItems,
            },
          };
        });
        break;

      case "ToolOutput":
        set((state) => {
          if (!state.streamingMessage) return state;
          // Find the last tool with this name (handles race conditions with ToolComplete)
          const toolIndex = state.streamingMessage.toolExecutions
            .map((t, i) => ({ t, i }))
            .filter(({ t }) => t.name === event.name)
            .pop()?.i;

          if (toolIndex === undefined) return state;

          const tools = state.streamingMessage.toolExecutions.map((t, i) =>
            i === toolIndex ? { ...t, output: t.output + event.output } : t
          );
          return {
            streamingMessage: {
              ...state.streamingMessage,
              toolExecutions: tools,
            },
          };
        });
        break;

      case "BashOutputLine":
        set((state) => {
          if (!state.streamingMessage) return state;
          // Find the last tool with this name (handles race conditions with ToolComplete)
          const toolIndex = state.streamingMessage.toolExecutions
            .map((t, i) => ({ t, i }))
            .filter(({ t }) => t.name === event.name)
            .pop()?.i;

          if (toolIndex === undefined) return state;

          const tools = state.streamingMessage.toolExecutions.map((t, i) =>
            i === toolIndex ? { ...t, output: t.output + event.line + "\n" } : t
          );
          return {
            streamingMessage: {
              ...state.streamingMessage,
              toolExecutions: tools,
            },
          };
        });
        break;

      case "ToolComplete":
        set((state) => {
          if (!state.streamingMessage) return state;
          // Find the last running tool with this name (most recent)
          const toolIndex = state.streamingMessage.toolExecutions
            .map((t, i) => ({ t, i }))
            .filter(({ t }) => t.name === event.name && t.success === undefined)
            .pop()?.i;

          if (toolIndex === undefined) return state;

          const tools = state.streamingMessage.toolExecutions.map((t, i) =>
            i === toolIndex ? { ...t, success: event.success, endTime: Date.now() } : t
          );
          return {
            streamingMessage: {
              ...state.streamingMessage,
              toolExecutions: tools,
            },
          };
        });
        break;

      case "FileDiff":
        // Type guard for FileDiff event
        if ('path' in event && typeof event.path === 'string') {
          const fileDiffEvent = event as { type: "FileDiff"; path: string; additions: number; deletions: number; diff: string };
          set((state) => ({
            fileChanges: [
              ...state.fileChanges.filter((f) => f.path !== fileDiffEvent.path),
              {
                path: fileDiffEvent.path,
                change_type: fileDiffEvent.additions > 0 && fileDiffEvent.deletions === 0 ? "created" : "modified",
                additions: fileDiffEvent.additions || 0,
                deletions: fileDiffEvent.deletions || 0,
                timestamp: new Date().toISOString(),
                diff: fileDiffEvent.diff || "",
              },
            ],
          }));
        }
        break;

      case "TokenUsage":
        // Accumulate tokens across the session
        set((state) => ({
          tokenUsage: {
            inputTokens: state.tokenUsage.inputTokens + event.input_tokens,
            outputTokens: state.tokenUsage.outputTokens + event.output_tokens,
            cacheReadTokens: state.tokenUsage.cacheReadTokens + (event.cache_read_tokens || 0),
            cacheCreationTokens: state.tokenUsage.cacheCreationTokens + (event.cache_creation_tokens || 0),
          },
        }));
        break;

      case "ContextCompressed":
        // Context was compacted to reduce token usage
        console.log("[Context] Compacted, tokens saved:", event.tokens_compressed);
        // Reset token tracking since context was compressed
        set((state) => ({
          tokenUsage: {
            ...state.tokenUsage,
            // Reduce input tokens by compressed amount (approximate)
            inputTokens: Math.max(0, state.tokenUsage.inputTokens - (event.tokens_compressed || 0)),
          },
        }));
        break;

      case "DoomLoopPrompt":
        // Show doom loop prompt to user for approval
        set({
          doomLoopPrompt: {
            id: event.prompt_id,
            message: event.message,
          },
        });
        break;

      case "TodoList":
        // Update todo list
        set({ todoList: event.todos });
        break;

      case "Error":
        // Flush pending text buffer on error
        if (flushTimeout) {
          clearTimeout(flushTimeout);
          flushTimeout = null;
        }
        textBuffer = "";
        isFirstChunk = true;
        console.error("Server error:", event.message);
        set({ isProcessing: false, streamingMessage: null, thinkingMessage: null });
        break;

      case "Completed":
        // Flush any pending text immediately
        if (flushTimeout) {
          clearTimeout(flushTimeout);
          flushTimeout = null;
        }
        const pendingText = textBuffer;
        textBuffer = "";
        isFirstChunk = true;

        set((state) => {
          console.log("[DEBUG] Completed - streamingMessage:", !!state.streamingMessage, "pendingText:", pendingText.length, "existing messages:", state.messages.length);
          if (!state.streamingMessage) {
            console.log("[DEBUG] Completed - no streaming message to finalize");
            return { thinkingMessage: null, isProcessing: false };
          }

          // Append any remaining buffered text
          const finalContent = state.streamingMessage.content + pendingText;
          console.log("[DEBUG] Completed - finalContent length:", finalContent.length, "tools:", state.streamingMessage.toolExecutions.length);

          // Only add message if there's actual content or tool executions
          if (finalContent || state.streamingMessage.toolExecutions.length > 0) {
            const finalMessage: Message = {
              id: `assistant_${Date.now()}`,
              role: "assistant",
              content: finalContent || "",
              timestamp: new Date().toISOString(),
              toolExecutions: state.streamingMessage.toolExecutions.length > 0
                ? [...state.streamingMessage.toolExecutions]
                : undefined,
            };
            console.log("[DEBUG] Completed - adding message, new total:", state.messages.length + 1);
            return {
              messages: [...state.messages, finalMessage],
              streamingMessage: null,
              thinkingMessage: null,
              isProcessing: false,
            };
          }

          console.log("[DEBUG] Completed - no content to add");
          return {
            streamingMessage: null,
            thinkingMessage: null,
            isProcessing: false,
          };
        });
        break;

      case "OrchestrateStarted":
        // Start tracking a new orchestration task
        set((state) => ({
          orchestrationTasks: [
            ...state.orchestrationTasks,
            {
              id: event.id,
              worker: event.worker,
              task: event.task,
              output: "",
              startTime: Date.now(),
            },
          ],
        }));
        break;

      case "OrchestrateOutput":
        // Append output line to the orchestration task
        set((state) => {
          const taskIndex = state.orchestrationTasks.findIndex(t => t.id === event.id);
          if (taskIndex === -1) return state;

          const tasks = [...state.orchestrationTasks];
          tasks[taskIndex] = {
            ...tasks[taskIndex],
            output: tasks[taskIndex].output + event.line + "\n",
          };
          return { orchestrationTasks: tasks };
        });
        break;

      case "OrchestrateCompleted":
        // Mark orchestration task as completed
        set((state) => {
          const taskIndex = state.orchestrationTasks.findIndex(t => t.id === event.id);
          if (taskIndex === -1) return state;

          const tasks = [...state.orchestrationTasks];
          tasks[taskIndex] = {
            ...tasks[taskIndex],
            success: event.success,
            output: tasks[taskIndex].output + (event.output || ""),
            endTime: Date.now(),
          };
          return { orchestrationTasks: tasks };
        });
        break;

      default:
        // Ignore unhandled event types
        break;
    }
  },

  setIsConnected: (connected: boolean) => {
    set({ isConnected: connected });
  },

  clearStreamingMessage: () => {
    set({ streamingMessage: null });
  },
}));
