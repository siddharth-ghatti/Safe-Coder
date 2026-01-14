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
} from "../types";
import * as api from "../api/client";

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
    try {
      const session = await api.getSession(sessionId);
      set({
        activeSessionId: sessionId,
        activeSession: session,
        messages: [],
        fileChanges: [],
        streamingMessage: null,
        thinkingMessage: null,
        tokenUsage: {
          inputTokens: 0,
          outputTokens: 0,
          cacheReadTokens: 0,
          cacheCreationTokens: 0,
        },
      });
      await get().loadMessages();
      await get().loadFileChanges();
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
      },
    }));

    try {
      console.log("Sending message to session:", sessionId);
      await api.sendMessage(sessionId, content);
      console.log("Message sent, waiting for events...");

      // Set a timeout to reset processing state if no Completed event received
      // This is a safety fallback in case SSE drops the event
      setTimeout(() => {
        const state = get();
        if (state.isProcessing && state.activeSessionId === sessionId) {
          console.warn("Processing timeout - checking if still active");
          // Only reset if there's been no activity (no streaming content)
          if (state.streamingMessage && state.streamingMessage.content === "") {
            console.warn("No response received after timeout, resetting state");
            set({
              isProcessing: false,
              streamingMessage: null,
              thinkingMessage: null
            });
          }
        }
      }, 120000); // 2 minute timeout for initial response
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
        console.log(`Session mode updated to: ${mode}`);
      } catch (error) {
        console.error("Failed to update session mode:", error);
      }
    }
  },

  // Handle server events
  handleServerEvent: (event: ServerEvent) => {
    console.log("[SSE Event]", event.type, event);

    switch (event.type) {
      case "Connected":
        console.log("[SSE] Connected to server");
        set({ isConnected: true });
        break;

      case "Thinking":
        console.log("[SSE] Thinking:", event.message);
        set({ thinkingMessage: event.message });
        break;

      case "Reasoning":
        console.log("[SSE] Reasoning:", event.text);
        set({ thinkingMessage: event.text });
        break;

      case "TextChunk":
        console.log("[SSE] TextChunk received, length:", event.text?.length);
        set((state) => {
          // If no streaming message exists, create one
          const streaming = state.streamingMessage || {
            role: "assistant" as const,
            content: "",
            isStreaming: true,
            toolExecutions: [],
          };
          return {
            thinkingMessage: null,
            streamingMessage: {
              ...streaming,
              content: streaming.content + event.text,
            },
          };
        });
        break;

      case "ToolStart":
        console.log("[SSE] ToolStart:", event.name);
        set((state) => {
          // If no streaming message exists, create one
          const streaming = state.streamingMessage || {
            role: "assistant" as const,
            content: "",
            isStreaming: true,
            toolExecutions: [],
          };
          const newTool: ToolExecution = {
            id: `tool_${Date.now()}`,
            name: event.name,
            description: event.description,
            output: "",
            startTime: Date.now(),
          };
          return {
            thinkingMessage: null,
            streamingMessage: {
              ...streaming,
              toolExecutions: [...streaming.toolExecutions, newTool],
            },
          };
        });
        break;

      case "ToolOutput":
        console.log("[SSE] ToolOutput for:", event.name);
        set((state) => {
          if (!state.streamingMessage) {
            console.warn("[SSE] ToolOutput received but no streaming message!");
            return state;
          }
          const tools = state.streamingMessage.toolExecutions.map((t) =>
            t.name === event.name ? { ...t, output: t.output + event.output } : t
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
        console.log("[SSE] BashOutputLine for:", event.name);
        set((state) => {
          if (!state.streamingMessage) return state;
          const tools = state.streamingMessage.toolExecutions.map((t) =>
            t.name === event.name ? { ...t, output: t.output + event.line + "\n" } : t
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
        console.log("[SSE] ToolComplete:", event.name, "success:", event.success);
        set((state) => {
          if (!state.streamingMessage) return state;
          const tools = state.streamingMessage.toolExecutions.map((t) =>
            t.name === event.name
              ? { ...t, success: event.success, endTime: Date.now() }
              : t
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
        console.log("FileDiff event received:", event);
        set((state) => ({
          fileChanges: [
            ...state.fileChanges.filter((f) => f.path !== event.path),
            {
              path: event.path,
              change_type: event.additions > 0 && event.deletions === 0 ? "created" : "modified",
              additions: event.additions,
              deletions: event.deletions,
              timestamp: new Date().toISOString(),
              diff: event.diff,
            },
          ],
        }));
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

      case "Error":
        console.error("Server error:", event.message);
        set({ isProcessing: false, streamingMessage: null, thinkingMessage: null });
        break;

      case "Completed":
        console.log("[SSE] Completed event received");
        set((state) => {
          console.log("[SSE] Finalizing - streamingMessage:", {
            hasContent: !!state.streamingMessage?.content,
            contentLength: state.streamingMessage?.content?.length,
            toolCount: state.streamingMessage?.toolExecutions?.length,
          });

          if (!state.streamingMessage) {
            console.log("[SSE] No streaming message to finalize");
            return { thinkingMessage: null, isProcessing: false };
          }

          // Only add message if there's actual content
          if (state.streamingMessage.content || state.streamingMessage.toolExecutions.length > 0) {
            const finalMessage: Message = {
              id: `assistant_${Date.now()}`,
              role: "assistant",
              content: state.streamingMessage.content || "[Tools executed]",
              timestamp: new Date().toISOString(),
            };
            console.log("[SSE] Adding final message, total messages will be:", state.messages.length + 1);
            return {
              messages: [...state.messages, finalMessage],
              streamingMessage: null,
              thinkingMessage: null,
              isProcessing: false,
            };
          }

          console.log("[SSE] No content to finalize, just resetting state");
          return {
            streamingMessage: null,
            thinkingMessage: null,
            isProcessing: false,
          };
        });
        break;

      default:
        console.log("Unknown event type:", event.type);
    }
  },

  setIsConnected: (connected: boolean) => {
    set({ isConnected: connected });
  },

  clearStreamingMessage: () => {
    set({ streamingMessage: null });
  },
}));
