import { useRef, useEffect } from "react";
import { Check, X } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ThinkingIndicator } from "./ThinkingIndicator";

export function MessageList() {
  const messages = useSessionStore((s) => s.messages);
  const streamingMessage = useSessionStore((s) => s.streamingMessage);
  const thinkingMessage = useSessionStore((s) => s.thinkingMessage);
  const isProcessing = useSessionStore((s) => s.isProcessing);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages or streaming updates
  useEffect(() => {
    if (scrollRef.current) {
      // Use smooth scroll for better UX
      scrollRef.current.scrollTo({
        top: scrollRef.current.scrollHeight,
        behavior: "smooth",
      });
    }
  }, [messages, streamingMessage?.content, streamingMessage?.toolExecutions?.length, thinkingMessage, isProcessing]);

  return (
    <div ref={scrollRef} className="h-full overflow-y-auto">
      <div className="max-w-4xl mx-auto py-4 px-4 space-y-4">
        {messages.map((message) =>
          message.role === "user" ? (
            <UserMessage key={message.id} message={message} />
          ) : (
            <AssistantMessage key={message.id} message={message} />
          )
        )}

        {/* Thinking/Reasoning indicator */}
        {thinkingMessage && (
          <div className="flex gap-3 animate-in fade-in-0 duration-150">
            <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
              <div className="flex gap-0.5">
                <div className="w-1.5 h-1.5 rounded-full bg-primary animate-bounce" style={{ animationDelay: "0ms" }} />
                <div className="w-1.5 h-1.5 rounded-full bg-primary animate-bounce" style={{ animationDelay: "150ms" }} />
                <div className="w-1.5 h-1.5 rounded-full bg-primary animate-bounce" style={{ animationDelay: "300ms" }} />
              </div>
            </div>
            <div className="flex-1 py-2">
              <span className="text-sm text-muted-foreground/80 italic">
                {thinkingMessage}
              </span>
            </div>
          </div>
        )}

        {/* Streaming message */}
        {streamingMessage && (
          <div className="space-y-3">
            {/* Tool executions */}
            {streamingMessage.toolExecutions.map((tool) => (
              <div
                key={tool.id}
                className="ml-11 bg-muted/30 rounded-lg p-3 border border-border/50 animate-in slide-in-from-left-2 duration-200"
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-xs font-medium text-primary">
                    {tool.name}
                  </span>
                  {tool.success === undefined && (
                    <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />
                  )}
                  {tool.success !== undefined && (
                    <span
                      className={`transition-colors duration-200 ${
                        tool.success ? "text-success" : "text-destructive"
                      }`}
                    >
                      {tool.success ? <Check className="w-3 h-3" /> : <X className="w-3 h-3" />}
                    </span>
                  )}
                </div>
                <p className="text-xs text-muted-foreground mb-2 transition-opacity duration-150">
                  {tool.description}
                </p>
                {tool.output && (
                  <pre className="text-xs text-muted-foreground/80 font-mono bg-background/50 p-2 rounded overflow-x-auto max-h-32 overflow-y-auto transition-all duration-150">
                    {tool.output.length > 500
                      ? tool.output.slice(0, 500) + "..."
                      : tool.output}
                  </pre>
                )}
              </div>
            ))}

            {/* Streaming text */}
            {streamingMessage.content && (
              <AssistantMessage
                message={{
                  id: "streaming",
                  role: "assistant",
                  content: streamingMessage.content,
                  timestamp: new Date().toISOString(),
                }}
                isStreaming
              />
            )}
          </div>
        )}

        {/* Initial thinking indicator (when no content yet) */}
        {isProcessing && !streamingMessage?.content && !thinkingMessage && (
          <ThinkingIndicator />
        )}
      </div>
    </div>
  );
}
