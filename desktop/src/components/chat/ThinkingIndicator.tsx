import { Bot, Loader2, Search, FileCode2, Brain } from "lucide-react";

interface ThinkingIndicatorProps {
  message?: string;
  type?: "thinking" | "reading" | "searching" | "analyzing";
}

export function ThinkingIndicator({ message, type = "thinking" }: ThinkingIndicatorProps) {
  const getIcon = () => {
    switch (type) {
      case "reading":
        return <FileCode2 className="w-4 h-4 text-primary" />;
      case "searching":
        return <Search className="w-4 h-4 text-primary" />;
      case "analyzing":
        return <Brain className="w-4 h-4 text-primary" />;
      default:
        return <Bot className="w-4 h-4 text-primary" />;
    }
  };

  const getDefaultMessage = () => {
    switch (type) {
      case "reading":
        return "Reading files...";
      case "searching":
        return "Searching codebase...";
      case "analyzing":
        return "Analyzing...";
      default:
        return "Thinking...";
    }
  };

  return (
    <div className="flex gap-3 animate-in fade-in-0 slide-in-from-bottom-2 duration-200">
      {/* Avatar with spinning indicator */}
      <div className="relative w-8 h-8 flex-shrink-0">
        <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center">
          {getIcon()}
        </div>
        {/* Spinning ring around avatar */}
        <div className="absolute inset-0 rounded-full border-2 border-transparent border-t-primary animate-spin" />
      </div>

      {/* Content card */}
      <div className="flex-1">
        <div className="bg-muted/50 border border-border/50 rounded-lg p-4 inline-block min-w-[200px]">
          {/* Working indicator */}
          <div className="flex items-center gap-3">
            <Loader2 className="w-4 h-4 text-primary animate-spin" />
            <span className="text-sm text-foreground font-medium">
              {message || getDefaultMessage()}
            </span>
          </div>

          {/* Progress bar */}
          <div className="mt-3 h-1 bg-muted rounded-full overflow-hidden">
            <div className="h-full bg-primary/60 rounded-full animate-progress" />
          </div>

          {/* Status dots */}
          <div className="flex items-center gap-1.5 mt-3">
            <span className="w-2 h-2 bg-primary rounded-full pulse-dot" style={{ animationDelay: "0ms" }} />
            <span className="w-2 h-2 bg-primary/60 rounded-full pulse-dot" style={{ animationDelay: "200ms" }} />
            <span className="w-2 h-2 bg-primary/40 rounded-full pulse-dot" style={{ animationDelay: "400ms" }} />
            <span className="text-[10px] text-muted-foreground ml-2">Processing your request</span>
          </div>
        </div>
      </div>
    </div>
  );
}
