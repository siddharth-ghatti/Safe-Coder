import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Plus, Settings, MessageSquare, Terminal } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { useUIStore } from "../../stores/uiStore";
import { SessionList } from "../sidebar/SessionList";
import { cn } from "../../lib/utils";

export function Sidebar() {
  const [projectPath, setProjectPath] = useState("");
  const [isCreating, setIsCreating] = useState(false);
  const createSession = useSessionStore((s) => s.createSession);
  const agentMode = useSessionStore((s) => s.agentMode);
  const setAgentMode = useSessionStore((s) => s.setAgentMode);
  const toggleTerminal = useUIStore((s) => s.toggleTerminal);
  const terminalOpen = useUIStore((s) => s.terminalOpen);

  const handleCreateSession = async (path?: string) => {
    const targetPath = path || projectPath.trim();
    if (!targetPath) return;

    setIsCreating(true);
    try {
      await createSession(targetPath);
      setProjectPath("");
    } catch (error) {
      console.error("Failed to create session:", error);
      alert(`Failed to create session: ${error}`);
    } finally {
      setIsCreating(false);
    }
  };

  const handleOpenProject = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Project Directory",
      });

      if (selected) {
        // Directly create session with selected path
        await handleCreateSession(selected);
      }
    } catch (error) {
      console.error("Failed to open directory picker:", error);
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-border">
        <div className="flex items-center gap-2 mb-4">
          <div className="w-8 h-8 rounded bg-primary/20 flex items-center justify-center">
            <MessageSquare className="w-4 h-4 text-primary" />
          </div>
          <span className="font-semibold text-foreground">Safe Coder</span>
        </div>

        {/* New session input */}
        <div className="space-y-2">
          <div className="flex gap-2">
            <input
              type="text"
              value={projectPath}
              onChange={(e) => setProjectPath(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreateSession()}
              placeholder="Project path..."
              className="flex-1 px-3 py-2 bg-muted border border-border rounded text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-primary"
            />
            <button
              onClick={() => handleCreateSession()}
              disabled={!projectPath.trim() || isCreating}
              className={cn(
                "p-2 bg-primary text-primary-foreground rounded hover:bg-primary/90 transition-colors",
                (!projectPath.trim() || isCreating) && "opacity-50 cursor-not-allowed"
              )}
              title="New session"
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>

          {/* Open folder button */}
          <button
            onClick={handleOpenProject}
            disabled={isCreating}
            className="w-full flex items-center justify-center gap-2 p-2 bg-muted hover:bg-muted/80 text-foreground rounded transition-colors"
          >
            <FolderOpen className="w-4 h-4" />
            <span className="text-sm">Open Project Folder</span>
          </button>
        </div>
      </div>

      {/* Sessions list */}
      <div className="flex-1 overflow-y-auto">
        <SessionList />
      </div>

      {/* Footer with mode toggle and settings */}
      <div className="p-4 border-t border-border space-y-3">
        {/* Mode toggle */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">Mode:</span>
          <div className="flex-1 flex bg-muted rounded p-0.5">
            <button
              onClick={() => setAgentMode("build")}
              className={cn(
                "flex-1 px-3 py-1 text-xs rounded transition-colors",
                agentMode === "build"
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              Build
            </button>
            <button
              onClick={() => setAgentMode("plan")}
              className={cn(
                "flex-1 px-3 py-1 text-xs rounded transition-colors",
                agentMode === "plan"
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              Plan
            </button>
          </div>
        </div>

        {/* Quick actions */}
        <div className="flex items-center gap-2">
          <button
            onClick={toggleTerminal}
            className={cn(
              "flex items-center gap-2 p-2 text-sm rounded transition-colors",
              terminalOpen
                ? "bg-primary/20 text-primary"
                : "text-muted-foreground hover:text-foreground hover:bg-muted"
            )}
            title="Toggle terminal"
          >
            <Terminal className="w-4 h-4" />
            <span>Terminal</span>
          </button>
          <button
            className="ml-auto p-2 text-muted-foreground hover:text-foreground hover:bg-muted rounded transition-colors"
            title="Settings"
          >
            <Settings className="w-4 h-4" />
          </button>
        </div>

        {/* Getting started hint */}
        <div className="p-3 bg-muted/50 rounded text-xs text-muted-foreground">
          <p className="font-medium mb-1">Getting started</p>
          <p>Click "Open Project Folder" to start a new session, or select an existing session.</p>
        </div>
      </div>
    </div>
  );
}
