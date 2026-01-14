import { useEffect, useRef } from "react";
import { subscribeToEvents } from "../api/client";
import { useSessionStore } from "../stores/sessionStore";
import type { ServerEvent } from "../types";

export function useSessionEvents(sessionId: string | null) {
  const cleanupRef = useRef<(() => void) | null>(null);

  // Use refs to avoid re-subscribing when callbacks change
  const handleServerEventRef = useRef(useSessionStore.getState().handleServerEvent);
  const setIsConnectedRef = useRef(useSessionStore.getState().setIsConnected);

  // Keep refs updated
  useEffect(() => {
    handleServerEventRef.current = useSessionStore.getState().handleServerEvent;
    setIsConnectedRef.current = useSessionStore.getState().setIsConnected;
  });

  useEffect(() => {
    // Cleanup previous subscription
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }

    if (!sessionId) {
      setIsConnectedRef.current(false);
      return;
    }

    // Subscribe to events
    const cleanup = subscribeToEvents(
      sessionId,
      (event) => {
        const data = event.data as Record<string, unknown> || {};
        const serverEvent = { type: event.type, ...data } as ServerEvent;
        handleServerEventRef.current(serverEvent);
      },
      () => {
        setIsConnectedRef.current(false);
      }
    );

    cleanupRef.current = cleanup;

    return () => {
      if (cleanupRef.current) {
        cleanupRef.current();
        cleanupRef.current = null;
      }
    };
  }, [sessionId]); // Only depend on sessionId
}
