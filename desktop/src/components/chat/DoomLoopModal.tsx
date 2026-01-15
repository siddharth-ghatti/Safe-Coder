import { AlertTriangle, XCircle, PlayCircle } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";

export function DoomLoopModal() {
  const doomLoopPrompt = useSessionStore((s) => s.doomLoopPrompt);
  const respondToDoomLoop = useSessionStore((s) => s.respondToDoomLoop);

  if (!doomLoopPrompt) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-background border border-border rounded-lg shadow-xl max-w-md w-full mx-4 animate-in fade-in-0 zoom-in-95 duration-200">
        {/* Header */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border bg-destructive/10">
          <AlertTriangle className="w-5 h-5 text-destructive" />
          <h2 className="text-sm font-semibold text-destructive">Potential Loop Detected</h2>
        </div>

        {/* Content */}
        <div className="p-4 space-y-3">
          <p className="text-sm text-muted-foreground">
            {doomLoopPrompt.message}
          </p>
          <p className="text-xs text-muted-foreground/70">
            The agent appears to be repeating similar actions. This may indicate it's stuck in a loop.
          </p>
        </div>

        {/* Actions */}
        <div className="flex gap-2 px-4 py-3 border-t border-border bg-muted/20">
          <button
            onClick={() => respondToDoomLoop(false)}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-md bg-muted hover:bg-muted/80 text-sm font-medium transition-colors"
          >
            <XCircle className="w-4 h-4" />
            Stop
          </button>
          <button
            onClick={() => respondToDoomLoop(true)}
            className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-md bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-medium transition-colors"
          >
            <PlayCircle className="w-4 h-4" />
            Continue Anyway
          </button>
        </div>
      </div>
    </div>
  );
}
