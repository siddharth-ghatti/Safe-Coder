import type { Message } from "../../types";

interface UserMessageProps {
  message: Message;
}

export function UserMessage({ message }: UserMessageProps) {
  return (
    <div className="py-2">
      <div className="inline-block bg-muted/80 border border-border/50 rounded-lg px-4 py-2.5 max-w-[85%]">
        <p className="text-sm text-foreground whitespace-pre-wrap break-words">
          {message.content}
        </p>
      </div>
    </div>
  );
}
