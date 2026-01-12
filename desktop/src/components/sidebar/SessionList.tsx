import { useEffect, useRef } from "react";
import { Trash2, RefreshCw } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { formatTimestamp, truncatePath, cn } from "../../lib/utils";

export function SessionList() {
  const sessions = useSessionStore((s) => s.sessions);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const loadSessions = useSessionStore((s) => s.loadSessions);
  const selectSession = useSessionStore((s) => s.selectSession);
  const deleteSession = useSessionStore((s) => s.deleteSession);
  const hasAutoSelected = useRef(false);

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

  if (sessions.length === 0) {
    return (
      <div className="p-4 text-center text-muted-foreground text-sm">
        <p>No sessions yet</p>
        <button
          onClick={() => loadSessions()}
          className="mt-2 text-xs text-primary hover:underline"
        >
          Refresh
        </button>
      </div>
    );
  }

  return (
    <div className="p-2">
      <div className="flex items-center justify-between px-2 py-1 mb-1">
        <span className="text-xs font-medium text-muted-foreground">
          Recent Sessions
        </span>
        <button
          onClick={() => loadSessions()}
          className="p-1 text-muted-foreground hover:text-foreground"
          title="Refresh sessions"
        >
          <RefreshCw className="w-3 h-3" />
        </button>
      </div>
      <div className="space-y-1">
        {sessions.map((session) => (
          <div
            key={session.id}
            onClick={() => selectSession(session.id)}
            className={cn(
              "group p-2 rounded cursor-pointer transition-colors",
              activeSessionId === session.id
                ? "bg-muted"
                : "hover:bg-muted/50"
            )}
          >
            <div className="flex items-start justify-between gap-2">
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium text-foreground truncate">
                  {truncatePath(session.project_path, 30)}
                </div>
                <div className="flex items-center gap-2 mt-1">
                  <span className="text-xs text-muted-foreground">
                    {formatTimestamp(session.created_at)}
                  </span>
                  {session.file_changes.total_files > 0 && (
                    <span className="text-xs">
                      <span className="text-success">
                        +{session.file_changes.additions}
                      </span>
                      {" "}
                      <span className="text-destructive">
                        -{session.file_changes.deletions}
                      </span>
                    </span>
                  )}
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  deleteSession(session.id);
                }}
                className="p-1 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-opacity"
                title="Delete session"
              >
                <Trash2 className="w-3 h-3" />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
