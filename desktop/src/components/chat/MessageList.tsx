import { useRef, useEffect, useState, useCallback } from "react";
import { Check, X, Loader2, ArrowDown, ChevronDown, ChevronRight, Circle, Bot } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { UserMessage } from "./UserMessage";
import { AssistantMessage } from "./AssistantMessage";
import { ThinkingIndicator } from "./ThinkingIndicator";
import { cn } from "../../lib/utils";
import type { ToolExecution, TodoItem } from "../../types";

// Todo list display component - compact inline checklist
function TodoListDisplay({ todos }: { todos: TodoItem[] }) {
  if (todos.length === 0) return null;

  const completed = todos.filter(t => t.status === "completed").length;
  const total = todos.length;

  return (
    <div className="border-l-2 border-border/50 pl-3 py-1 my-2">
      <div className="text-[10px] text-muted-foreground mb-1">
        {completed}/{total} tasks
      </div>
      <div className="space-y-0.5">
        {todos.map((todo, idx) => (
          <div
            key={idx}
            className={cn(
              "flex items-center gap-1.5 text-xs",
              todo.status === "completed" && "text-muted-foreground/60",
              todo.status === "in_progress" && "text-foreground"
            )}
          >
            {todo.status === "completed" ? (
              <Check className="w-3 h-3 text-muted-foreground/60 flex-shrink-0" />
            ) : todo.status === "in_progress" ? (
              <Loader2 className="w-3 h-3 text-primary animate-spin flex-shrink-0" />
            ) : (
              <Circle className="w-3 h-3 text-muted-foreground/40 flex-shrink-0" />
            )}
            <span className={todo.status === "completed" ? "line-through" : ""}>
              {todo.status === "in_progress" && todo.active_form
                ? todo.active_form
                : todo.content}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// Generate smart summary for tool output (Claude Code style)
function getToolSummary(toolName: string, output: string | undefined): string {
  if (!output) return "";
  const lines = output.split('\n').filter(l => l.trim());
  const lineCount = lines.length;

  switch (toolName.toLowerCase()) {
    case "read_file":
    case "read":
      return `Read ${lineCount} lines`;
    case "edit_file":
    case "edit":
    case "write_file":
    case "write":
      return lineCount > 0 ? `${lineCount} lines` : "Updated";
    case "bash":
      return lineCount === 0 ? "Completed" : `${lineCount} lines`;
    case "grep":
    case "glob":
    case "code_search":
      return lineCount === 0 ? "No matches" : `${lineCount} matches`;
    default:
      return lineCount === 0 ? "Done" : `${lineCount} lines`;
  }
}

// Tool execution card - Claude Code style (compact with summary)
function ToolExecutionCard({ tool }: { tool: ToolExecution }) {
  const [expanded, setExpanded] = useState(false);

  // The description IS the target (file path, pattern, command, etc.)
  // It may or may not have backticks depending on source
  const target = tool.description?.replace(/`/g, '') || "";

  // Get summary and preview
  const summary = getToolSummary(tool.name, tool.output);
  const outputLines = tool.output?.split('\n').filter(l => l.trim()) || [];
  const previewLines = outputLines.slice(0, 3);
  const hasMoreLines = outputLines.length > 3;

  // Format tool name for display
  const displayName = tool.name.replace(/_/g, ' ');

  return (
    <div className="mb-2">
      {/* Reasoning before tool call - with bot icon */}
      {tool.reasoning && (
        <div className="flex gap-3 mb-2">
          <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
            <Bot className="w-4 h-4 text-primary" />
          </div>
          <div className="flex-1 py-1">
            <p className="text-sm text-foreground leading-relaxed">
              {tool.reasoning}
            </p>
          </div>
        </div>
      )}

      {/* Tool card with border */}
      <div className="border border-border/40 rounded-md overflow-hidden bg-muted/10">
        {/* Tool header: status + name + target */}
        <div
          className="flex items-center gap-2 px-2 py-1.5 cursor-pointer hover:bg-muted/20 transition-colors"
          onClick={() => tool.output && setExpanded(!expanded)}
        >
          {tool.success === undefined ? (
            <Loader2 className="w-3.5 h-3.5 text-primary animate-spin flex-shrink-0" />
          ) : tool.success ? (
            <Check className="w-3.5 h-3.5 text-primary flex-shrink-0" />
          ) : (
            <X className="w-3.5 h-3.5 text-destructive flex-shrink-0" />
          )}

          <span className="text-xs font-medium text-foreground">
            {displayName}
          </span>

          {target && (
            <code className="text-xs text-muted-foreground font-mono truncate max-w-[300px]" title={target}>
              {target}
            </code>
          )}

          {tool.success === undefined && (
            <span className="text-[10px] text-muted-foreground ml-auto">
              Running...
            </span>
          )}

          {tool.output && (
            <span className="ml-auto text-muted-foreground flex items-center gap-1">
              {summary && <span className="text-[10px]">{summary}</span>}
              {expanded ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
            </span>
          )}
        </div>

        {/* Preview lines (collapsed) - always show when there's output */}
        {tool.output && !expanded && previewLines.length > 0 && (
          <div className="px-2 py-1.5 border-t border-border/30 bg-background/50">
            {previewLines.map((line, i) => (
              <div key={i} className="text-[11px] text-muted-foreground/70 font-mono truncate">
                {line.slice(0, 100)}
              </div>
            ))}
            {hasMoreLines && (
              <div className="text-[10px] text-muted-foreground/50 mt-0.5">
                ... +{outputLines.length - 3} more lines
              </div>
            )}
          </div>
        )}

        {/* Expanded output */}
        {tool.output && expanded && (
          <div className="px-2 py-1.5 border-t border-border/30 bg-background/50">
            <pre className="text-[11px] font-mono overflow-x-auto whitespace-pre-wrap max-h-64 overflow-y-auto text-muted-foreground/80">
              {tool.output}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
}

// Render tools in order (reasoning is attached to each tool via tool.reasoning)
function ToolsDisplay({ toolExecutions }: { toolExecutions: ToolExecution[] }) {
  return (
    <div className="space-y-1">
      {toolExecutions.map((tool) => (
        <ToolExecutionCard key={tool.id} tool={tool} />
      ))}
    </div>
  );
}

export function MessageList() {
  const messages = useSessionStore((s) => s.messages);
  const streamingMessage = useSessionStore((s) => s.streamingMessage);
  const thinkingMessage = useSessionStore((s) => s.thinkingMessage);
  const isProcessing = useSessionStore((s) => s.isProcessing);
  const todoList = useSessionStore((s) => s.todoList);
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
              {/* Show todo list alongside thinking message */}
              {todoList.length > 0 && <TodoListDisplay todos={todoList} />}
            </div>
          </div>
        )}

        {/* Streaming message with tool executions */}
        {streamingMessage && (
          <div className="space-y-2">
            {/* Render tools with their reasoning (interleaved) */}
            {streamingMessage.toolExecutions.length > 0 && (
              <ToolsDisplay toolExecutions={streamingMessage.toolExecutions} />
            )}

            {/* Todo list display */}
            {todoList.length > 0 && <TodoListDisplay todos={todoList} />}

            {/* Final text content AFTER tools - pass toolExecutions so it can filter redundant reasoning */}
            {streamingMessage.content && (
              <AssistantMessage
                message={{
                  id: "streaming",
                  role: "assistant",
                  content: streamingMessage.content,
                  timestamp: new Date().toISOString(),
                  toolExecutions: streamingMessage.toolExecutions.length > 0
                    ? streamingMessage.toolExecutions
                    : undefined,
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
          <>
            <ThinkingIndicator />
            {/* Show todo list even during initial thinking state */}
            {todoList.length > 0 && <TodoListDisplay todos={todoList} />}
          </>
        )}

        {/* Persistent todo list display - shown when there are todos but no active streaming */}
        {!isProcessing && !streamingMessage && todoList.length > 0 && (
          <TodoListDisplay todos={todoList} />
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
