#!/bin/bash
# Stop the safe-coder development server

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_DIR="$(dirname "$SCRIPT_DIR")"
PID_FILE="$DESKTOP_DIR/.server.pid"

if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        echo "Stopping safe-coder server (PID: $PID)..."
        kill "$PID" 2>/dev/null || true
        rm -f "$PID_FILE"
        echo "Server stopped"
    else
        echo "Server not running"
        rm -f "$PID_FILE"
    fi
else
    echo "No server PID file found"
fi
