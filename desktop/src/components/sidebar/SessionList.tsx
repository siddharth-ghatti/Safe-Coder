import { useEffect, useRef, useState, useMemo } from "react";
import { Trash2, RefreshCw, MessageSquare, FolderOpen, ChevronRight, Plus } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "../../stores/sessionStore";
import { formatTimestamp, cn } from "../../lib/utils";
import type { SessionSummary } from "../../types";

interface ProjectGroup {
  projectPath: string;
  projectName: string;
  sessions: SessionSummary[];
}

export function SessionList() {
  const sessions = useSessionStore((s) => s.sessions);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const loadSessions = useSessionStore((s) => s.loadSessions);
  const selectSession = useSessionStore((s) => s.selectSession);
  const deleteSession = useSessionStore((s) => s.deleteSession);
  const createSession = useSessionStore((s) => s.createSession);
  const hasAutoSelected = useRef(false);
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set());
  const [isCreating, setIsCreating] = useState(false);

  // Group sessions by project
  const projectGroups = useMemo(() => {
    const groups: Map<string, ProjectGroup> = new Map();

    sessions.forEach((session) => {
      const projectPath = session.project_path;
      const projectName = projectPath.split("/").pop() || "Unknown";

      if (!groups.has(projectPath)) {
        groups.set(projectPath, {
          projectPath,
          projectName,
          sessions: [],
        });
      }
      groups.get(projectPath)!.sessions.push(session);
    });

    // Sort sessions within each group by created_at (newest first)
    groups.forEach((group) => {
      group.sessions.sort((a, b) =>
        new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    });

    // Convert to array and sort by most recent session
    return Array.from(groups.values()).sort((a, b) => {
      const aLatest = new Date(a.sessions[0]?.created_at || 0).getTime();
      const bLatest = new Date(b.sessions[0]?.created_at || 0).getTime();
      return bLatest - aLatest;
    });
  }, [sessions]);

  // Load sessions on mount and poll every 5 seconds
  useEffect(() => {
    loadSessions();
    const interval = setInterval(loadSessions, 5000);
    return () => clearInterval(interval);
  }, [loadSessions]);

  // Auto-select first session if none selected
  useEffect(() => {
    if (sessions.length > 0 && !activeSessionId && !hasAutoSelected.current) {
      hasAutoSelected.current = true;
      selectSession(sessions[0].id);
    }
  }, [sessions, activeSessionId, selectSession]);

  // Auto-expand project containing active session
  useEffect(() => {
    if (activeSessionId) {
      const activeSession = sessions.find(s => s.id === activeSessionId);
      if (activeSession) {
        setExpandedProjects((prev) => new Set([...prev, activeSession.project_path]));
      }
    }
  }, [activeSessionId, sessions]);

  const toggleProject = (projectPath: string) => {
    setExpandedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(projectPath)) {
        next.delete(projectPath);
      } else {
        next.add(projectPath);
      }
      return next;
    });
  };

  const handleNewProject = async () => {
    try {
      setIsCreating(true);
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Project Directory",
      });

      if (selected) {
        await createSession(selected);
      }
    } catch (error) {
      console.error("Failed to create session:", error);
    } finally {
      setIsCreating(false);
    }
  };

  const handleNewSessionInProject = async (projectPath: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      setIsCreating(true);
      await createSession(projectPath);
    } catch (error) {
      console.error("Failed to create session:", error);
    } finally {
      setIsCreating(false);
    }
  };

  if (sessions.length === 0) {
    return (
      <div className="p-4 text-center text-muted-foreground text-sm">
        <MessageSquare className="w-8 h-8 mx-auto mb-2 opacity-30" />
        <p className="text-xs">No sessions yet</p>
        <button
          onClick={handleNewProject}
          disabled={isCreating}
          className="mt-2 text-xs text-primary hover:underline inline-flex items-center gap-1"
        >
          <FolderOpen className="w-3 h-3" />
          Open Project
        </button>
      </div>
    );
  }

  return (
    <div className="p-2">
      <div className="flex items-center justify-between px-2 py-1.5 mb-2">
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          Projects
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={handleNewProject}
            disabled={isCreating}
            className="p-1 text-muted-foreground hover:text-foreground rounded transition-colors"
            title="Open new project"
          >
            <FolderOpen className="w-3 h-3" />
          </button>
          <button
            onClick={() => loadSessions()}
            className="p-1 text-muted-foreground hover:text-foreground rounded transition-colors"
            title="Refresh sessions"
          >
            <RefreshCw className="w-3 h-3" />
          </button>
        </div>
      </div>

      <div className="space-y-1">
        {projectGroups.map((group) => {
          const isExpanded = expandedProjects.has(group.projectPath);
          const hasActiveSession = group.sessions.some(s => s.id === activeSessionId);

          return (
            <div key={group.projectPath}>
              {/* Project Header */}
              <div
                onClick={() => toggleProject(group.projectPath)}
                className={cn(
                  "group flex items-center gap-2 px-2 py-1.5 rounded-lg cursor-pointer transition-all",
                  hasActiveSession
                    ? "bg-muted/50"
                    : "hover:bg-muted/30"
                )}
              >
                <ChevronRight
                  className={cn(
                    "w-3 h-3 text-muted-foreground transition-transform",
                    isExpanded && "rotate-90"
                  )}
                />
                <div className={cn(
                  "w-5 h-5 rounded flex items-center justify-center text-xs font-medium flex-shrink-0",
                  hasActiveSession
                    ? "bg-primary/20 text-primary"
                    : "bg-muted text-muted-foreground"
                )}>
                  {group.projectName.charAt(0).toUpperCase()}
                </div>
                <span className={cn(
                  "flex-1 text-sm font-medium truncate",
                  hasActiveSession ? "text-foreground" : "text-muted-foreground"
                )}>
                  {group.projectName}
                </span>
                <span className="text-xs text-muted-foreground">
                  {group.sessions.length}
                </span>
                <button
                  onClick={(e) => handleNewSessionInProject(group.projectPath, e)}
                  disabled={isCreating}
                  className="p-1 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-primary rounded transition-all"
                  title="New session in this project"
                >
                  <Plus className="w-3 h-3" />
                </button>
              </div>

              {/* Sessions List (when expanded) */}
              {isExpanded && (
                <div className="ml-4 mt-1 space-y-0.5 border-l border-border/30 pl-2">
                  {group.sessions.map((session) => (
                    <div
                      key={session.id}
                      onClick={() => selectSession(session.id)}
                      className={cn(
                        "group flex items-center gap-2 px-2 py-1.5 rounded cursor-pointer transition-all",
                        activeSessionId === session.id
                          ? "bg-primary/10 border border-primary/20"
                          : "hover:bg-muted/30"
                      )}
                    >
                      <MessageSquare className={cn(
                        "w-3 h-3 flex-shrink-0",
                        activeSessionId === session.id
                          ? "text-primary"
                          : "text-muted-foreground"
                      )} />

                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className={cn(
                            "text-xs truncate",
                            activeSessionId === session.id
                              ? "text-foreground font-medium"
                              : "text-muted-foreground"
                          )}>
                            {session.id.slice(0, 8)}
                          </span>
                          <span className="text-[10px] text-muted-foreground/60">
                            {formatTimestamp(session.created_at)}
                          </span>
                        </div>

                        {/* File changes badge */}
                        {session.file_changes.total_files > 0 && (
                          <div className="flex items-center gap-1 text-[10px] mt-0.5">
                            <span className="text-success font-medium">
                              +{session.file_changes.additions}
                            </span>
                            <span className="text-destructive font-medium">
                              -{session.file_changes.deletions}
                            </span>
                          </div>
                        )}
                      </div>

                      {/* Delete button */}
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          deleteSession(session.id);
                        }}
                        className="p-1 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive rounded transition-all"
                        title="Delete session"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
