#!/usr/bin/env bash
# cookbook/scripts/test-dev-server.sh
#
# Tests the "Dev Server with Hot Reload" cookbook example.
# Validates: config file, spawn subcommand, stop-command, shutdown.
#
# Usage: ./docs/cookbook/scripts/test-dev-server.sh
#   or:  VRW_BIN=./target/debug/vrw ./docs/cookbook/scripts/test-dev-server.sh

set -euo pipefail

VRW_BIN="${VRW_BIN:-vrw}"
PORT=$((19201 + RANDOM % 100))
BASE_URL="http://127.0.0.1:${PORT}"
PASS=0
FAIL=0

pass() { echo "  PASS: $1"; ((PASS++)); }
fail() { echo "  FAIL: $1"; ((FAIL++)); }
section() { echo ""; echo "=== $1 ==="; }

cleanup() {
    echo ""
    echo "--- Cleanup ---"
    if [ -n "${VRW_PID:-}" ] && kill -0 "$VRW_PID" 2>/dev/null; then
        echo "Stopping vrw (pid $VRW_PID)..."
        kill "$VRW_PID" 2>/dev/null || true
        wait "$VRW_PID" 2>/dev/null || true
    fi
    rm -f "${CONFIG_FILE}"
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [ "$FAIL" -eq 0 ] || exit 1
}
trap cleanup EXIT

echo "=== Cookbook Test: Dev Server with Hot Reload ==="
echo "Port: $PORT"

# ── 1. Create a config file ──
section "Create config file"

CONFIG_FILE="/tmp/vrw-test-devserver-$$.yaml"
cat > "$CONFIG_FILE" <<EOF
server:
  bind: "127.0.0.1"
  port: ${PORT}

vtty:
  rows: 30
  cols: 120
  scrollback: 2000

web:
  update_mode: "push"

display:
  refresh_ms: 80

default_exit:
  exit:
    timeout_secs: 5
EOF
[ -f "$CONFIG_FILE" ] && pass "Config file created at $CONFIG_FILE" || fail "Config file creation failed"

# ── 2. Start vrw with config file ──
section "Start vrw with config"

$VRW_BIN --config "$CONFIG_FILE" -- sleep infinity &
VRW_PID=$!

for i in $(seq 1 30); do
    curl -sf "${BASE_URL}/api/info" >/dev/null 2>&1 && break
    sleep 0.2
done

# Verify config was applied
RESP=$(curl -sf "${BASE_URL}/api/info")
MODE=$(echo "$RESP" | jq -r '.data.web.update_mode')
[ "$MODE" = "push" ] && pass "Config applied: web.update_mode=push" || fail "update_mode=$MODE (expected push)"

# ── 3. Spawn via CLI subcommand ──
section "Spawn via CLI 'vrw spawn'"

$VRW_BIN --port "$PORT" spawn -- sleep 999
sleep 0.3

RESP=$(curl -sf "${BASE_URL}/api/commands")
NAMES=$(echo "$RESP" | jq -r '.data[].name')
echo "$NAMES" | grep -q "sleep" && pass "'vrw spawn -- sleep 999' created a command" \
    || fail "No 'sleep' command found. Names: $NAMES"

# ── 4. Spawn via API (as in the cookbook) ──
section "Spawn via API"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
ID=$(echo "$RESP" | jq -r '.data.id')
[ "$ID" != "null" ] && pass "Spawned cat via API" || fail "API spawn failed: $RESP"

# ── 5. stop-command subcommand ──
section "vrw stop-command"

# The cookbook mentions "vrw stop-command <PID>"
# Test: stop the cat command by its ID
$VRW_BIN --port "$PORT" stop-command "$ID" >/dev/null 2>&1
sleep 0.3

RESP=$(curl -sf "${BASE_URL}/api/commands")
STATUS=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$ID\") | .status")
[ "$STATUS" = "exited" ] && pass "stop-command stopped the target command" \
    || fail "Command status after stop-command: $STATUS"

# ── 6. Verify VTTY endpoints ──
section "VTTY endpoints"

RESP=$(curl -sf "${BASE_URL}/api/commands")
LIVE_ID=$(echo "$RESP" | jq -r '.data[0].id')

# vtty/html
RESP=$(curl -sf "${BASE_URL}/api/commands/${LIVE_ID}/vtty/html")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "GET /api/commands/{id}/vtty/html works" \
    || fail "vtty/html failed"

# vtty/text
RESP=$(curl -sf "${BASE_URL}/api/commands/${LIVE_ID}/vtty/text")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "GET /api/commands/{id}/vtty/text works" \
    || fail "vtty/text failed"

# vtty/changed (poll mode)
RESP=$(curl -sf "${BASE_URL}/api/commands/${LIVE_ID}/vtty/changed")
HAS_CHANGED=$(echo "$RESP" | jq '.data | has("changed")')
[ "$HAS_CHANGED" = "true" ] && pass "GET /api/commands/{id}/vtty/changed works" \
    || fail "vtty/changed failed"

# ── 7. Shutdown via API ──
section "Shutdown via API"

RESP=$(curl -sf -X POST "${BASE_URL}/api/shutdown")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "POST /api/shutdown works" \
    || fail "shutdown failed: $RESP"
