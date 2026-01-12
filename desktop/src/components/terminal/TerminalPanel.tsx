import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { X } from "lucide-react";
import { useSessionStore } from "../../stores/sessionStore";
import { useUIStore } from "../../stores/uiStore";
import { getBaseUrl } from "../../api/client";
import "@xterm/xterm/css/xterm.css";

export function TerminalPanel() {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const toggleTerminal = useUIStore((s) => s.toggleTerminal);

  useEffect(() => {
    if (!terminalRef.current || !activeSessionId) return;

    // Create xterm instance
    const term = new XTerm({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: "'JetBrains Mono', Menlo, Monaco, monospace",
      scrollback: 10000,
      scrollOnUserInput: true,
      theme: {
        background: "#0d0d0d",
        foreground: "#e5e5e5",
        cursor: "#22c55e",
        cursorAccent: "#0d0d0d",
        selectionBackground: "rgba(34, 197, 94, 0.3)",
        black: "#0d0d0d",
        red: "#ef4444",
        green: "#22c55e",
        yellow: "#f59e0b",
        blue: "#3b82f6",
        magenta: "#a855f7",
        cyan: "#06b6d4",
        white: "#e5e5e5",
        brightBlack: "#525252",
        brightRed: "#f87171",
        brightGreen: "#4ade80",
        brightYellow: "#fbbf24",
        brightBlue: "#60a5fa",
        brightMagenta: "#c084fc",
        brightCyan: "#22d3ee",
        brightWhite: "#ffffff",
      },
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);
    term.open(terminalRef.current);

    // Fit to container
    fitAddon.fit();

    xtermRef.current = term;
    fitAddonRef.current = fitAddon;

    // Connect to PTY WebSocket
    const baseUrl = getBaseUrl();
    const wsUrl = baseUrl.replace("http", "ws");
    const ws = new WebSocket(`${wsUrl}/api/sessions/${activeSessionId}/pty`);

    ws.binaryType = "arraybuffer";

    ws.onopen = () => {
      term.writeln("Connected to terminal");
      term.writeln("");

      // Send initial resize
      const { cols, rows } = term;
      ws.send(`resize:${cols}:${rows}`);
    };

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        const data = new Uint8Array(event.data);
        term.write(data);
      } else {
        term.write(event.data);
      }
      // Auto-scroll to bottom on new output
      term.scrollToBottom();
    };

    ws.onerror = (error) => {
      console.error("WebSocket error:", error);
      term.writeln("\r\n\x1b[31mConnection error\x1b[0m");
    };

    ws.onclose = () => {
      term.writeln("\r\n\x1b[33mConnection closed\x1b[0m");
    };

    wsRef.current = ws;

    // Send user input to server
    term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) {
        const encoder = new TextEncoder();
        ws.send(encoder.encode(data));
      }
    });

    // Handle resize
    const handleResize = () => {
      fitAddon.fit();
      if (ws.readyState === WebSocket.OPEN) {
        const { cols, rows } = term;
        ws.send(`resize:${cols}:${rows}`);
      }
    };

    window.addEventListener("resize", handleResize);

    // Cleanup
    return () => {
      window.removeEventListener("resize", handleResize);
      ws.close();
      term.dispose();
    };
  }, [activeSessionId]);

  // Refit on visibility change
  useEffect(() => {
    if (fitAddonRef.current) {
      fitAddonRef.current.fit();
    }
  }, []);

  return (
    <div className="h-64 border-t border-border bg-background flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-1 border-b border-border bg-card">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">Terminal</span>
        </div>
        <button
          onClick={toggleTerminal}
          className="p-1 text-muted-foreground hover:text-foreground"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Terminal */}
      <div ref={terminalRef} className="flex-1" />
    </div>
  );
}
