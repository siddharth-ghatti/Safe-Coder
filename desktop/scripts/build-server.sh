#!/bin/bash
# Build the safe-coder server binary for bundling with Tauri

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$DESKTOP_DIR")"
TARGET_DIR="$DESKTOP_DIR/src-tauri/binaries"

echo "Building safe-coder server..."

# Get the target triple for the current platform
get_target_triple() {
    case "$(uname -s)" in
        Darwin)
            case "$(uname -m)" in
                arm64) echo "aarch64-apple-darwin" ;;
                x86_64) echo "x86_64-apple-darwin" ;;
            esac
            ;;
        Linux)
            case "$(uname -m)" in
                x86_64) echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "x86_64-pc-windows-msvc"
            ;;
    esac
}

TARGET_TRIPLE=$(get_target_triple)

# Create binaries directory
mkdir -p "$TARGET_DIR"

# Build safe-coder in release mode
cd "$PROJECT_ROOT"
echo "Building safe-coder for $TARGET_TRIPLE..."
cargo build --release

# Copy binary to Tauri binaries directory with target triple suffix
# Tauri expects: binary-name-target-triple[.exe]
SOURCE_BINARY="$PROJECT_ROOT/target/release/safe-coder"
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    SOURCE_BINARY="$SOURCE_BINARY.exe"
    DEST_BINARY="$TARGET_DIR/safe-coder-$TARGET_TRIPLE.exe"
else
    DEST_BINARY="$TARGET_DIR/safe-coder-$TARGET_TRIPLE"
fi

echo "Copying binary to $DEST_BINARY..."
cp "$SOURCE_BINARY" "$DEST_BINARY"
chmod +x "$DEST_BINARY"

echo "Server binary built successfully: $DEST_BINARY"
