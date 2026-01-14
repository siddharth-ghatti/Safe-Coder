import { useRef, useEffect, useState, useCallback } from "react";
import { Check, X, Loader2, ArrowDown, FileCode, Terminal, Search, Eye, ChevronDown, ChevronRight } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ThinkingIndicator } from "./ThinkingIndicator";
import { cn } from "../../lib/utils";
import type { ToolExecution } from "../../types";

// Tool execution card with enhanced display for file operations
function ToolExecutionCard({ tool }: { tool: ToolExecution }) {
  const [expanded, setExpanded] = useState(false);

  // Determine tool type and extract relevant info from description
  const isFileOperation = tool.name === "edit_file" || tool.name === "write_file" || tool.name === "read_file";
  const isBashCommand = tool.name === "bash";
  const isSearchTool = tool.name === "grep" || tool.name === "glob" || tool.name === "list_file";

  // Extract file path from description (e.g., "📖 Read `path/to/file`" -> "path/to/file")
  const filePathMatch = tool.description?.match(/`([^`]+)`/);
  const filePath = filePathMatch?.[1];

  // Get icon based on tool type
  const getToolIcon = () => {
    if (isFileOperation) return <FileCode className="w-3.5 h-3.5" />;
    if (isBashCommand) return <Terminal className="w-3.5 h-3.5" />;
    if (isSearchTool) return <Search className="w-3.5 h-3.5" />;
    return <Eye className="w-3.5 h-3.5" />;
  };

  // For file operations, show more of the output
  const outputLimit = isFileOperation ? 2000 : 500;
  const hasLongOutput = (tool.output?.length || 0) > outputLimit;

  return (
    <div
      className={cn(
        "bg-muted/30 rounded-lg border border-border/50 overflow-hidden tool-execution",
        tool.success === undefined && "border-primary/30 animate-pulse-subtle"
      )}
    >
      {/* Tool header */}
      <div
        className="flex items-center gap-2 px-3 py-2 bg-muted/20 cursor-pointer hover:bg-muted/30 transition-colors"
        onClick={() => tool.output && setExpanded(!expanded)}
      >
        {tool.success === undefined ? (
          <Loader2 className="w-3.5 h-3.5 text-primary animate-spin flex-shrink-0" />
        ) : tool.success ? (
          <Check className="w-3.5 h-3.5 text-success flex-shrink-0" />
        ) : (
          <X className="w-3.5 h-3.5 text-destructive flex-shrink-0" />
        )}

        <span className={cn(
          "flex-shrink-0",
          isFileOperation ? "text-blue-400" : isBashCommand ? "text-orange-400" : "text-muted-foreground"
        )}>
          {getToolIcon()}
        </span>

        <span className="text-xs font-medium text-primary">
          {tool.name}
        </span>

        {filePath && (
          <code className="text-xs text-muted-foreground font-mono truncate flex-1" title={filePath}>
            {filePath}
          </code>
        )}

        {tool.success === undefined && (
          <span className="text-[10px] text-muted-foreground ml-auto">
            Running...
          </span>
        )}

        {tool.output && (
          <button className="ml-auto text-muted-foreground hover:text-foreground">
            {expanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
          </button>
        )}
      </div>

      {/* Tool description - shown when no file path */}
      {!filePath && tool.description && (
        <div className="px-3 py-2 border-t border-border/30">
          <p className="text-xs text-muted-foreground">
            {tool.description}
          </p>
        </div>
      )}

      {/* Tool output - expandable */}
      {tool.output && expanded && (
        <div className="border-t border-border/30 bg-background/30">
          <pre className={cn(
            "text-xs font-mono overflow-x-auto whitespace-pre-wrap p-3",
            isFileOperation || isBashCommand ? "max-h-96" : "max-h-48",
            "overflow-y-auto text-muted-foreground/90"
          )}>
            {hasLongOutput && !expanded
              ? tool.output.slice(0, outputLimit) + "\n...(truncated)"
              : tool.output}
          </pre>
        </div>
      )}

      {/* Collapsed preview for output */}
      {tool.output && !expanded && (
        <div className="px-3 py-1.5 border-t border-border/30 bg-background/20">
          <p className="text-[10px] text-muted-foreground/60 truncate">
            {tool.output.split('\n')[0]?.slice(0, 80) || 'Click to expand output'}
          </p>
        </div>
      )}
    </div>
  );
}

export function MessageList() {
  const messages = useSessionStore((s) => s.messages);
  const streamingMessage = useSessionStore((s) => s.streamingMessage);
  const thinkingMessage = useSessionStore((s) => s.thinkingMessage);
  const isProcessing = useSessionStore((s) => s.isProcessing);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [showScrollButton, setShowScrollButton] = useState(false);
  const isAutoScrolling = useRef(false);
  const isNearBottomRef = useRef(true);
  const lastScrollHeight = useRef(0);

  // Check if user is near bottom of scroll container
  const checkIfNearBottom = useCallback(() => {
    if (!scrollRef.current) return true;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    const threshold = 100;
    return scrollHeight - scrollTop - clientHeight < threshold;
  }, []);

  // Handle scroll events to detect user scrolling
  const handleScroll = useCallback(() => {
    if (isAutoScrolling.current) return;

    const nearBottom = checkIfNearBottom();
    isNearBottomRef.current = nearBottom;
    setShowScrollButton(!nearBottom && (isProcessing || streamingMessage !== null));
  }, [checkIfNearBottom, isProcessing, streamingMessage]);

  // Smooth scroll to bottom
  const scrollToBottom = useCallback((instant = false) => {
    if (!scrollRef.current) return;

    isAutoScrolling.current = true;

    scrollRef.current.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: instant ? "instant" : "smooth",
    });

    setTimeout(() => {
      isAutoScrolling.current = false;
      isNearBottomRef.current = true;
      setShowScrollButton(false);
    }, instant ? 0 : 150);
  }, []);

  // Auto-scroll on new content only if user is near bottom
  useEffect(() => {
    if (!scrollRef.current) return;

    const currentScrollHeight = scrollRef.current.scrollHeight;
    const hasNewContent = currentScrollHeight > lastScrollHeight.current;
    lastScrollHeight.current = currentScrollHeight;

    // Only auto-scroll if near bottom and there's new content
    if (isNearBottomRef.current && hasNewContent) {
      requestAnimationFrame(() => {
        scrollToBottom(false);
      });
    }
  }, [messages, streamingMessage?.content, streamingMessage?.toolExecutions?.length, thinkingMessage, scrollToBottom]);

  // Force scroll to bottom when user sends a new message
  useEffect(() => {
    if (messages.length > 0) {
      const lastMessage = messages[messages.length - 1];
      if (lastMessage.role === "user") {
        scrollToBottom(true);
        isNearBottomRef.current = true;
      }
    }
  }, [messages.length, scrollToBottom]);

  return (
    <div
      ref={scrollRef}
      className="h-full overflow-y-auto scroll-smooth"
      onScroll={handleScroll}
    >
      <div className="max-w-4xl mx-auto py-4 px-4 space-y-4 pb-8">
        {/* Existing messages - filter out empty ones */}
        {messages
          .filter((message) => {
            // Skip messages with no content and no tool executions
            const hasContent = message.content && message.content.trim().length > 0;
            const hasToolExecutions = message.toolExecutions && message.toolExecutions.length > 0;
            return hasContent || hasToolExecutions;
          })
          .map((message) =>
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
                {streamingMessage.toolExecutions.map((tool) => (
                  <ToolExecutionCard key={tool.id} tool={tool} />
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

      {/* Scroll to bottom button */}
      {showScrollButton && (
        <button
          onClick={() => scrollToBottom(false)}
          className={cn(
            "fixed bottom-24 left-1/2 -translate-x-1/2 z-50",
            "flex items-center gap-2 px-3 py-2 rounded-full",
            "bg-primary/90 text-primary-foreground shadow-lg",
            "hover:bg-primary transition-all duration-200",
            "animate-in fade-in-0 slide-in-from-bottom-4 duration-200"
          )}
        >
          <ArrowDown className="w-4 h-4" />
          <span className="text-xs font-medium">New content</span>
        </button>
      )}
    </div>
  );
}
