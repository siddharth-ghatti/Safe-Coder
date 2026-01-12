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

export function DiffViewer({ diff, showLineNumbers = false }: DiffViewerProps) {
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
    <div className="font-mono text-xs overflow-x-auto bg-background rounded">
      {lines.map((line, index) => {
        if (line.type === "header") {
          return (
            <div
              key={index}
              className="px-3 py-1 bg-accent/10 text-accent border-y border-border"
            >
              {line.content}
            </div>
          );
        }

        return (
          <div
            key={index}
            className={cn(
              "flex",
              line.type === "add" && "bg-diff-add-bg",
              line.type === "remove" && "bg-diff-remove-bg"
            )}
          >
            {showLineNumbers && (
              <>
                <span className="diff-line-number w-10 px-2 py-0.5 text-right select-none border-r border-border">
                  {line.type === "remove" || line.type === "context"
                    ? line.oldLineNumber
                    : ""}
                </span>
                <span className="diff-line-number w-10 px-2 py-0.5 text-right select-none border-r border-border">
                  {line.type === "add" || line.type === "context"
                    ? line.newLineNumber
                    : ""}
                </span>
              </>
            )}

            <span
              className={cn(
                "w-5 px-1 py-0.5 text-center select-none",
                line.type === "add" && "text-diff-add",
                line.type === "remove" && "text-diff-remove"
              )}
            >
              {line.type === "add" ? "+" : line.type === "remove" ? "-" : " "}
            </span>

            <span
              className={cn(
                "flex-1 px-2 py-0.5 whitespace-pre",
                line.type === "add" && "text-diff-add",
                line.type === "remove" && "text-diff-remove",
                line.type === "context" && "text-muted-foreground"
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
