# API Usage

Learn how to programmatically control vrunner using `curl` and shell scripts — from listing and killing commands to sending keystrokes, capturing snapshots, and reading logs.

All examples use `http://localhost:8080`. Replace with your server address as needed. For the full OpenAPI schema, see [`../reference/api.md`](../reference/api.md).

## Listing Commands

Retrieve all spawned commands with their status, PID, and metadata:

```bash
curl -s http://localhost:8080/api/commands | jq .
```

Response:

```json
[
  {
    "id": "cmd_a1b2c3",
    "name": "system-monitor",
    "command": "htop",
    "status": "running",
    "pid": 12345,
    "cwd": "/home/user",
    "started_at": "2025-01-15T10:30:00Z"
  }
]
```

Filter by status with query parameters:

```bash
curl -s "http://localhost:8080/api/commands?status=running" | jq .
```

## Killing Commands

Terminate a specific command by its ID:

```bash
curl -X DELETE http://localhost:8080/api/commands/cmd_a1b2c3
```

Kill all running commands at once:

```bash
curl -X POST http://localhost:8080/api/commands/kill-all
```

## Sending Keystrokes

Send individual keystrokes or control sequences to a running command:

```bash
curl -X POST http://localhost:8080/api/commands/cmd_a1b2c3/input \
  -H "Content-Type: application/json" \
  -d '{"data": "q"}'
```

### Common Escape Sequences

Use these special character sequences in the `data` field:

| Sequence | Key | Example `data` value |
|----------|-----|---------------------|
| `\n` | Enter | `{"data": "\n"}` |
| `\x03` | Ctrl+C | `{"data": "\x03"}` |
| `\x04` | Ctrl+D | `{"data": "\x04"}` |
| `\x1a` | Ctrl+Z | `{"data": "\x1a"}` |
| `\x17` | Ctrl+W | `{"data": "\x17"}` |
| `\x15` | Ctrl+U | `{"data": "\x15"}` |
| `\x0c` | Ctrl+L | `{"data": "\x0c"}` |
| `\x7f` | Backspace | `{"data": "\x7f"}` |
| `\t` | Tab | `{"data": "\t"}` |
| `\x1b[A` | Up arrow | `{"data": "\x1b[A"}` |
| `\x1b[B` | Down arrow | `{"data": "\x1b[B"}` |
| `\x1b[D` | Left arrow | `{"data": "\x1b[D"}` |
| `\x1b[C` | Right arrow | `{"data": "\x1b[C"}` |
| `\x1b` | Escape | `{"data": "\x1b"}` |

Send a multi-character string:

```bash
# Type "ls -la" and press Enter
curl -X POST http://localhost:8080/api/commands/cmd_a1b2c3/input \
  -H "Content-Type: application/json" \
  -d '{"data": "ls -la\n"}'
```

## Freeze and Thaw

Pause a command's output processing (freeze) and resume it later (thaw):

```bash
# Freeze — stops reading from the PTY
curl -X POST http://localhost:8080/api/commands/cmd_a1b2c3/freeze

# Thaw — resumes reading from the PTY
curl -X POST http://localhost:8080/api/commands/cmd_a1b2c3/thaw
```

This is useful for inspecting terminal output without it scrolling away.

## Resizing

Change the terminal dimensions of a running command:

```bash
curl -X POST http://localhost:8080/api/commands/cmd_a1b2c3/resize \
  -H "Content-Type: application/json" \
  -d '{"cols": 120, "rows": 40}'
```

## VTTY Endpoints

vrunner provides multiple ways to retrieve terminal output, each suited to different use cases.

### Full ANSI Output

Returns the complete terminal buffer as raw ANSI text:

```bash
curl -s http://localhost:8080/api/commands/cmd_a1b2c3/vtty/ansi
```

### HTML Output

Returns the terminal rendered as styled HTML, suitable for embedding in web pages:

```bash
curl -s http://localhost:8080/api/commands/cmd_a1b2c3/vtty/html
```

### Partial Output

Retrieve a specific range of rows from the terminal buffer:

```bash
# Get rows 10 through 30
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/vtty/partial?start=10&end=30"
```

## Snapshots and Diffs

Capture terminal state at a point in time and compare snapshots:

```bash
# Create a snapshot
curl -X POST http://localhost:8080/api/commands/cmd_a1b2c3/snapshots \
  -H "Content-Type: application/json" \
  -d '{"label": "after-build"}'

# List all snapshots
curl -s http://localhost:8080/api/commands/cmd_a1b2c3/snapshots | jq .

# Compute a diff between two snapshots
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/snapshots/diff?from=initial&to=after-build"
```

See [`snapshots-diffs.md`](snapshots-diffs.md) for full details.

## Log Reading

Read the persistent log for a command with optional search and pagination:

```bash
# Read the full log (plain text)
curl -s http://localhost:8080/api/commands/cmd_a1b2c3/logs

# Search for a pattern
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/logs?search=error"

# Paginate: skip first 100 lines, return next 50
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/logs?offset=100&limit=50"

# Get ANSI-formatted log with colors
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/logs?format=ansi"

# Get logs as JSON with metadata
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/logs?format=json"
```

## Scripting Example

Here is a complete script that spawns a command, waits for it to finish, and retrieves its logs:

```bash
#!/usr/bin/env bash
set -euo pipefail

SERVER="http://localhost:8080"

# Spawn the command
RESPONSE=$(curl -s -X POST "$SERVER/api/commands" \
  -H "Content-Type: application/json" \
  -d '{"command": "npm run build", "name": "build"}')

CMD_ID=$(echo "$RESPONSE" | jq -r '.id')
echo "Spawned command: $CMD_ID"

# Poll until finished
while true; do
  STATUS=$(curl -s "$SERVER/api/commands/$CMD_ID" | jq -r '.status')
  if [ "$STATUS" != "running" ]; then
    echo "Command finished with status: $STATUS"
    break
  fi
  sleep 2
done

# Retrieve logs
curl -s "$SERVER/api/commands/$CMD_ID/logs?search=error" || true
echo "Logs retrieved."
```

For full endpoint documentation, see [`../reference/api.md`](../reference/api.md).
