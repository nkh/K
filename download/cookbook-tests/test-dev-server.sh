#!/usr/bin/env bash
# test-dev-server.sh — Tests "Dev Server with Hot Reload" cookbook.
# Validates: config file, spawn subcommand, stop-command, shutdown.

set -euo pipefail

VRUNNER_BIN="${VRUNNER_BIN:-vrunner}"
PORT=$((19201 + RANDOM % 100))
BASE_URL="http://127.0.0.1:${PORT}"
PASS=0
FAIL=0
CONFIG_FILE="/tmp/vrunner-test-devserver-$$.yaml"

pass() { echo "  PASS: $1"; ((PASS++)) || true; }
fail() { echo "  FAIL: $1"; ((FAIL++)) || true; }
section() { echo ""; echo "=== $1 ==="; }

cleanup() {
    echo ""
    echo "--- Cleanup ---"
    if [ -n "${VRUNNER_PID:-}" ] && kill -0 "$VRUNNER_PID" 2>/dev/null; then
        echo "Stopping vrunner (pid $VRUNNER_PID)..."
        kill -TERM "$VRUNNER_PID" 2>/dev/null || true
        timeout 3 wait "$VRUNNER_PID" 2>/dev/null || true
        kill -KILL "$VRUNNER_PID" 2>/dev/null || true
    fi
    rm -f "${CONFIG_FILE}"
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [ "$FAIL" -eq 0 ] || exit 1
}
trap cleanup EXIT

echo "=== Cookbook Test: Dev Server with Hot Reload ==="
echo "Port: $PORT"

section "Create config file"
cat > "$CONFIG_FILE" <<EOF
server:
  bind: "127.0.0.1"
  port: ${PORT}

web:
  update_mode: "push"
  dirty_check_ms: 200
  default_poll_ms: 500

default_exit:
  exit:
    timeout_secs: 5
EOF
[ -f "$CONFIG_FILE" ] && pass "Config file created" || fail "Config creation failed"

section "Start vrunner with config"
$VRUNNER_BIN --config "$CONFIG_FILE" -- sleep infinity &
VRUNNER_PID=$!

for i in $(seq 1 30); do
    curl -sf "${BASE_URL}/api/info" >/dev/null 2>&1 && break
    sleep 0.2
done

RESP=$(curl -sf "${BASE_URL}/api/info")
MODE=$(echo "$RESP" | jq -r '.data.web.update_mode')
[ "$MODE" = "push" ] && pass "Config applied: web.update_mode=push" || fail "update_mode=$MODE"

section "Spawn via CLI 'vrunner spawn'"
$VRUNNER_BIN --port "$PORT" spawn -- sleep 999
sleep 0.3

RESP=$(curl -sf "${BASE_URL}/api/commands")
NAMES=$(echo "$RESP" | jq -r '.data[].name')
echo "$NAMES" | grep -q "sleep" && pass "'vrunner spawn -- sleep 999' works" || fail "No sleep found: $NAMES"

section "Spawn via API"
RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
ID=$(echo "$RESP" | jq -r '.data.id')
[ "$ID" != "null" ] && pass "Spawned cat via API" || fail "API spawn failed: $RESP"

section "vrunner stop-command"
# Use command name "cat" instead of UUID (stop-command resolves by name/PID)
$VRUNNER_BIN --port "$PORT" stop-command cat >/dev/null 2>&1
sleep 0.3

RESP=$(curl -sf "${BASE_URL}/api/commands")
# stop-command is destructive (same as kill API) — command is removed
GONE=$(echo "$RESP" | jq "[.data[] | select(.id==\"$ID\")] | length")
[ "$GONE" = "0" ] && pass "stop-command removed the target" || fail "Command still in list"

section "VTTY endpoints"
RESP=$(curl -sf "${BASE_URL}/api/commands")
LIVE_ID=$(echo "$RESP" | jq -r '.data[0].id')

RESP=$(curl -sf "${BASE_URL}/api/commands/${LIVE_ID}/vtty/html")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "GET vtty/html" || fail "vtty/html failed"

RESP=$(curl -sf "${BASE_URL}/api/commands/${LIVE_ID}/vtty/text")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "GET vtty/text" || fail "vtty/text failed"

RESP=$(curl -sf "${BASE_URL}/api/commands/${LIVE_ID}/vtty/changed")
[ "$(echo "$RESP" | jq '.data | has("changed")')" = "true" ] && pass "GET vtty/changed" || fail "vtty/changed failed"

section "Shutdown via API"
RESP=$(curl -sf -X POST "${BASE_URL}/api/shutdown")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "POST /api/shutdown" || fail "shutdown failed: $RESP"
