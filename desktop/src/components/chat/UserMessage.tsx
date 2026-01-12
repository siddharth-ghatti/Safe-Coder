import type { Message } from "../../types";
import { User } from "lucide-react";

interface UserMessageProps {
  message: Message;
}

export function UserMessage({ message }: UserMessageProps) {
  return (
    <div className="flex gap-3">
      <div className="w-8 h-8 rounded-full bg-secondary flex items-center justify-center flex-shrink-0">
        <User className="w-4 h-4 text-secondary-foreground" />
      </div>
      <div className="flex-1 min-w-0">
        <div className="bg-muted rounded-lg p-3">
          <p className="text-sm text-foreground whitespace-pre-wrap break-words">
            {message.content}
          </p>
        </div>
      </div>
    </div>
  );
}
