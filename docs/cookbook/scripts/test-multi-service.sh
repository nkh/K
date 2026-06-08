#!/usr/bin/env bash
# cookbook/scripts/test-multi-service.sh
#
# Tests the "Monitor Multiple Services" cookbook example.
# Validates: spawn, list, status fields, freeze/thaw, VTTY html,
#           retain_on_exit, kill, purge.
#
# Usage: ./docs/cookbook/scripts/test-multi-service.sh
#   or:  VRW_BIN=./target/debug/vrw ./docs/cookbook/scripts/test-multi-service.sh

set -euo pipefail

VRW_BIN="${VRW_BIN:-vrw}"
PORT=$((19001 + RANDOM % 100))
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
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [ "$FAIL" -eq 0 ] || exit 1
}
trap cleanup EXIT

echo "=== Cookbook Test: Monitor Multiple Services ==="
echo "Port: $PORT"

# ── Start vrw (no command, daemon-style via &) ──
section "Start vrw"
$VRW_BIN --port "$PORT" --bind 127.0.0.1 -- sleep infinity &
VRW_PID=$!
sleep 1

# Wait for server
for i in $(seq 1 30); do
    curl -sf "${BASE_URL}/api/info" >/dev/null 2>&1 && break
    sleep 0.2
done

# ── 1. Spawn multiple services ──
section "Spawn services via API"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "sleep", "args": ["999"], "retain_on_exit": true}')
ID1=$(echo "$RESP" | jq -r '.data.id')
[ "$ID1" != "null" ] && pass "Spawn service 1 (sleep) got id=$ID1" || fail "Spawn service 1 failed: $RESP"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
ID2=$(echo "$RESP" | jq -r '.data.id')
[ "$ID2" != "null" ] && pass "Spawn service 2 (cat) got id=$ID2" || fail "Spawn service 2 failed"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "echo", "args": ["hello"]}')
ID3=$(echo "$RESP" | jq -r '.data.id')
[ "$ID3" != "null" ] && pass "Spawn service 3 (echo) got id=$ID3" || fail "Spawn service 3 failed"

sleep 0.5  # let echo finish

# ── 2. List commands and check status fields ──
section "List commands & check status fields"

RESP=$(curl -sf "${BASE_URL}/api/commands")
STATUS=$(echo "$RESP" | jq -r '.status')
[ "$STATUS" = "ok" ] && pass "list_commands returns status=ok" || fail "list_commands status: $STATUS"

# Check each command has the documented fields
for FIELD in id name args pid alive status frozen runtime_secs certificate exit; do
    HAS=$(echo "$RESP" | jq ".data[0] | has(\"$FIELD\")")
    [ "$HAS" = "true" ] && pass "Command object has field '$FIELD'" || fail "Missing field '$FIELD'"
done

# Check status values: running, frozen, or exited
STATUSES=$(echo "$RESP" | jq -r '.data[].status' | sort -u)
for S in $STATUSES; do
    case "$S" in
        running|frozen|exited) pass "Status '$S' is a valid value" ;;
        *) fail "Unknown status value '$S'" ;;
    esac
done

# ── 3. Freeze a running service ──
section "Freeze service (SIGSTOP)"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${ID1}/freeze")
FROZEN=$(echo "$RESP" | jq -r '.data.frozen')
[ "$FROZEN" = "true" ] && pass "freeze returns frozen=true" || fail "freeze response: $RESP"

# Verify status shows "frozen" in list
RESP=$(curl -sf "${BASE_URL}/api/commands")
S1=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$ID1\") | .status")
[ "$S1" = "frozen" ] && pass "List shows frozen status" || fail "Expected frozen, got: $S1"

# ── 4. Thaw the service ──
section "Thaw service (SIGCONT)"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${ID1}/thaw")
FROZEN=$(echo "$RESP" | jq -r '.data.frozen')
[ "$FROZEN" = "false" ] && pass "thaw returns frozen=false" || fail "thaw response: $RESP"

RESP=$(curl -sf "${BASE_URL}/api/commands")
S1=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$ID1\") | .status")
[ "$S1" = "running" ] && pass "List shows running status after thaw" || fail "Expected running, got: $S1"

# ── 5. Get VTTY HTML ──
section "VTTY HTML endpoint"

RESP=$(curl -sf "${BASE_URL}/api/commands/${ID2}/vtty/html")
HAS_HTML=$(echo "$RESP" | jq '.data | has("html")')
[ "$HAS_HTML" = "true" ] && pass "vtty/html returns html field" || fail "Missing html field"

HAS_DIMS=$(echo "$RESP" | jq '.data | has("dimensions")')
[ "$HAS_DIMS" = "true" ] && pass "vtty/html returns dimensions" || fail "Missing dimensions"

HAS_CURSOR=$(echo "$RESP" | jq '.data | has("cursor")')
[ "$HAS_CURSOR" = "true" ] && pass "vtty/html returns cursor position" || fail "Missing cursor"

# ── 6. Kill service 1 (sleep) ──
section "Kill service"

curl -sf -X POST "${BASE_URL}/api/commands/${ID1}/kill" >/dev/null
sleep 0.5

RESP=$(curl -sf "${BASE_URL}/api/commands")
S1=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$ID1\") | .status")
# With retain_on_exit, status should be "exited"
[ "$S1" = "exited" ] && pass "Killed command shows exited status (retain_on_exit works)" \
    || fail "Expected exited after kill, got: $S1"

# ── 7. Purge exited service ──
section "Purge exited service"

RESP=$(curl -sf -X DELETE "${BASE_URL}/api/commands/${ID1}")
PURGED=$(echo "$RESP" | jq -r '.data.purged')
[ "$PURGED" = "true" ] && pass "purge returns purged=true" || fail "purge response: $RESP"

# Verify it's gone from the list
RESP=$(curl -sf "${BASE_URL}/api/commands")
COUNT=$(echo "$RESP" | jq '[.data[] | select(.id=="'"$ID1"'")] | length')
[ "$COUNT" = "0" ] && pass "Purged command no longer in list" || fail "Still in list after purge"

# ── 8. Info endpoint ──
section "Info endpoint"

RESP=$(curl -sf "${BASE_URL}/api/info")
HAS_COUNT=$(echo "$RESP" | jq '.data | has("command_count")')
[ "$HAS_COUNT" = "true" ] && pass "info has command_count" || fail "Missing command_count"

# ── Done ──
section "Shutdown"
curl -sf -X POST "${BASE_URL}/api/shutdown" >/dev/null 2>&1 || true
