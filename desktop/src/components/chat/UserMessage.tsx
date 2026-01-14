import type { Message } from "../../types";
import { User } from "lucide-react";

interface UserMessageProps {
  message: Message;
}

export function UserMessage({ message }: UserMessageProps) {
  return (
    <div className="flex justify-end animate-in fade-in-0 slide-in-from-bottom-2 duration-300">
      <div className="flex gap-3 max-w-[85%]">
        <div className="flex-1 min-w-0">
          <div className="bg-primary/10 border border-primary/20 rounded-2xl rounded-tr-sm px-4 py-3">
            <p className="text-sm text-foreground whitespace-pre-wrap break-words">
              {message.content}
            </p>
          </div>
        </div>
        <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center flex-shrink-0">
          <User className="w-4 h-4 text-primary" />
        </div>
      </div>
    </div>
  );
}
