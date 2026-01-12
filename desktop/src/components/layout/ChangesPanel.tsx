import { useState } from "react";
import { ChevronDown, ChevronRight, FileText } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { DiffViewer } from "../changes/DiffViewer";
import { cn, truncatePath, getFileExtension } from "../../lib/utils";

export function ChangesPanel() {
  const fileChanges = useSessionStore((s) => s.fileChanges);
  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set());

  // Calculate totals
  const totalAdditions = fileChanges.reduce((sum, f) => sum + f.additions, 0);
  const totalDeletions = fileChanges.reduce((sum, f) => sum + f.deletions, 0);

  const toggleFile = (path: string) => {
    const newExpanded = new Set(expandedFiles);
    if (newExpanded.has(path)) {
      newExpanded.delete(path);
    } else {
      newExpanded.add(path);
    }
    setExpandedFiles(newExpanded);
  };

  const collapseAll = () => {
    setExpandedFiles(new Set());
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b border-border">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">Review</span>
          <span className="text-xs text-muted-foreground">
            {fileChanges.length}
          </span>
        </div>
        {fileChanges.length > 0 && (
          <button
            onClick={collapseAll}
            className="text-xs text-muted-foreground hover:text-foreground"
          >
            Collapse all
          </button>
        )}
      </div>

      {/* Stats */}
      {fileChanges.length > 0 && (
        <div className="px-3 py-2 border-b border-border">
          <div className="text-sm font-medium mb-1">Session changes</div>
          <div className="flex items-center gap-4 text-xs">
            <span className="text-success">+{totalAdditions}</span>
            <span className="text-destructive">-{totalDeletions}</span>
          </div>
        </div>
      )}

      {/* File list */}
      <div className="flex-1 overflow-y-auto">
        {fileChanges.length === 0 ? (
          <div className="p-4 text-center text-muted-foreground text-sm">
            No file changes yet
          </div>
        ) : (
          <div className="p-2 space-y-1">
            {fileChanges.map((file) => (
              <div key={file.path}>
                {/* File header */}
                <div
                  onClick={() => {
                    toggleFile(file.path);
                  }}
                  className={cn(
                    "flex items-center gap-2 p-2 rounded cursor-pointer transition-colors",
                    expandedFiles.has(file.path)
                      ? "bg-muted"
                      : "hover:bg-muted/50"
                  )}
                >
                  <button className="text-muted-foreground">
                    {expandedFiles.has(file.path) ? (
                      <ChevronDown className="w-4 h-4" />
                    ) : (
                      <ChevronRight className="w-4 h-4" />
                    )}
                  </button>
                  <FileText className="w-4 h-4 text-muted-foreground" />
                  <span className="flex-1 text-sm truncate" title={file.path}>
                    {truncatePath(file.path, 25)}
                  </span>
                  <div className="flex items-center gap-1 text-xs">
                    <span className="text-success">+{file.additions}</span>
                    <span className="text-destructive">-{file.deletions}</span>
                  </div>
                </div>

                {/* Expanded diff */}
                {expandedFiles.has(file.path) && file.diff && (
                  <div className="ml-6 mt-1 mb-2">
                    <DiffViewer
                      diff={file.diff}
                      language={getFileExtension(file.path)}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

    </div>
  );
}
