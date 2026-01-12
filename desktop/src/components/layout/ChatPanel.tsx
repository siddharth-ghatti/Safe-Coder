import { useSessionStore } from "../../stores/sessionStore";
import { useUIStore } from "../../stores/uiStore";
import { MessageList } from "../chat/MessageList";
import { ChatInput } from "../input/ChatInput";
import { TerminalPanel } from "../terminal/TerminalPanel";
import { PanelLeft, PanelRight, Terminal } from "lucide-react";
import { cn } from "../../lib/utils";

export function ChatPanel() {
  const activeSession = useSessionStore((s) => s.activeSession);
  const isConnected = useSessionStore((s) => s.isConnected);
  const tokenUsage = useSessionStore((s) => s.tokenUsage);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);
  const toggleChangesPanel = useUIStore((s) => s.toggleChangesPanel);
  const toggleTerminal = useUIStore((s) => s.toggleTerminal);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const changesPanelCollapsed = useUIStore((s) => s.changesPanelCollapsed);
  const terminalOpen = useUIStore((s) => s.terminalOpen);

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border bg-card">
        <div className="flex items-center gap-2">
          <button
            onClick={toggleSidebar}
            className={cn(
              "p-1.5 rounded transition-colors",
              sidebarCollapsed
                ? "text-foreground bg-muted"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            )}
            title={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
          >
            <PanelLeft className="w-4 h-4" />
          </button>

          {activeSession && (
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-foreground">
                {activeSession.project_path.split("/").pop()}
              </span>
              <span
                className={cn(
                  "w-2 h-2 rounded-full",
                  isConnected ? "bg-success" : "bg-muted-foreground"
                )}
                title={isConnected ? "Connected" : "Disconnected"}
              />
            </div>
          )}
        </div>

        <div className="flex items-center gap-2">
          {/* Token usage - show total context tokens */}
          {(tokenUsage.inputTokens > 0 || tokenUsage.outputTokens > 0) && (
            <div className="text-xs text-muted-foreground" title={`Input: ${tokenUsage.inputTokens.toLocaleString()} | Output: ${tokenUsage.outputTokens.toLocaleString()}`}>
              {(tokenUsage.inputTokens + tokenUsage.outputTokens).toLocaleString()} tokens
            </div>
          )}

          <button
            onClick={toggleTerminal}
            className="p-1.5 rounded text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
            title="Toggle terminal"
          >
            <Terminal className="w-4 h-4" />
          </button>

          <button
            onClick={toggleChangesPanel}
            className={cn(
              "p-1.5 rounded transition-colors",
              changesPanelCollapsed
                ? "text-foreground bg-muted"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            )}
            title={changesPanelCollapsed ? "Show changes" : "Hide changes"}
          >
            <PanelRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Messages area */}
      <div className="flex-1 overflow-hidden">
        {activeSession ? (
          <MessageList />
        ) : (
          <div className="h-full flex items-center justify-center">
            <div className="text-center text-muted-foreground">
              <p className="text-lg font-medium mb-2">No session selected</p>
              <p className="text-sm">
                Create a new session or select an existing one to get started.
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Input area */}
      {activeSession && <ChatInput />}

      {/* Terminal panel */}
      {terminalOpen && activeSession && <TerminalPanel />}
    </div>
  );
}
