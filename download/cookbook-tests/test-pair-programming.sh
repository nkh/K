#!/usr/bin/env bash
# test-pair-programming.sh — Tests "Pair Programming Setup" cookbook.
# Validates: shared command via API, keystrokes, shared buffer, lookup.

set -euo pipefail

VRW_BIN="${VRW_BIN:-vrw}"
PORT=$((19401 + RANDOM % 100))
BASE_URL="http://127.0.0.1:${PORT}"
PASS=0
FAIL=0

pass() { echo "  PASS: $1"; ((PASS++)) || true; }
fail() { echo "  FAIL: $1"; ((FAIL++)) || true; }
section() { echo ""; echo "=== $1 ==="; }

cleanup() {
    echo ""
    echo "--- Cleanup ---"
    if [ -n "${VRW_PID:-}" ] && kill -0 "$VRW_PID" 2>/dev/null; then
        echo "Stopping vrw (pid $VRW_PID)..."
        kill -TERM "$VRW_PID" 2>/dev/null || true
        timeout 3 wait "$VRW_PID" 2>/dev/null || true
        kill -KILL "$VRW_PID" 2>/dev/null || true
    fi
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [ "$FAIL" -eq 0 ] || exit 1
}
trap cleanup EXIT

echo "=== Cookbook Test: Pair Programming ==="
echo "Port: $PORT"

section "Start vrw (shared instance)"
$VRW_BIN --port "$PORT" --bind 127.0.0.1 -- sleep infinity &
VRW_PID=$!

for i in $(seq 1 30); do
    curl -sf "${BASE_URL}/api/info" >/dev/null 2>&1 && break
    sleep 0.2
done
echo "Server ready"

section "Spawn shared session via API"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
SESSION_ID=$(echo "$RESP" | jq -r '.data.id')
[ "$SESSION_ID" != "null" ] && pass "Session started, id=$SESSION_ID" || fail "Spawn failed: $RESP"

section "Developer 1 sends input"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${SESSION_ID}/keys" \
    -H "Content-Type: application/json" \
    -d '{"keys": "Hello from Dev 1!"}')
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "Dev 1 sent keystrokes" || fail "Keys failed: $RESP"

sleep 0.3

section "Developer 2 reads shared buffer"

RESP=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/text")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "Dev 2 can read VTTY text" || fail "vtty/text failed"

TEXT=$(echo "$RESP" | jq -r '.data.text')
echo "  Buffer: '$TEXT'"
echo "$TEXT" | grep -q "Hello from Dev 1" && pass "Dev 2 sees Dev 1's input" \
    || echo "  INFO: Text not found yet (timing)"

section "Developer 2 sends input"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${SESSION_ID}/keys" \
    -H "Content-Type: application/json" \
    -d '{"keys": "<Enter>And Dev 2 replies!"}')
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "Dev 2 sent keystrokes" || fail "Keys failed: $RESP"

sleep 0.3

section "Shared buffer contains both inputs"

RESP=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/text")
TEXT=$(echo "$RESP" | jq -r '.data.text')
echo "  Buffer: '$TEXT'"
echo "$TEXT" | grep -q "Dev 1" && pass "Buffer has Dev 1's text" || fail "Dev 1's text missing"
echo "$TEXT" | grep -q "Dev 2" && pass "Buffer has Dev 2's text" || fail "Dev 2's text missing"

section "Both devs read VTTY HTML (identical)"

RESP1=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/html")
RESP2=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/html")
HTML1=$(echo "$RESP1" | jq -r '.data.html')
HTML2=$(echo "$RESP2" | jq -r '.data.html')
[ "$HTML1" = "$HTML2" ] && pass "Both devs see identical HTML" || fail "HTML differs"

section "Multiple isolated pair sessions"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
SESSION_B=$(echo "$RESP" | jq -r '.data.id')
[ "$SESSION_B" != "null" ] && pass "Second session started" || fail "Second spawn failed"

RESP=$(curl -sf "${BASE_URL}/api/commands")
COUNT=$(echo "$RESP" | jq '.data | length')
[ "$COUNT" -ge 3 ] && pass "Multiple sessions listed (count=$COUNT)" || fail "Expected >= 3"

section "Lookup command by name"

RESP=$(curl -sf "${BASE_URL}/api/commands/lookup/cat")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "GET /api/commands/lookup/cat" || fail "lookup failed"
MATCHES=$(echo "$RESP" | jq '.data | length')
[ "$MATCHES" -ge 1 ] && pass "Found $MATCHES cat commands" || fail "No matches"

section "Cleanup"
for ID in "$SESSION_ID" "$SESSION_B"; do
    curl -sf -X POST "${BASE_URL}/api/commands/${ID}/kill" -H "Content-Type: application/json" -d "{}" >/dev/null 2>&1 || true
done
sleep 0.3
for ID in "$SESSION_ID" "$SESSION_B"; do
    curl -sf -X DELETE "${BASE_URL}/api/commands/${ID}" >/dev/null 2>&1 || true
done

section "Shutdown"
curl -sf -X POST "${BASE_URL}/api/shutdown" >/dev/null 2>&1 || true
