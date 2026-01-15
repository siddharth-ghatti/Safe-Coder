import { useState } from "react";
import { ChevronDown, ChevronRight, FileCode2, ChevronsDownUp, Plus } from "lucide-react";
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

  const expandAll = () => {
    setExpandedFiles(new Set(fileChanges.map(f => f.path)));
  };

  // Get file icon color based on extension
  const getFileColor = (path: string) => {
    const ext = getFileExtension(path);
    const colors: Record<string, string> = {
      rs: "text-orange-400",
      ts: "text-blue-400",
      tsx: "text-blue-400",
      js: "text-yellow-400",
      jsx: "text-yellow-400",
      py: "text-green-400",
      go: "text-cyan-400",
      json: "text-yellow-500",
      md: "text-gray-400",
    };
    return colors[ext] || "text-muted-foreground";
  };

  return (
    <div className="h-full flex flex-col bg-card">
      {/* Header tabs */}
      <div className="flex items-center gap-4 px-4 py-3 border-b border-border">
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1">
            <div className="flex gap-0.5">
              <div className="w-0.5 h-3 bg-primary" />
              <div className="w-0.5 h-3 bg-primary" />
              <div className="w-0.5 h-3 bg-primary" />
              <div className="w-0.5 h-3 bg-primary" />
            </div>
          </div>
          <span className="text-sm font-medium">Review</span>
          <span className="text-xs text-muted-foreground bg-muted px-1.5 py-0.5 rounded">
            {fileChanges.length}
          </span>
        </div>
        <button className="p-1 text-muted-foreground hover:text-foreground rounded transition-colors">
          <Plus className="w-4 h-4" />
        </button>
      </div>

      {/* Session changes header */}
      {fileChanges.length > 0 && (
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border">
          <span className="text-sm font-medium">Session changes</span>
          <button
            onClick={expandedFiles.size > 0 ? collapseAll : expandAll}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            <ChevronsDownUp className="w-3.5 h-3.5" />
            {expandedFiles.size > 0 ? "Collapse all" : "Expand all"}
          </button>
        </div>
      )}

      {/* File list */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {fileChanges.length === 0 ? (
          <div className="p-8 text-center text-muted-foreground">
            <FileCode2 className="w-10 h-10 mx-auto mb-3 opacity-30" />
            <p className="text-sm">No file changes yet</p>
            <p className="text-xs mt-1">Changes will appear here as you work</p>
          </div>
        ) : (
          <div className="p-2 space-y-1">
            {fileChanges.map((file) => (
              <div key={file.path} className="rounded-lg overflow-hidden">
                {/* File header */}
                <div
                  onClick={() => toggleFile(file.path)}
                  className={cn(
                    "flex items-center gap-2 px-3 py-2 cursor-pointer transition-colors",
                    expandedFiles.has(file.path)
                      ? "bg-muted border-b border-border"
                      : "hover:bg-muted/50"
                  )}
                >
                  <button className="text-muted-foreground flex-shrink-0">
                    {expandedFiles.has(file.path) ? (
                      <ChevronDown className="w-4 h-4" />
                    ) : (
                      <ChevronRight className="w-4 h-4" />
                    )}
                  </button>

                  <FileCode2 className={cn("w-4 h-4 flex-shrink-0", getFileColor(file.path))} />

                  <span className="flex-1 text-sm font-mono truncate" title={file.path}>
                    {truncatePath(file.path, 35)}
                  </span>

                  <div className="flex items-center gap-1.5 text-xs font-medium">
                    <span className="text-success">+{file.additions}</span>
                    <span className="text-destructive">-{file.deletions}</span>
                  </div>
                </div>

                {/* Expanded diff with code viewer */}
                {expandedFiles.has(file.path) && file.diff && (
                  <div className="bg-background">
                    <DiffViewer
                      diff={file.diff}
                      language={getFileExtension(file.path)}
                      showLineNumbers
                    />
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Footer stats */}
      {fileChanges.length > 0 && (
        <div className="px-4 py-2.5 border-t border-border bg-muted/30">
          <div className="flex items-center justify-between text-xs">
            <span className="text-muted-foreground">
              {fileChanges.length} file{fileChanges.length !== 1 ? "s" : ""} changed
            </span>
            <div className="flex items-center gap-3">
              <span className="text-success font-medium">+{totalAdditions}</span>
              <span className="text-destructive font-medium">-{totalDeletions}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
