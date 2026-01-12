import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppLayout } from "./components/layout/AppLayout";
import { Sidebar } from "./components/layout/Sidebar";
import { ChatPanel } from "./components/layout/ChatPanel";
import { ChangesPanel } from "./components/layout/ChangesPanel";
import { useSessionStore } from "./stores/sessionStore";
import { useSessionEvents } from "./hooks/useSSE";
import * as api from "./api/client";

const DEFAULT_PORT = 9876;

function App() {
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const loadSessions = useSessionStore((s) => s.loadSessions);
  const [serverStatus, setServerStatus] = useState<"starting" | "ready" | "error">("starting");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Subscribe to SSE events for the active session
  useSessionEvents(activeSessionId);

  // Start server and load sessions on mount
  useEffect(() => {
    const init = async () => {
      try {
        setServerStatus("starting");

        // Check if we're running in Tauri (desktop) or web
        const isTauri = "__TAURI__" in window;

        if (isTauri) {
          // Try to start or connect to the server via Tauri command
          try {
            const result = await invoke<string>("start_server", { port: DEFAULT_PORT });
            console.log("Server status:", result);
          } catch (err) {
            console.warn("Could not start server via Tauri:", err);
            // Server might already be running (e.g., started by npm run dev)
          }
        }

        // Wait for server to be ready
        let attempts = 0;
        const maxAttempts = 30;

        while (attempts < maxAttempts) {
          try {
            const health = await api.checkHealth();
            console.log("Server health:", health);
            setServerStatus("ready");
            await loadSessions();
            return;
          } catch {
            attempts++;
            await new Promise((resolve) => setTimeout(resolve, 500));
          }
        }

        throw new Error("Server did not become ready in time");
      } catch (error) {
        console.error("Failed to connect to server:", error);
        setServerStatus("error");
        setErrorMessage(error instanceof Error ? error.message : "Unknown error");
      }
    };

    init();
  }, [loadSessions]);

  // Show loading screen while server is starting
  if (serverStatus === "starting") {
    return (
      <div className="h-screen w-screen bg-background flex items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4"></div>
          <p className="text-muted-foreground">Starting Safe Coder server...</p>
        </div>
      </div>
    );
  }

  // Show error screen if server failed to start
  if (serverStatus === "error") {
    return (
      <div className="h-screen w-screen bg-background flex items-center justify-center">
        <div className="text-center max-w-md p-6">
          <div className="text-destructive text-4xl mb-4">⚠</div>
          <h1 className="text-xl font-semibold text-foreground mb-2">
            Failed to Start Server
          </h1>
          <p className="text-muted-foreground mb-4">
            {errorMessage || "Could not connect to the Safe Coder server."}
          </p>
          <button
            onClick={() => window.location.reload()}
            className="px-4 py-2 bg-primary text-primary-foreground rounded hover:bg-primary/90"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <AppLayout
      sidebar={<Sidebar />}
      main={<ChatPanel />}
      changes={<ChangesPanel />}
    />
  );
}

export default App;
