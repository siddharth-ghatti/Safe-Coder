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
          ...m,
          role: m.role as "user" | "assistant",
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
      await api.sendMessage(sessionId, content);
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

  // Set agent mode
  setAgentMode: (mode: AgentMode) => {
    set({ agentMode: mode });
  },

  // Handle server events
  handleServerEvent: (event: ServerEvent) => {
    switch (event.type) {
      case "Connected":
        set({ isConnected: true });
        break;

      case "Thinking":
        // Set thinking message separately, don't mix with content
        set({ thinkingMessage: event.message });
        break;

      case "Reasoning":
        // Reasoning is pre-tool explanation, show as thinking
        set({ thinkingMessage: event.text });
        break;

      case "TextChunk":
        // Clear thinking message when actual content arrives
        set((state) => ({
          thinkingMessage: null,
          streamingMessage: state.streamingMessage
            ? {
                ...state.streamingMessage,
                content: state.streamingMessage.content + event.text,
              }
            : null,
        }));
        break;

      case "ToolStart":
        set((state) => {
          if (!state.streamingMessage) return state;
          const newTool: ToolExecution = {
            id: `tool_${Date.now()}`,
            name: event.name,
            description: event.description,
            output: "",
            startTime: Date.now(),
          };
          return {
            thinkingMessage: null, // Clear thinking when tool starts
            streamingMessage: {
              ...state.streamingMessage,
              toolExecutions: [...state.streamingMessage.toolExecutions, newTool],
            },
          };
        });
        break;

      case "ToolOutput":
        set((state) => {
          if (!state.streamingMessage) return state;
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
        set({ isProcessing: false });
        break;

      case "Completed":
        // Finalize the streaming message
        set((state) => {
          if (!state.streamingMessage) {
            return { thinkingMessage: null, isProcessing: false };
          }
          const finalMessage: Message = {
            id: `assistant_${Date.now()}`,
            role: "assistant",
            content: state.streamingMessage.content,
            timestamp: new Date().toISOString(),
          };
          return {
            messages: [...state.messages, finalMessage],
            streamingMessage: null,
            thinkingMessage: null,
            isProcessing: false,
          };
        });
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
