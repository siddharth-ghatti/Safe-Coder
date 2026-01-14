import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import type { Message } from "../../types";
import { Bot } from "lucide-react";
import { cn } from "../../lib/utils";

interface AssistantMessageProps {
  message: Message;
  isStreaming?: boolean;
}

export function AssistantMessage({ message, isStreaming }: AssistantMessageProps) {
  const contentRef = useRef<HTMLDivElement>(null);
  const prevLengthRef = useRef(0);

  // Add smooth scroll and highlight effect for new content
  useEffect(() => {
    if (isStreaming && contentRef.current) {
      const newLength = message.content.length;
      if (newLength > prevLengthRef.current) {
        // Scroll the last element into view smoothly
        const lastChild = contentRef.current.lastElementChild;
        if (lastChild) {
          lastChild.scrollIntoView({ behavior: "smooth", block: "nearest" });
        }
      }
      prevLengthRef.current = newLength;
    }
  }, [message.content, isStreaming]);

  return (
    <div className="flex gap-3 animate-in fade-in-0 slide-in-from-bottom-2 duration-300">
      <div className={cn(
        "w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0 transition-all duration-300",
        isStreaming && "ring-2 ring-primary/30 ring-offset-2 ring-offset-background"
      )}>
        <Bot className={cn(
          "w-4 h-4 text-primary transition-transform duration-300",
          isStreaming && "animate-pulse"
        )} />
      </div>
      <div className="flex-1 min-w-0">
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
                      className="bg-muted px-1.5 py-0.5 rounded text-sm font-mono"
                      {...props}
                    >
                      {children}
                    </code>
                  );
                }

                return (
                  <SyntaxHighlighter
                    style={oneDark as { [key: string]: React.CSSProperties }}
                    language={match[1]}
                    PreTag="div"
                    customStyle={{
                      margin: 0,
                      borderRadius: "0.5rem",
                      fontSize: "0.875rem",
                    }}
                  >
                    {String(children).replace(/\n$/, "")}
                  </SyntaxHighlighter>
                );
              },
              p({ children }) {
                return (
                  <p className="text-sm text-foreground mb-2 last:mb-0">
                    {children}
                  </p>
                );
              },
              ul({ children }) {
                return <ul className="list-disc list-inside mb-2">{children}</ul>;
              },
              ol({ children }) {
                return <ol className="list-decimal list-inside mb-2">{children}</ol>;
              },
              li({ children }) {
                return <li className="text-sm text-foreground">{children}</li>;
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
            }}
          >
            {message.content}
          </ReactMarkdown>
        </div>
      </div>
    </div>
  );
}
