import { useMemo } from "react";
import { cn } from "../../lib/utils";

interface DiffViewerProps {
  diff: string;
  language?: string;
  showLineNumbers?: boolean;
}

interface DiffLine {
  type: "add" | "remove" | "context" | "header";
  content: string;
  oldLineNumber?: number;
  newLineNumber?: number;
}

export function DiffViewer({ diff, showLineNumbers = true }: DiffViewerProps) {
  const lines = useMemo(() => {
    const result: DiffLine[] = [];
    let oldLine = 0;
    let newLine = 0;

    diff.split("\n").forEach((line) => {
      // Parse hunk header: @@ -start,count +start,count @@
      const hunkMatch = line.match(/@@ -(\d+),?\d* \+(\d+),?\d* @@/);
      if (hunkMatch) {
        oldLine = parseInt(hunkMatch[1], 10) - 1;
        newLine = parseInt(hunkMatch[2], 10) - 1;
        result.push({ type: "header", content: line });
        return;
      }

      // Skip diff headers
      if (
        line.startsWith("---") ||
        line.startsWith("+++") ||
        line.startsWith("diff ") ||
        line.startsWith("index ")
      ) {
        return;
      }

      if (line.startsWith("+")) {
        newLine++;
        result.push({
          type: "add",
          content: line.slice(1),
          newLineNumber: newLine,
        });
      } else if (line.startsWith("-")) {
        oldLine++;
        result.push({
          type: "remove",
          content: line.slice(1),
          oldLineNumber: oldLine,
        });
      } else if (line.startsWith(" ") || line === "") {
        oldLine++;
        newLine++;
        result.push({
          type: "context",
          content: line.startsWith(" ") ? line.slice(1) : line,
          oldLineNumber: oldLine,
          newLineNumber: newLine,
        });
      }
    });

    return result;
  }, [diff]);

  return (
    <div className="font-mono text-xs overflow-x-auto bg-[#1e1e2e] rounded-b-lg">
      {lines.map((line, index) => {
        if (line.type === "header") {
          return (
            <div
              key={index}
              className="px-3 py-1.5 bg-accent/10 text-accent border-b border-border/50 text-xs"
            >
              {line.content}
            </div>
          );
        }

        return (
          <div
            key={index}
            className={cn(
              "flex group hover:bg-white/5 transition-colors",
              line.type === "add" && "bg-diff-add-bg",
              line.type === "remove" && "bg-diff-remove-bg"
            )}
          >
            {showLineNumbers && (
              <div className="flex flex-shrink-0 select-none border-r border-border/30">
                <span
                  className={cn(
                    "w-12 px-2 py-0.5 text-right text-muted-foreground/50",
                    line.type === "add" && "bg-diff-add-bg/50",
                    line.type === "remove" && "bg-diff-remove-bg/50"
                  )}
                >
                  {line.type === "remove" || line.type === "context"
                    ? line.oldLineNumber
                    : ""}
                </span>
                <span
                  className={cn(
                    "w-12 px-2 py-0.5 text-right text-muted-foreground/50 border-r border-border/30",
                    line.type === "add" && "bg-diff-add-bg/50",
                    line.type === "remove" && "bg-diff-remove-bg/50"
                  )}
                >
                  {line.type === "add" || line.type === "context"
                    ? line.newLineNumber
                    : ""}
                </span>
              </div>
            )}

            {/* Change indicator */}
            <span
              className={cn(
                "w-6 px-1.5 py-0.5 text-center select-none flex-shrink-0",
                line.type === "add" && "text-diff-add bg-diff-add-bg/30",
                line.type === "remove" && "text-diff-remove bg-diff-remove-bg/30",
                line.type === "context" && "text-muted-foreground/30"
              )}
            >
              {line.type === "add" ? "+" : line.type === "remove" ? "-" : " "}
            </span>

            {/* Code content */}
            <span
              className={cn(
                "flex-1 px-3 py-0.5 whitespace-pre overflow-x-auto",
                line.type === "add" && "text-diff-add",
                line.type === "remove" && "text-diff-remove",
                line.type === "context" && "text-muted-foreground/80"
              )}
            >
              {line.content || " "}
            </span>
          </div>
        );
      })}
    </div>
  );
}
