import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Plus, Settings, MessageSquare, Plug, Folder } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { SessionList } from "../sidebar/SessionList";
import { cn } from "../../lib/utils";

export function Sidebar() {
  const [projectPath, setProjectPath] = useState("");
  const [isCreating, setIsCreating] = useState(false);
  const createSession = useSessionStore((s) => s.createSession);
  const agentMode = useSessionStore((s) => s.agentMode);
  const setAgentMode = useSessionStore((s) => s.setAgentMode);

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
        await handleCreateSession(selected);
      }
    } catch (error) {
      console.error("Failed to open directory picker:", error);
    }
  };

  return (
    <div className="h-full flex flex-col bg-card">
      {/* Logo/Brand header */}
      <div className="p-4 border-b border-border">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-lg bg-primary/20 flex items-center justify-center">
            <MessageSquare className="w-4 h-4 text-primary" />
          </div>
          <div>
            <span className="font-semibold text-foreground text-sm">Safe Coder</span>
            <p className="text-[10px] text-muted-foreground">AI Coding Assistant</p>
          </div>
        </div>
      </div>

      {/* Sessions list */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        <SessionList />
      </div>

      {/* Getting Started section - OpenCode style */}
      <div className="border-t border-border">
        <div className="p-4">
          <p className="text-sm font-medium mb-2">Getting started</p>
          <p className="text-xs text-muted-foreground mb-4">
            Safe Coder uses GitHub Copilot so you can start immediately.
          </p>

          {/* Quick action buttons */}
          <div className="space-y-2">
            <button
              onClick={handleOpenProject}
              disabled={isCreating}
              className="w-full flex items-center gap-2 px-3 py-2 text-sm text-muted-foreground hover:text-foreground hover:bg-muted rounded-md transition-colors"
            >
              <Folder className="w-4 h-4" />
              <span>Open project</span>
            </button>
          </div>
        </div>

        {/* Mode toggle & settings footer */}
        <div className="px-4 pb-4 space-y-3">
          {/* Mode toggle */}
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">Mode:</span>
            <div className="flex-1 flex bg-muted rounded-md p-0.5">
              <button
                onClick={() => setAgentMode("build")}
                className={cn(
                  "flex-1 px-3 py-1.5 text-xs rounded-md transition-all",
                  agentMode === "build"
                    ? "bg-primary text-primary-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                Build
              </button>
              <button
                onClick={() => setAgentMode("plan")}
                className={cn(
                  "flex-1 px-3 py-1.5 text-xs rounded-md transition-all",
                  agentMode === "plan"
                    ? "bg-amber-500 text-white shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                Plan
              </button>
            </div>
          </div>

          {/* Settings button */}
          <button
            className="w-full flex items-center justify-center gap-2 p-2 text-xs text-muted-foreground hover:text-foreground hover:bg-muted rounded-md transition-colors"
            title="Settings"
          >
            <Settings className="w-4 h-4" />
            <span>Settings</span>
          </button>
        </div>
      </div>
    </div>
  );
}
