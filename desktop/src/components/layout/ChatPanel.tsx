import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "../../stores/sessionStore";
import { useUIStore } from "../../stores/uiStore";
import { MessageList } from "../chat/MessageList";
import { ChatInput } from "../input/ChatInput";
import { TerminalPanel } from "../terminal/TerminalPanel";
import { PanelLeft, PanelRight, Terminal, Loader2, Plus, ChevronDown, LayoutGrid } from "lucide-react";
import { cn } from "../../lib/utils";

export function ChatPanel() {
  const [isCreating, setIsCreating] = useState(false);
  const activeSession = useSessionStore((s) => s.activeSession);
  const createSession = useSessionStore((s) => s.createSession);
  const isConnected = useSessionStore((s) => s.isConnected);
  const isProcessing = useSessionStore((s) => s.isProcessing);
  const tokenUsage = useSessionStore((s) => s.tokenUsage);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);
  const toggleChangesPanel = useUIStore((s) => s.toggleChangesPanel);
  const toggleTerminal = useUIStore((s) => s.toggleTerminal);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const changesPanelCollapsed = useUIStore((s) => s.changesPanelCollapsed);
  const terminalOpen = useUIStore((s) => s.terminalOpen);

  // Extract project name from path
  const projectName = activeSession?.project_path.split("/").pop() || "";
  const sessionTitle = activeSession ? `Session ${activeSession.id.slice(0, 8)}` : "";

  const handleNewSession = async () => {
    try {
      setIsCreating(true);

      // If we have an active session, create a new session in the same project
      if (activeSession?.project_path) {
        await createSession(activeSession.project_path);
      } else {
        // Otherwise, open the folder picker dialog
        const selected = await open({
          directory: true,
          multiple: false,
          title: "Select Project Directory",
        });

        if (selected) {
          await createSession(selected);
        }
      }
    } catch (error) {
      console.error("Failed to create session:", error);
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header - OpenCode style */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-border bg-card">
        <div className="flex items-center gap-1">
          {/* Sidebar toggle */}
          <button
            onClick={toggleSidebar}
            className={cn(
              "p-1.5 rounded transition-colors mr-2",
              sidebarCollapsed
                ? "text-foreground bg-muted"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            )}
            title={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
          >
            <PanelLeft className="w-4 h-4" />
          </button>

          {activeSession && (
            <>
              {/* Project name with dropdown */}
              <button className="flex items-center gap-1 px-2 py-1 text-sm font-medium text-foreground hover:bg-muted rounded transition-colors">
                {projectName}
                <ChevronDown className="w-3 h-3 text-muted-foreground" />
              </button>

              <span className="text-muted-foreground">/</span>

              {/* Session name with dropdown */}
              <button className="flex items-center gap-1 px-2 py-1 text-sm text-muted-foreground hover:text-foreground hover:bg-muted rounded transition-colors">
                {sessionTitle}
                <ChevronDown className="w-3 h-3" />
              </button>
            </>
          )}
        </div>

        <div className="flex items-center gap-2">
          {/* New session button */}
          <button
            onClick={handleNewSession}
            disabled={isCreating}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1.5 text-sm bg-muted hover:bg-muted/80 text-foreground rounded-md transition-colors border border-border/50",
              isCreating && "opacity-50 cursor-not-allowed"
            )}
          >
            {isCreating ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Plus className="w-3.5 h-3.5" />
            )}
            New session
          </button>

          {/* Processing indicator */}
          {isProcessing && (
            <div className="flex items-center gap-1.5 px-2 py-1 rounded-full bg-primary/10 border border-primary/20">
              <Loader2 className="w-3 h-3 text-primary animate-spin" />
              <span className="text-xs text-primary font-medium">Working...</span>
            </div>
          )}

          {/* Connection status */}
          {!isProcessing && (
            <span
              className={cn(
                "w-2 h-2 rounded-full",
                isConnected ? "bg-success" : "bg-muted-foreground"
              )}
              title={isConnected ? "Connected" : "Disconnected"}
            />
          )}

          {/* Token usage */}
          {(tokenUsage.inputTokens > 0 || tokenUsage.outputTokens > 0) && (
            <div className="text-xs text-muted-foreground px-2" title={`Input: ${tokenUsage.inputTokens.toLocaleString()} | Output: ${tokenUsage.outputTokens.toLocaleString()}`}>
              {(tokenUsage.inputTokens + tokenUsage.outputTokens).toLocaleString()} tokens
            </div>
          )}

          {/* Terminal toggle */}
          <button
            onClick={toggleTerminal}
            className={cn(
              "p-1.5 rounded transition-colors",
              terminalOpen
                ? "text-foreground bg-muted"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            )}
            title="Toggle terminal"
          >
            <Terminal className="w-4 h-4" />
          </button>

          {/* Changes panel toggle */}
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

      {/* Messages area - min-h-0 is critical for flex child scrolling */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {activeSession ? (
          <MessageList />
        ) : (
          <div className="h-full flex items-center justify-center">
            <div className="text-center text-muted-foreground">
              <LayoutGrid className="w-12 h-12 mx-auto mb-4 opacity-20" />
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
