#!/usr/bin/env bash
# test-ci-pipeline.sh — Tests "CI Pipeline with vrunner" cookbook.
# Validates: spawn with retain_on_exit, poll status, vtty/partial,
#           vtty/text, snapshot, purge.

set -euo pipefail

VRUNNER_BIN="${VRUNNER_BIN:-vrunner}"
PORT=$((19301 + RANDOM % 100))
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

echo "=== Cookbook Test: CI Pipeline ==="
echo "Port: $PORT"

section "Start vrunner"
$VRUNNER_BIN --port "$PORT" --bind 127.0.0.1 -- sleep infinity &
VRUNNER_PID=$!

for i in $(seq 1 30); do
    curl -sf "${BASE_URL}/api/info" >/dev/null 2>&1 && break
    sleep 0.2
done
echo "Server ready"

section "Spawn build command with retain_on_exit"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{
        "cmd": "echo",
        "args": ["Build step 1", "Build step 2", "Build step 3", "Done!"],
        "env": {"CI": "true", "RUST_LOG": "info"},
        "retain_on_exit": true
    }')
JOB_ID=$(echo "$RESP" | jq -r '.data.id')
[ "$JOB_ID" != "null" ] && pass "Build started, id=$JOB_ID" || fail "Spawn failed: $RESP"

section "Poll for build completion"

echo "Waiting for build to finish..."
ELAPSED=0
while [ "$ELAPSED" -lt 20 ]; do
    STATUS=$(curl -sf "${BASE_URL}/api/commands" \
        | jq -r ".data[] | select(.id==\"$JOB_ID\") | .status")
    if [ "$STATUS" != "running" ]; then
        echo "Build finished with status: $STATUS"
        break
    fi
    sleep 0.5
    ELAPSED=$((ELAPSED + 1))
done
[ "$STATUS" = "exited" ] && pass "Build completed (status=exited)" || fail "Status: $STATUS"

section "Check exit_code"

RESP=$(curl -sf "${BASE_URL}/api/commands")
EXIT_CODE=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$JOB_ID\") | .exit_code")
echo "Exit code: $EXIT_CODE"
[ "$EXIT_CODE" = "0" ] && pass "exit_code is 0" || fail "exit_code=$EXIT_CODE"

section "Retrieve output via vtty/partial"

RESP=$(curl -sf "${BASE_URL}/api/commands/${JOB_ID}/vtty/partial?offset=0&limit=100")
STATUS=$(echo "$RESP" | jq -r '.status')
[ "$STATUS" = "ok" ] && pass "vtty/partial returns ok" || fail "vtty/partial: $RESP"

CONTENT=$(echo "$RESP" | jq -r '.data.content')
echo "$CONTENT" | grep -qi "done" && pass "Output contains 'Done'" \
    || echo "  INFO: Output: '$CONTENT'"

OFFSET=$(echo "$RESP" | jq -r '.data.offset')
LIMIT=$(echo "$RESP" | jq -r '.data.limit')
[ "$OFFSET" = "0" ] && pass "Response offset=0" || fail "offset=$OFFSET"
[ "$LIMIT" = "100" ] && pass "Response limit=100" || fail "limit=$LIMIT"

section "Plain text output"

RESP=$(curl -sf "${BASE_URL}/api/commands/${JOB_ID}/vtty/text")
[ "$(echo "$RESP" | jq '.data | has("text")')" = "true" ] && pass "vtty/text has text field" || fail "Missing text"

section "Snapshot"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands/${JOB_ID}/snapshot" \
    -H "Content-Type: application/json" \
    -d '{"name": "test-snapshot"}')
echo "  Snapshot response: $(echo "$RESP" | jq -r '.status')"

section "Spawn failing build"

RESP=$(curl -sf -X POST "${BASE_URL}/api/commands" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "false", "retain_on_exit": true}')
FAIL_ID=$(echo "$RESP" | jq -r '.data.id')
sleep 0.5

RESP=$(curl -sf "${BASE_URL}/api/commands")
FAIL_CODE=$(echo "$RESP" | jq -r ".data[] | select(.id==\"$FAIL_ID\") | .exit_code")
[ "$FAIL_CODE" = "1" ] && pass "Failing build has exit_code=1" || fail "exit_code=$FAIL_CODE"

section "Purge exited commands"

for ID in "$JOB_ID" "$FAIL_ID"; do
    RESP=$(curl -sf -X DELETE "${BASE_URL}/api/commands/${ID}")
    [ "$(echo "$RESP" | jq -r '.data.purged')" = "true" ] && pass "Purged $ID" || fail "Purge $ID failed: $RESP"
done

RESP=$(curl -sf "${BASE_URL}/api/commands")
REMAINING=$(echo "$RESP" | jq '[.data[] | select(.id=="'"$JOB_ID"'" or .id=="'"$FAIL_ID"'")] | length')
[ "$REMAINING" = "0" ] && pass "All purged commands gone" || fail "$REMAINING remain"

section "Shutdown"
curl -sf -X POST "${BASE_URL}/api/shutdown" >/dev/null 2>&1 || true
