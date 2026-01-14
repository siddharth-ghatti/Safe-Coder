import { useRef, useEffect } from "react";
import { Check, X, Loader2, FileCode2, Terminal } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ThinkingIndicator } from "./ThinkingIndicator";
import { cn } from "../../lib/utils";

export function MessageList() {
  const messages = useSessionStore((s) => s.messages);
  const streamingMessage = useSessionStore((s) => s.streamingMessage);
  const thinkingMessage = useSessionStore((s) => s.thinkingMessage);
  const isProcessing = useSessionStore((s) => s.isProcessing);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Debug logging
  useEffect(() => {
    console.log("MessageList state:", {
      messagesCount: messages.length,
      isProcessing,
      hasStreamingMessage: !!streamingMessage,
      streamingContent: streamingMessage?.content?.slice(0, 50),
      toolExecutionsCount: streamingMessage?.toolExecutions?.length,
      thinkingMessage,
    });
  }, [messages, streamingMessage, thinkingMessage, isProcessing]);

  // Auto-scroll to bottom on new messages or streaming updates
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTo({
        top: scrollRef.current.scrollHeight,
        behavior: "smooth",
      });
    }
  }, [messages, streamingMessage?.content, streamingMessage?.toolExecutions?.length, thinkingMessage, isProcessing]);

  return (
    <div ref={scrollRef} className="h-full overflow-y-auto scroll-smooth">
      <div className="max-w-4xl mx-auto py-4 px-4 space-y-4 pb-8">
        {/* Existing messages */}
        {messages.map((message) =>
          message.role === "user" ? (
            <UserMessage key={message.id} message={message} />
          ) : (
            <AssistantMessage key={message.id} message={message} />
          )
        )}

        {/* Thinking/Reasoning indicator - shown when we have a thinking message */}
        {thinkingMessage && (
          <div className="flex gap-3 animate-in fade-in-0 duration-150">
            <div className="relative w-8 h-8 flex-shrink-0">
              <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center">
                <Loader2 className="w-4 h-4 text-primary animate-spin" />
              </div>
            </div>
            <div className="flex-1 py-2">
              <div className="bg-muted/30 border border-border/30 rounded-lg px-3 py-2 inline-block">
                <span className="text-sm text-muted-foreground/80 italic">
                  {thinkingMessage}
                </span>
              </div>
            </div>
          </div>
        )}

        {/* Streaming message with tool executions */}
        {streamingMessage && (
          <div className="space-y-3">
            {/* Tool executions - each shown in a card */}
            {streamingMessage.toolExecutions.length > 0 && (
              <div className="space-y-2">
                {streamingMessage.toolExecutions.map((tool, index) => (
                  <div
                    key={tool.id}
                    className={cn(
                      "bg-muted/30 rounded-lg border border-border/50 overflow-hidden tool-execution",
                      tool.success === undefined && "border-primary/30"
                    )}
                  >
                    {/* Tool header */}
                    <div className="flex items-center gap-2 px-3 py-2 bg-muted/20">
                      {tool.success === undefined ? (
                        <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
                      ) : tool.success ? (
                        <Check className="w-3.5 h-3.5 text-success" />
                      ) : (
                        <X className="w-3.5 h-3.5 text-destructive" />
                      )}

                      <span className="text-xs font-medium text-primary">
                        {tool.name}
                      </span>

                      {tool.success === undefined && (
                        <span className="text-[10px] text-muted-foreground">
                          Running...
                        </span>
                      )}
                    </div>

                    {/* Tool description */}
                    <div className="px-3 py-2 border-t border-border/30">
                      <p className="text-xs text-muted-foreground">
                        {tool.description || `Executing ${tool.name}...`}
                      </p>
                    </div>

                    {/* Tool output - if any */}
                    {tool.output && (
                      <div className="px-3 py-2 border-t border-border/30 bg-background/30">
                        <pre className="text-xs text-muted-foreground/80 font-mono overflow-x-auto max-h-32 overflow-y-auto whitespace-pre-wrap">
                          {tool.output.length > 500
                            ? tool.output.slice(0, 500) + "..."
                            : tool.output}
                        </pre>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

            {/* Streaming text content */}
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

            {/* Show activity indicator if tools are running but no text yet */}
            {streamingMessage.toolExecutions.length > 0 &&
             !streamingMessage.content &&
             streamingMessage.toolExecutions.some(t => t.success === undefined) && (
              <div className="flex items-center gap-2 text-xs text-muted-foreground ml-3">
                <Loader2 className="w-3 h-3 animate-spin" />
                <span>Working on your request...</span>
              </div>
            )}
          </div>
        )}

        {/* Initial thinking indicator (when no content yet) */}
        {isProcessing && !streamingMessage?.content && !streamingMessage?.toolExecutions?.length && !thinkingMessage && (
          <ThinkingIndicator />
        )}
      </div>
    </div>
  );
}
