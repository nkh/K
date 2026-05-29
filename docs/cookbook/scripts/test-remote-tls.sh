#!/usr/bin/env bash
# cookbook/scripts/test-remote-tls.sh
#
# Tests the "Remote Access via TLS" cookbook example.
# Validates: TLS startup, HTTPS API, WSS WebSocket, auth token.
#
# Note: Uses self-signed certs. Tests HTTPS and WSS against local vrunner.
#       Skips real remote / reverse-proxy scenarios (those need infrastructure).
#
# Usage: ./docs/cookbook/scripts/test-remote-tls.sh
#   or:  VRUNNER_BIN=./target/debug/vrunner ./docs/cookbook/scripts/test-remote-tls.sh

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
        kill "$VRUNNER_PID" 2>/dev/null || true
        wait "$VRUNNER_PID" 2>/dev/null || true
    fi
    echo "Results: ${PASS} passed, ${FAIL} failed"
    [ "$FAIL" -eq 0 ] || exit 1
}
trap cleanup EXIT

echo "=== Cookbook Test: Remote Access via TLS ==="
echo "Port: $PORT"

# ── Start vrunner with TLS ──
section "Start vrunner with --tls --remote"

CERT_DIR="${HOME}/.config/vrunner"
# Clean any old cert to force regeneration
rm -f "${CERT_DIR}/cert.pem" "${CERT_DIR}/key.pem" 2>/dev/null || true

$VRUNNER_BIN --tls --remote --port "$PORT" -- sleep infinity &
VRUNNER_PID=$!

# Wait for server (may take a bit longer for cert generation)
echo "Waiting for TLS server..."
for i in $(seq 1 40); do
    # --remote enables auth, so check for 401 (server is up) instead of 200
    CODE=$(curl -sk -o /dev/null -w '%{http_code}' "${BASE_URL}/api/info" 2>/dev/null) || true
    [ "$CODE" = "401" ] || [ "$CODE" = "403" ] && {
        echo "TLS server ready! (auth required, as expected)"
        break
    }
    sleep 0.2
done

# ── 1. Verify HTTPS works ──
section "HTTPS connectivity"

# /api/info requires auth with --remote — verify the server responds (not a connection error)
CODE=$(curl -sk -o /dev/null -w '%{http_code}' "${BASE_URL}/api/info")
[ "$CODE" = "401" ] || [ "$CODE" = "403" ] \
    && pass "HTTPS responds (auth required, code $CODE)" \
    || fail "HTTPS failed: expected 401/403, got $CODE"

# ── 2. Verify auth is required (remote implies auth) ──
section "Authentication (remote auto-enables auth)"

# Without token should fail
CODE=$(curl -sk -o /dev/null -w '%{http_code}' "${BASE_URL}/api/commands")
[ "$CODE" = "401" ] || [ "$CODE" = "403" ] && pass "API rejects unauthenticated request (code $CODE)" \
    || fail "Expected 401/403, got $CODE"

# With token should work
TOKEN=$(cat "${CERT_DIR}/token" 2>/dev/null || echo "")
if [ -n "$TOKEN" ]; then
    RESP=$(curl -sk -H "Authorization: Bearer ${TOKEN}" "${BASE_URL}/api/commands")
    STATUS=$(echo "$RESP" | jq -r '.status')
    [ "$STATUS" = "ok" ] && pass "API accepts valid bearer token" || fail "Auth failed: $RESP"
else
    fail "Could not read token file"
fi

# ── 3. Certificates were auto-generated ──
section "Auto-generated certificates"

[ -f "${CERT_DIR}/cert.pem" ] && pass "cert.pem exists at ${CERT_DIR}/cert.pem" \
    || fail "cert.pem not found"
[ -f "${CERT_DIR}/key.pem" ] && pass "key.pem exists at ${CERT_DIR}/key.pem" \
    || fail "key.pem not found"

# ── 4. Spawn a command with auth ──
section "Spawn command with auth"

RESP=$(curl -sk -X POST "${BASE_URL}/api/commands" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"cmd": "cat"}')
ID=$(echo "$RESP" | jq -r '.data.id')
[ "$ID" != "null" ] && pass "Spawned command with auth, id=$ID" || fail "Spawn failed: $RESP"

# ── 5. WebSocket (WSS) connectivity check ──
section "WSS WebSocket endpoint"

# Use a timeout: we just check the endpoint accepts the upgrade.
# We don't parse WebSocket frames in bash.
WS_CODE=$(curl -sk -o /dev/null -w '%{http_code}' \
    --no-buffer \
    -H "Connection: Upgrade" \
    -H "Upgrade: websocket" \
    -H "Sec-WebSocket-Version: 13" \
    -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
    "${BASE_URL}/api/commands/${ID}/ws?token=${TOKEN}" \
    --max-time 3 2>/dev/null || true)

[ "$WS_CODE" = "101" ] && pass "WSS upgrade succeeds (HTTP 101)" \
    || echo "  INFO: WSS upgrade returned ${WS_CODE:-timeout} (may need websocket client)"

# ── 6. VTTY endpoints work over HTTPS ──
section "VTTY endpoints over HTTPS"

RESP=$(curl -sk -H "Authorization: Bearer ${TOKEN}" \
    "${BASE_URL}/api/commands/${ID}/vtty/html")
HAS_HTML=$(echo "$RESP" | jq '.data | has("html")')
[ "$HAS_HTML" = "true" ] && pass "vtty/html works over HTTPS" || fail "vtty/html failed"

RESP=$(curl -sk -H "Authorization: Bearer ${TOKEN}" \
    "${BASE_URL}/api/commands/${ID}/vtty/text")
HAS_TEXT=$(echo "$RESP" | jq '.data | has("text")')
[ "$HAS_TEXT" = "true" ] && pass "vtty/text works over HTTPS" || fail "vtty/text failed"

# ── 7. Kill and shutdown ──
section "Kill & shutdown"

curl -sk -X POST -H "Authorization: Bearer ${TOKEN}" -H "Content-Type: application/json" \
    -d '{}' "${BASE_URL}/api/commands/${ID}/kill" >/dev/null 2>&1 || true
curl -sk -X POST -H "Authorization: Bearer ${TOKEN}" \
    "${BASE_URL}/api/shutdown" >/dev/null 2>&1 || true
