import { useState, useRef, useEffect } from "react";
import { Send, Square, Paperclip } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { cn } from "../../lib/utils";

export function ChatInput() {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendMessage = useSessionStore((s) => s.sendMessage);
  const cancelOperation = useSessionStore((s) => s.cancelOperation);
  const isProcessing = useSessionStore((s) => s.isProcessing);
  const agentMode = useSessionStore((s) => s.agentMode);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(
        textareaRef.current.scrollHeight,
        200
      )}px`;
    }
  }, [input]);

  const handleSubmit = async () => {
    if (!input.trim() || isProcessing) return;
    const message = input.trim();
    setInput("");
    await sendMessage(message);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="border-t border-border bg-card p-4">
      <div className="max-w-4xl mx-auto">
        <div className="flex items-end gap-2 bg-muted rounded-lg p-2">
          {/* Attachment button */}
          <button
            className="p-2 text-muted-foreground hover:text-foreground transition-colors"
            title="Attach file"
          >
            <Paperclip className="w-4 h-4" />
          </button>

          {/* Input */}
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={`Ask anything... "${agentMode === "plan" ? "Plan" : "Build"}" mode`}
            disabled={isProcessing}
            rows={1}
            className="flex-1 bg-transparent text-sm text-foreground placeholder:text-muted-foreground resize-none focus:outline-none min-h-[36px] max-h-[200px] py-2"
          />

          {/* Mode indicator */}
          <div
            className={cn(
              "px-2 py-1 text-xs rounded",
              agentMode === "build"
                ? "bg-primary/20 text-primary"
                : "bg-accent/20 text-accent"
            )}
          >
            {agentMode === "build" ? "Build" : "Plan"}
          </div>

          {/* Send/Cancel button */}
          {isProcessing ? (
            <button
              onClick={cancelOperation}
              className="p-2 bg-destructive text-destructive-foreground rounded hover:bg-destructive/90 transition-colors"
              title="Cancel"
            >
              <Square className="w-4 h-4" />
            </button>
          ) : (
            <button
              onClick={handleSubmit}
              disabled={!input.trim()}
              className={cn(
                "p-2 rounded transition-colors",
                input.trim()
                  ? "bg-primary text-primary-foreground hover:bg-primary/90"
                  : "bg-muted-foreground/20 text-muted-foreground cursor-not-allowed"
              )}
              title="Send (Enter)"
            >
              <Send className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Hints */}
        <div className="flex items-center justify-between mt-2 text-xs text-muted-foreground">
          <span>Press Enter to send, Shift+Enter for new line</span>
          <span className="flex items-center gap-2">
            <kbd className="px-1.5 py-0.5 bg-muted rounded">Cmd+K</kbd>
            <span>Commands</span>
          </span>
        </div>
      </div>
    </div>
  );
}
