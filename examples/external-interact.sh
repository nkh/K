#!/usr/bin/env bash
# examples/external-interact.sh
#
# Demonstrates how an external script can interact with a running vrw
# instance via its HTTP REST API.  This script:
#
#   1. Starts vrw in headless mode with `cat` (simple command)
#   2. Sends keystrokes to the running command
#   3. Waits for VTTY buffer changes (polls until content changes)
#   4. Checks what the change is (reads the VTTY plain text)
#   5. Sends another command
#   6. Kills the command and triggers vrw shutdown
#
# Prerequisites:
#   - vrw binary in PATH (or set VRW_BIN below)
#   - curl, jq installed
#
# Usage:
#   ./examples/external-interact.sh

set -euo pipefail

VRW_BIN="${VRW_BIN:-vrw}"
BASE_URL="http://127.0.0.1:9091"
TMPDIR="${TMPDIR:-/tmp}"
SNAPSHOT_FILE="${TMPDIR}/vrw-snapshot-$$.txt"

# ── Cleanup ──
cleanup() {
    echo "--- Cleanup ---"
    # Kill vrw if still running
    if [ -n "${VRW_PID:-}" ] && kill -0 "$VRW_PID" 2>/dev/null; then
        echo "Stopping vrw (pid $VRW_PID)..."
        kill "$VRW_PID" 2>/dev/null || true
        wait "$VRW_PID" 2>/dev/null || true
    fi
    rm -f "$SNAPSHOT_FILE"
}
trap cleanup EXIT

echo "=== VRunner External Interaction Example ==="
echo ""

# ── 1. Start vrw in headless mode ──
echo "--- Step 1: Starting vrw with 'cat' ---"
# Use a unique port to avoid conflicts
PORT=9091
$VRW_BIN --port "$PORT" --snapshot-on-exit "$SNAPSHOT_FILE" -- cat
VRW_PID=$!
echo "vrw started with pid=$VRW_PID on port=$PORT"

# Wait for the server to be ready
echo "Waiting for server..."
for i in $(seq 1 20); do
    if curl -sf "${BASE_URL}/api/commands" >/dev/null 2>&1; then
        echo "Server is ready!"
        break
    fi
    sleep 0.1
done

# ── 2. List commands and get the command ID ──
echo ""
echo "--- Step 2: Listing running commands ---"
RESPONSE=$(curl -sf "${BASE_URL}/api/commands")
echo "API response: $RESPONSE" | jq '.' 2>/dev/null || echo "$RESPONSE"

CMD_ID=$(echo "$RESPONSE" | jq -r '.data[0].id')
CMD_NAME=$(echo "$RESPONSE" | jq -r '.data[0].name')
echo "Command ID: $CMD_ID"
echo "Command name: $CMD_NAME"

# ── 3. Send initial keystrokes ──
echo ""
echo "--- Step 3: Sending keystrokes to the command ---"
# Send "hello world" followed by Enter
# Note: cat echoes input back, so we'll see it in the VTTY
curl -sf -X POST "${BASE_URL}/api/commands/${CMD_ID}/keys" \
    -H "Content-Type: application/json" \
    -d '{"keys": "Hello from external script!"}' \
    | jq '.'
echo "Sent text to cat"

# Wait a moment for the VTTY to update
sleep 0.3

# ── 4. Get the current VTTY content ──
echo ""
echo "--- Step 4: Reading VTTY plain text ---"
VTTY_CONTENT=$(curl -sf "${BASE_URL}/api/commands/${CMD_ID}/vtty/buffer?format=plain" 2>/dev/null || \
    curl -sf "${BASE_URL}/api/commands/${CMD_ID}/vtty/plain")
echo "VTTY content:"
echo "$VTTY_CONTENT"
echo "---"

# Verify our text appeared
if echo "$VTTY_CONTENT" | grep -q "Hello from external script"; then
    echo "SUCCESS: Text found in VTTY buffer"
else
    echo "NOTE: Text may not have appeared yet (timing)"
fi

# ── 5. Send Ctrl+D to close cat's stdin ──
echo ""
echo "--- Step 5: Sending Ctrl+D to end cat ---"
curl -sf -X POST "${BASE_URL}/api/commands/${CMD_ID}/keys" \
    -H "Content-Type: application/json" \
    -d '{"keys": "<C-d>"}' \
    | jq '.'

# Wait for cat to exit
sleep 0.5

# Check if the command has exited
RESPONSE=$(curl -sf "${BASE_URL}/api/commands" 2>/dev/null || echo '{}')
ALIVE=$(echo "$RESPONSE" | jq -r '.data[0].alive // "false"')
echo "Command alive: $ALIVE"

# ── 6. Check the snapshot file ──
echo ""
echo "--- Step 6: Checking snapshot file ---"
if [ -f "$SNAPSHOT_FILE" ]; then
    echo "Snapshot file exists: $SNAPSHOT_FILE"
    echo "Contents:"
    cat "$SNAPSHOT_FILE"
else
    echo "Snapshot file not found (command may still be running)"
fi

# ── 7. Shutdown vrw ──
echo ""
echo "--- Step 7: Shutting down vrw ---"
curl -sf -X POST "${BASE_URL}/api/shutdown" | jq '.' 2>/dev/null || echo "Shutdown sent"
wait "$VRW_PID" 2>/dev/null || true
echo "vrw exited"

echo ""
echo "=== Example complete ==="
echo ""
echo "Summary of API endpoints used:"
echo "  GET    /api/commands              - List running commands"
echo "  POST   /api/commands/:id/keys     - Send keystrokes"
echo "  GET    /api/commands/:id/vtty/plain - Read VTTY as plain text"
echo "  POST   /api/shutdown              - Trigger graceful shutdown"
echo ""
echo "Key notation for send_keys:"
echo "  Plain text:  'hello world'        - Sent as-is"
echo "  Enter:       '<Enter>'            - Newline"
echo "  Ctrl+C:      '<C-c>'              - Interrupt"
echo "  Ctrl+D:      '<C-d>'              - EOF"
echo "  Tab:         '<Tab>'              - Tab character"
echo "  Arrow keys:  '<Up>', '<Down>', '<Left>', '<Right>'"
echo "  Function:    '<F1>' through '<F12>'"
echo "  Alt+key:     '<A-x>'              - Alt + x"
echo "  Escape:      '<Esc>'"
echo ""
echo "Alternative: wait for VTTY change instead of sleep"
echo "  GET /api/commands/:id/vtty/changed?since=<generation>"
echo "  Returns true if the buffer has changed since the given generation."
echo "  Use the 'generation' field from the list commands response."
