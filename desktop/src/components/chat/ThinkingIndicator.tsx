import { Bot } from "lucide-react";

export function ThinkingIndicator() {
  return (
    <div className="flex gap-3">
      <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
        <Bot className="w-4 h-4 text-primary" />
      </div>
      <div className="flex-1">
        <div className="bg-muted/50 rounded-lg p-3 inline-block">
          <div className="flex items-center gap-2">
            <div className="flex gap-1">
              <span className="w-2 h-2 bg-primary/60 rounded-full pulse-dot" style={{ animationDelay: "0ms" }} />
              <span className="w-2 h-2 bg-primary/60 rounded-full pulse-dot" style={{ animationDelay: "200ms" }} />
              <span className="w-2 h-2 bg-primary/60 rounded-full pulse-dot" style={{ animationDelay: "400ms" }} />
            </div>
            <span className="text-xs text-muted-foreground">Thinking...</span>
          </div>
        </div>
      </div>
    </div>
  );
}
