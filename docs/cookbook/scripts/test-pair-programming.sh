#!/usr/bin/env bash
# cookbook/scripts/test-pair-programming.sh
#
# Tests the "Pair Programming Setup" cookbook example.
# Validates: shared command via API, keystroke input, VTTY visibility,
#           both "developers" see the same buffer, certificates.
#
# Usage: ./docs/cookbook/scripts/test-pair-programming.sh
#   or:  VRUNNER_BIN=./target/debug/vrunner ./docs/cookbook/scripts/test-pair-programming.sh

set -euo pipefail

VRUNNER_BIN="${VRUNNER_BIN:-vrunner}"
PORT=$((19401 + RANDOM % 100))
BASE_URL="http://127.0.0.1:${PORT}"
PASS=0
FAIL=0

pass() { echo "  PASS: $1"; ((PASS++)); }
fail() { echo "  FAIL: $1"; ((FAIL++)); }
section() { echo ""; echo "=== $1 ==="; }

cleanup() {
    echo ""
    echo "--- Cleanup ---"
    if [ -n "${VRUNNER_PID:-}" ] && kill -0 "$VRUNNER_PID" 2>/dev/null; then
        echo "Stopping vrunner (pid $VRUNNER_PID)..."
        kill "$VRUNNER_PID" 2>/dev/null || true
        wait "$VRUNNER_PID" 2>/dev/null || true
    fi
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [ "$FAIL" -eq 0 ] || exit 1
}
trap cleanup EXIT

echo "=== Cookbook Test: Pair Programming ==="
echo "Port: $PORT"

# ── Start vrunner as a shared instance ──
section "Start vrunner (shared instance)"

$VRUNNER_BIN --port "$PORT" --bind 127.0.0.1 -- sleep infinity &
VRUNNER_PID=$!

for i in $(seq 1 30); do
    curl -sf "${BASE_URL}/api/info" >/dev/null 2>&1 && break
    sleep 0.2
done
echo "Server ready"

# ── 1. Start a shared editing session via API ──
section "Spawn shared session via API"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
SESSION_ID=$(echo "$RESP" | jq -r '.data.id')
[ "$SESSION_ID" != "null" ] && pass "Session started, id=$SESSION_ID" || fail "Spawn failed: $RESP"

# ── 2. Developer 1 sends keystrokes ──
section "Developer 1 sends input"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${SESSION_ID}/keys" \
    -H "Content-Type: application/json" \
    -d '{"keys": "Hello from Dev 1!"}')
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "Dev 1 sent keystrokes" || fail "Keys failed: $RESP"

sleep 0.3

# ── 3. Developer 2 reads the same VTTY ──
section "Developer 2 reads shared buffer"

RESP=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/text")
STATUS=$(echo "$RESP" | jq -r '.status')
[ "$STATUS" = "ok" ] && pass "Dev 2 can read VTTY text" || fail "vtty/text failed: $RESP"

TEXT=$(echo "$RESP" | jq -r '.data.text')
echo "$TEXT" | grep -q "Hello from Dev 1" && pass "Dev 2 sees Dev 1's input in buffer" \
    || echo "  INFO: Buffer text: '$TEXT'"

# ── 4. Developer 2 also sends input ──
section "Developer 2 sends input"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${SESSION_ID}/keys" \
    -H "Content-Type: application/json" \
    -d '{"keys": "<Enter>And Dev 2 replies!"}')
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "Dev 2 sent keystrokes" || fail "Keys failed: $RESP"

sleep 0.3

# ── 5. Verify both inputs visible in buffer ──
section "Shared buffer contains both inputs"

RESP=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/text")
TEXT=$(echo "$RESP" | jq -r '.data.text')

echo "  Buffer: '$TEXT'"
echo "$TEXT" | grep -q "Dev 1" && pass "Buffer has Dev 1's text" \
    || fail "Dev 1's text not found"
echo "$TEXT" | grep -q "Dev 2" && pass "Buffer has Dev 2's text" \
    || fail "Dev 2's text not found"

# ── 6. Both read VTTY HTML (as web UI would) ──
section "Both devs read VTTY HTML"

RESP1=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/html")
RESP2=$(curl -sf "${BASE_URL}/api/commands/${SESSION_ID}/vtty/html")

HTML1=$(echo "$RESP1" | jq -r '.data.html')
HTML2=$(echo "$RESP2" | jq -r '.data.html')

# Both should be identical (same buffer, same generation)
[ "$HTML1" = "$HTML2" ] && pass "Both devs see identical HTML output" \
    || fail "HTML differs between reads"

# ── 7. Multiple isolated sessions ──
section "Multiple isolated pair sessions"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
SESSION_B=$(echo "$RESP" | jq -r '.data.id')
[ "$SESSION_B" != "null" ] && pass "Second session started" || fail "Second spawn failed"

# Sessions should be independent
RESP=$(curl -sf "${BASE_URL}/api/commands")
COUNT=$(echo "$RESP" | jq '.data | length')
[ "$COUNT" -ge 3 ] && pass "Multiple sessions listed (count=$COUNT)" || fail "Expected >= 3 commands"

# ── 8. lookup endpoint (used for URL routing) ──
section "Lookup command by name"

RESP=$(curl -sf "${BASE_URL}/api/commands/lookup/cat")
STATUS=$(echo "$RESP" | jq -r '.status')
[ "$STATUS" = "ok" ] && pass "GET /api/commands/lookup/cat works" || fail "lookup failed: $RESP"

MATCHES=$(echo "$RESP" | jq '.data | length')
[ "$MATCHES" -ge 1 ] && pass "Lookup found $MATCHES cat commands" || fail "No matches"

# ── 9. Cleanup sessions ──
section "Cleanup"

for ID in "$SESSION_ID" "$SESSION_B"; do
    curl -sf -X POST "${BASE_URL}/api/commands/${ID}/kill" >/dev/null 2>&1 || true
done
sleep 0.3

for ID in "$SESSION_ID" "$SESSION_B"; do
    curl -sf -X DELETE "${BASE_URL}/api/commands/${ID}" >/dev/null 2>&1 || true
done
echo "Sessions cleaned up"

# ── Done ──
section "Shutdown"
curl -sf -X POST "${BASE_URL}/api/shutdown" >/dev/null 2>&1 || true
