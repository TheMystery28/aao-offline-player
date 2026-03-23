#!/usr/bin/env bash
# Run AAO engine regression tests.
#
# Usage:
#   1. Start the app:  npm run tauri dev
#   2. In another terminal:  bash scripts/run-tests.sh
#
# The script:
#   - Syncs engine/tests/ into the dev resource directory
#   - Reads the server port from .server_port
#   - Opens the test runner in the default browser

set -euo pipefail
cd "$(dirname "$0")/.."

# --- Sync tests into the dev resource directory ---
# In dev mode, Tauri serves from target/debug/engine/ (not source engine/).
# The tests/ dir isn't in tauri.conf.json resources, so we sync it manually.
TARGETS=(
    "src-tauri/target/debug/engine/tests"
    "src-tauri/target/release/engine/tests"
)

synced=false
for target_dir in "${TARGETS[@]}"; do
    parent="$(dirname "$target_dir")"
    if [ -d "$parent" ]; then
        # Use cp -r to sync (overwrite existing)
        rm -rf "$target_dir"
        cp -r engine/tests "$target_dir"
        echo "[run-tests] Synced engine/tests/ -> $target_dir"
        synced=true
    fi
done

if [ "$synced" = false ]; then
    echo "[run-tests] WARNING: No target/*/engine/ directory found."
    echo "            Make sure 'npm run tauri dev' has been run at least once."
fi

# --- Find the server port ---
PORT_FILE=""
for pf in "src-tauri/target/debug/engine/.server_port" "src-tauri/target/release/engine/.server_port"; do
    if [ -f "$pf" ]; then
        PORT_FILE="$pf"
        break
    fi
done

if [ -z "$PORT_FILE" ]; then
    echo "[run-tests] ERROR: .server_port file not found."
    echo "            Start the app first: npm run tauri dev"
    exit 1
fi

PORT=$(cat "$PORT_FILE")
if [ -z "$PORT" ]; then
    echo "[run-tests] ERROR: .server_port file is empty."
    exit 1
fi

URL="http://localhost:${PORT}/tests/test_runner.html"
echo "[run-tests] Server port: $PORT"
echo "[run-tests] Opening: $URL"

# --- Open in default browser ---
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) start "$URL" ;;
    Darwin*)               open "$URL" ;;
    *)                     xdg-open "$URL" 2>/dev/null || echo "Open manually: $URL" ;;
esac
