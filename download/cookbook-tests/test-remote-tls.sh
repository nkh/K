#!/usr/bin/env bash
# test-remote-tls.sh — Tests "Remote Access via TLS" cookbook.
# Validates: TLS startup, HTTPS API, auth token, VTTY over HTTPS.

set -euo pipefail

VRUNNER_BIN="${VRUNNER_BIN:-vrunner}"
PORT=$((19101 + RANDOM % 100))
BASE_URL="https://127.0.0.1:${PORT}"
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

echo "=== Cookbook Test: Remote Access via TLS ==="
echo "Port: $PORT"

section "Start vrunner with --tls --remote"

CERT_DIR="${HOME}/.config/vrunner"
rm -f "${CERT_DIR}/cert.pem" "${CERT_DIR}/key.pem" 2>/dev/null || true

$VRUNNER_BIN --tls --remote --port "$PORT" -- sleep infinity &
VRUNNER_PID=$!

echo "Waiting for TLS server..."
for i in $(seq 1 40); do
    curl -skf "${BASE_URL}/api/info" >/dev/null 2>&1 && { echo "TLS server ready!"; break; }
    sleep 0.2
done

section "HTTPS connectivity"

RESP=$(curl -sk "${BASE_URL}/api/info")
STATUS=$(echo "$RESP" | jq -r '.status')
[ "$STATUS" = "ok" ] && pass "HTTPS GET /api/info works" || fail "HTTPS failed: $RESP"

section "Authentication (remote auto-enables auth)"

CODE=$(curl -sk -o /dev/null -w '%{http_code}' "${BASE_URL}/api/commands")
[ "$CODE" = "401" ] || [ "$CODE" = "403" ] && pass "API rejects unauthenticated (code $CODE)" || fail "Expected 401/403, got $CODE"

TOKEN=$(cat "${CERT_DIR}/token" 2>/dev/null || echo "")
if [ -n "$TOKEN" ]; then
    RESP=$(curl -sk -H "Authorization: Bearer ${TOKEN}" "${BASE_URL}/api/commands")
    STATUS=$(echo "$RESP" | jq -r '.status')
    [ "$STATUS" = "ok" ] && pass "API accepts valid bearer token" || fail "Auth failed: $RESP"
else
    fail "Could not read token file"
fi

section "Auto-generated certificates"

[ -f "${CERT_DIR}/cert.pem" ] && pass "cert.pem exists" || fail "cert.pem not found"
[ -f "${CERT_DIR}/key.pem" ] && pass "key.pem exists" || fail "key.pem not found"

section "Spawn command with auth"

RESP=$(curl -sk -X POST "${BASE_URL}/api/commands" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
ID=$(echo "$RESP" | jq -r '.data.id')
[ "$ID" != "null" ] && pass "Spawned with auth, id=$ID" || fail "Spawn failed: $RESP"

section "VTTY endpoints over HTTPS"

RESP=$(curl -sk -H "Authorization: Bearer ${TOKEN}" "${BASE_URL}/api/commands/${ID}/vtty/html")
[ "$(echo "$RESP" | jq '.data | has("html")')" = "true" ] && pass "vtty/html over HTTPS" || fail "vtty/html failed"

RESP=$(curl -sk -H "Authorization: Bearer ${TOKEN}" "${BASE_URL}/api/commands/${ID}/vtty/text")
[ "$(echo "$RESP" | jq '.data | has("text")')" = "true" ] && pass "vtty/text over HTTPS" || fail "vtty/text failed"

section "Kill & shutdown"
curl -sk -X POST -H "Authorization: Bearer ${TOKEN}" -H "Content-Type: application/json" -d "{}" "${BASE_URL}/api/commands/${ID}/kill" >/dev/null 2>&1 || true
curl -sk -X POST -H "Authorization: Bearer ${TOKEN}" "${BASE_URL}/api/shutdown" >/dev/null 2>&1 || true
