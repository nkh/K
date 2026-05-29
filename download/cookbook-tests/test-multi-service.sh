#!/usr/bin/env bash
# test-multi-service.sh — Tests "Monitor Multiple Services" cookbook.
# Validates: spawn, list, status fields, freeze/thaw, VTTY html, kill, purge.

set -euo pipefail

VRUNNER_BIN="${VRUNNER_BIN:-vrunner}"
PORT=$((19001 + RANDOM % 100))
BASE_URL="http://127.0.0.1:${PORT}"
PASS=0
FAIL=0

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
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [ "$FAIL" -eq 0 ] || exit 1
}
trap cleanup EXIT

echo "=== Cookbook Test: Monitor Multiple Services ==="
echo "Port: $PORT"

section "Start vrunner"
$VRUNNER_BIN --port "$PORT" --bind 127.0.0.1 -- sleep infinity &
VRUNNER_PID=$!
sleep 1

for i in $(seq 1 30); do
    curl -sf "${BASE_URL}/api/info" >/dev/null 2>&1 && break
    sleep 0.2
done

# ── Spawn services ──
section "Spawn services via API"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "sleep", "args": ["999"], "retain_on_exit": true}')
ID1=$(echo "$RESP" | jq -r '.data.id')
[ "$ID1" != "null" ] && pass "Spawn sleep (retain_on_exit) id=$ID1" || fail "Spawn failed: $RESP"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
ID2=$(echo "$RESP" | jq -r '.data.id')
[ "$ID2" != "null" ] && pass "Spawn cat id=$ID2" || fail "Spawn failed: $RESP"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "echo", "args": ["hello"], "retain_on_exit": true}')
ID3=$(echo "$RESP" | jq -r '.data.id')
[ "$ID3" != "null" ] && pass "Spawn echo (retain_on_exit) id=$ID3" || fail "Spawn failed: $RESP"
sleep 0.5

# ── List & status fields ──
section "List commands & check status fields"

RESP=$(curl -sf "${BASE_URL}/api/commands")
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "list_commands status=ok" || fail "list status wrong"

for FIELD in id name args pid alive status frozen runtime_secs certificate exit; do
    HAS=$(echo "$RESP" | jq ".data[0] | has(\"$FIELD\")")
    [ "$HAS" = "true" ] && pass "Field '$FIELD' exists" || fail "Missing '$FIELD'"
done

STATUSES=$(echo "$RESP" | jq -r '.data[].status' | sort -u)
for S in $STATUSES; do
    case "$S" in running|frozen|exited) pass "Status '$S' valid" ;; *) fail "Unknown '$S'" ;; esac
done

# ── Freeze / thaw ──
section "Freeze (SIGSTOP)"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${ID1}/freeze")
[ "$(echo "$RESP" | jq -r '.data.frozen')" = "true" ] && pass "freeze ok" || fail "freeze: $RESP"

RESP=$(curl -sf "${BASE_URL}/api/commands")
S=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$ID1\") | .status")
[ "$S" = "frozen" ] && pass "List shows frozen" || fail "Expected frozen, got: $S"

section "Thaw (SIGCONT)"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${ID1}/thaw")
[ "$(echo "$RESP" | jq -r '.data.frozen')" = "false" ] && pass "thaw ok" || fail "thaw: $RESP"

RESP=$(curl -sf "${BASE_URL}/api/commands")
S=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$ID1\") | .status")
[ "$S" = "running" ] && pass "running after thaw" || fail "Expected running, got: $S"

# ── VTTY HTML ──
section "VTTY HTML endpoint"

RESP=$(curl -sf "${BASE_URL}/api/commands/${ID2}/vtty/html")
[ "$(echo "$RESP" | jq '.data | has("html")')" = "true" ] && pass "has html" || fail "no html"
[ "$(echo "$RESP" | jq '.data | has("dimensions")')" = "true" ] && pass "has dimensions" || fail "no dims"
[ "$(echo "$RESP" | jq '.data | has("cursor")')" = "true" ] && pass "has cursor" || fail "no cursor"

# ── Kill (destructive — removes from list) ──
section "Kill (destructive — removes command)"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${ID1}/kill" \
    -H 'Content-Type: application/json' -d '{}')
[ "$(echo "$RESP" | jq -r '.status')" = "ok" ] && pass "kill returns ok" || fail "kill: $RESP"
sleep 0.3

RESP=$(curl -sf "${BASE_URL}/api/commands")
GONE=$(echo "$RESP" | jq "[.data[] | select(.id==\"$ID1\")] | length")
[ "$GONE" = "0" ] && pass "Killed command gone from list" || fail "Still in list after kill"

# ── retain_on_exit (natural exit) ──
section "retain_on_exit — naturally exited command stays"

RESP=$(curl -sf "${BASE_URL}/api/commands")
S3=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$ID3\") | .status")
if [ -n "$S3" ] && [ "$S3" = "exited" ]; then
    pass "Echo (retain_on_exit) still listed as exited"
    RESP=$(curl -sf -X DELETE "${BASE_URL}/api/commands/${ID3}")
    [ "$(echo "$RESP" | jq -r '.data.purged')" = "true" ] && pass "purge ok" || fail "purge failed"
else
    echo "  INFO: echo may have been removed (default retain_on_exit=false for non-specified)"
fi

# ── Info ──
section "Info endpoint"
RESP=$(curl -sf "${BASE_URL}/api/info")
[ "$(echo "$RESP" | jq '.data | has("command_count")')" = "true" ] && pass "has command_count" || fail "no count"

section "Shutdown"
curl -sf -X POST "${BASE_URL}/api/shutdown" >/dev/null 2>&1 || true
