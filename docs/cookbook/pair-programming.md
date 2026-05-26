# Pair Programming Setup

This recipe shows how to use vrunner's web interface for pair programming, allowing two or more developers to share a terminal session through a browser.

## Scenario

Two developers want to collaborate on the same code. One is running a local development server, and both need to see the same terminal output and send input.

## Setup

### 1. One developer starts the shared vrunner instance

```bash
# Developer 1: Start vrunner with a shared coding session
vrunner --port 8080 --display-all --tabs -- vim pair-project.rs
```

Or start it as a daemon for long-running sessions:

```bash
vrunner --port 8080 --daemon
```

### 2. Start a shared command via the API

```bash
# Start a shared editing session
SESSION_ID=$(curl -s -X POST http://localhost:8080/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "vim", "args": ["src/main.rs"]}' \
  | jq -r '.data.id')

echo "Session ID: $SESSION_ID"
```

### 3. Both developers connect from their browsers

Navigate to `http://developer1-machine:8080/<SESSION_ID>` or `http://developer1-machine:8080/admin` and click on the command.

Both developers see the same terminal output in real time. Keystrokes sent by either developer are forwarded to the shared process.

### 4. For remote teams (over the internet)

```bash
# On the shared server
vrunner --remote --tls --port 443 --daemon

# Share the token and certificate
TOKEN=$(cat ~/.config/vrunner/token)
echo "Token: $TOKEN"
echo "Certificate at: ~/.config/vrunner/cert.pem"
```

Remote developers connect to `https://shared-server:443/admin` and provide the bearer token when prompted.

## Advanced: Multiple Pair Sessions

Run multiple isolated pair sessions on the same server:

```bash
# Session A — Frontend pair
curl -s -X POST http://localhost:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "vim", "args": ["frontend/App.tsx"]}'

# Session B — Backend pair
curl -s -X POST http://localhost:9090/api/commands \
  -H "Content-Type: application/json" \
  -d '{"cmd": "vim", "args": ["src/api/handler.rs"]}'
```

Use certificates to restrict access per session:

```bash
vrunner cert generate frontend-pair
vrunner cert generate backend-pair

# Frontend pair uses their certificate token
# Backend pair uses their certificate token
```

## Tips

- The web UI supports WebSocket streaming, so both developers see output with sub-second latency.
- Use the split-pane view (`Ctrl+S` in interactive display) to monitor multiple sessions.
- Use `--retain-on-exit` to keep the session accessible if both developers disconnect temporarily.
- Mouse interaction works in the web UI — both developers can scroll through output history independently.
