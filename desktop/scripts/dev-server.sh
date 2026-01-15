#!/bin/bash
# Start the safe-coder server for development

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$DESKTOP_DIR")"

PORT=${SAFE_CODER_PORT:-9876}
PID_FILE="$DESKTOP_DIR/.server.pid"

# Function to cleanup on exit
cleanup() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if kill -0 "$PID" 2>/dev/null; then
            echo "Stopping safe-coder server (PID: $PID)..."
            kill "$PID" 2>/dev/null || true
        fi
        rm -f "$PID_FILE"
    fi
}

# Set up trap for cleanup
trap cleanup EXIT INT TERM

# Check if server is already running on port
if curl -s "http://127.0.0.1:$PORT/api/health" > /dev/null 2>&1; then
    echo "Server already running on port $PORT"
    # Keep script alive while server is running
    while curl -s "http://127.0.0.1:$PORT/api/health" > /dev/null 2>&1; do
        sleep 5
    done
    exit 0
fi

# Check if we have a stale PID file
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if ! kill -0 "$PID" 2>/dev/null; then
        rm -f "$PID_FILE"
    fi
fi

# Build and run safe-coder server
cd "$PROJECT_ROOT"

# Check if binary exists, if not build it
if [ ! -f "target/release/safe-coder" ]; then
    echo "Building safe-coder..."
    cargo build --release
fi

echo "Starting safe-coder server on port $PORT..."
./target/release/safe-coder serve --port "$PORT" --cors &
SERVER_PID=$!

# Save PID
echo "$SERVER_PID" > "$PID_FILE"

echo "Server started (PID: $SERVER_PID)"

# Wait for server to be ready
echo "Waiting for server to be ready..."
for i in {1..30}; do
    if curl -s "http://127.0.0.1:$PORT/api/health" > /dev/null 2>&1; then
        echo "Server is ready!"
        break
    fi
    sleep 0.5
done

# Keep the script running and wait for the server process
# This prevents the script from exiting and triggering cleanup
wait $SERVER_PID
