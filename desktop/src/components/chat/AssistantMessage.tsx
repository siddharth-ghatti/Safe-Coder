import { useRef, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import type { Message, ToolExecution } from "../../types";
import { Bot, Copy, Check, FileCode, ChevronDown, ChevronRight, Wrench, CheckCircle2, XCircle } from "lucide-react";
import { cn } from "../../lib/utils";

interface AssistantMessageProps {
  message: Message;
  isStreaming?: boolean;
}

// Collapsible tool execution history
function ToolExecutionHistory({ executions }: { executions: ToolExecution[] }) {
  const [expanded, setExpanded] = useState(false);

  if (executions.length === 0) return null;

  const successCount = executions.filter(t => t.success === true).length;
  const failCount = executions.filter(t => t.success === false).length;

  return (
    <div className="mb-3 rounded-lg border border-border/50 overflow-hidden bg-muted/20">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 hover:bg-muted/30 transition-colors"
      >
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="w-4 h-4 text-muted-foreground" />
        )}
        <Wrench className="w-4 h-4 text-muted-foreground" />
        <span className="text-xs text-muted-foreground flex-1 text-left">
          {executions.length} tool{executions.length !== 1 ? 's' : ''} executed
        </span>
        <div className="flex items-center gap-2 text-xs">
          {successCount > 0 && (
            <span className="flex items-center gap-1 text-success">
              <CheckCircle2 className="w-3 h-3" />
              {successCount}
            </span>
          )}
          {failCount > 0 && (
            <span className="flex items-center gap-1 text-destructive">
              <XCircle className="w-3 h-3" />
              {failCount}
            </span>
          )}
        </div>
      </button>

      {expanded && (
        <div className="border-t border-border/30 divide-y divide-border/30">
          {executions.map((tool) => (
            <div key={tool.id} className="px-3 py-2">
              <div className="flex items-center gap-2">
                {tool.success === true ? (
                  <CheckCircle2 className="w-3.5 h-3.5 text-success" />
                ) : tool.success === false ? (
                  <XCircle className="w-3.5 h-3.5 text-destructive" />
                ) : (
                  <div className="w-3.5 h-3.5 rounded-full bg-muted-foreground/30" />
                )}
                <span className="text-xs font-medium text-foreground">{tool.name}</span>
                <span className="text-xs text-muted-foreground truncate flex-1">{tool.description}</span>
              </div>
              {tool.output && (
                <pre className="mt-2 text-xs text-muted-foreground bg-background/50 rounded p-2 overflow-x-auto max-h-32 overflow-y-auto">
                  {tool.output.slice(0, 500)}{tool.output.length > 500 ? '...' : ''}
                </pre>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// Code block with header and copy button
function CodeBlock({ language, children }: { language: string; children: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(children);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="rounded-lg border border-border/50 overflow-hidden my-3 bg-[#1e1e1e]">
      {/* Code block header */}
      <div className="flex items-center justify-between px-3 py-2 bg-muted/30 border-b border-border/30">
        <div className="flex items-center gap-2">
          <FileCode className="w-3.5 h-3.5 text-muted-foreground" />
          <span className="text-xs text-muted-foreground font-medium">{language}</span>
        </div>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1.5 px-2 py-1 text-xs text-muted-foreground hover:text-foreground rounded transition-colors hover:bg-muted/50"
        >
          {copied ? (
            <>
              <Check className="w-3 h-3 text-success" />
              <span className="text-success">Copied</span>
            </>
          ) : (
            <>
              <Copy className="w-3 h-3" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      {/* Code content */}
      <SyntaxHighlighter
        style={oneDark as { [key: string]: React.CSSProperties }}
        language={language}
        PreTag="div"
        customStyle={{
          margin: 0,
          padding: "1rem",
          background: "transparent",
          fontSize: "0.8125rem",
          lineHeight: "1.5",
        }}
      >
        {children}
      </SyntaxHighlighter>
    </div>
  );
}

export function AssistantMessage({ message, isStreaming }: AssistantMessageProps) {
  const contentRef = useRef<HTMLDivElement>(null);

  // Memoize content to prevent unnecessary re-renders
  const content = useMemo(() => message.content, [message.content]);

  return (
    <div className={cn(
      "flex gap-3",
      !isStreaming && "animate-in fade-in-0 slide-in-from-bottom-2 duration-300"
    )}>
      <div className={cn(
        "w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0 transition-all duration-200",
        isStreaming && "ring-2 ring-primary/40 ring-offset-2 ring-offset-background shadow-[0_0_12px_rgba(34,197,94,0.3)]"
      )}>
        <Bot className={cn(
          "w-4 h-4 text-primary transition-all duration-200",
          isStreaming && "scale-110"
        )} />
      </div>
      <div className="flex-1 min-w-0">
        {/* Show tool executions from message history */}
        {message.toolExecutions && message.toolExecutions.length > 0 && (
          <ToolExecutionHistory executions={message.toolExecutions} />
        )}
        <div
          ref={contentRef}
          className={cn(
            "prose prose-invert prose-sm max-w-none",
            isStreaming && "streaming-content"
          )}
        >
          <ReactMarkdown
            components={{
              code({ node, className, children, ...props }) {
                const match = /language-(\w+)/.exec(className || "");
                const isInline = !match;

                if (isInline) {
                  return (
                    <code
                      className="bg-muted/80 text-primary/90 px-1.5 py-0.5 rounded text-[13px] font-mono"
                      {...props}
                    >
                      {children}
                    </code>
                  );
                }

                return (
                  <CodeBlock language={match[1]}>
                    {String(children).replace(/\n$/, "")}
                  </CodeBlock>
                );
              },
              pre({ children }) {
                // Let the code component handle the styling
                return <>{children}</>;
              },
              p({ children }) {
                return (
                  <p className="text-sm text-foreground mb-3 last:mb-0 leading-relaxed">
                    {children}
                  </p>
                );
              },
              h1({ children }) {
                return <h1 className="text-lg font-semibold text-foreground mt-4 mb-2">{children}</h1>;
              },
              h2({ children }) {
                return <h2 className="text-base font-semibold text-foreground mt-4 mb-2">{children}</h2>;
              },
              h3({ children }) {
                return <h3 className="text-sm font-semibold text-foreground mt-3 mb-1.5">{children}</h3>;
              },
              ul({ children }) {
                return <ul className="list-disc list-outside ml-4 mb-3 space-y-1">{children}</ul>;
              },
              ol({ children }) {
                return <ol className="list-decimal list-outside ml-4 mb-3 space-y-1">{children}</ol>;
              },
              li({ children }) {
                return <li className="text-sm text-foreground leading-relaxed">{children}</li>;
              },
              a({ href, children }) {
                return (
                  <a
                    href={href}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-primary hover:underline"
                  >
                    {children}
                  </a>
                );
              },
              blockquote({ children }) {
                return (
                  <blockquote className="border-l-2 border-primary/50 pl-3 my-3 text-muted-foreground italic">
                    {children}
                  </blockquote>
                );
              },
              hr() {
                return <hr className="border-border/50 my-4" />;
              },
              strong({ children }) {
                return <strong className="font-semibold text-foreground">{children}</strong>;
              },
            }}
          >
            {content}
          </ReactMarkdown>
          {isStreaming && (
            <span className="streaming-cursor" aria-hidden="true" />
          )}
        </div>
      </div>
    </div>
  );
}
