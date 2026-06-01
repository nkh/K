# Snapshots and Diffs

Learn how to capture terminal state at specific moments, list saved snapshots, compute differences between them, and use them for automated testing and debugging.

> **This guide covers the vrw HTTP API for snapshots and diffs.** This feature requires vrw (the HTTP server + web dashboard binary). For local-only terminal capture in vrc, use `vrc cat --color-always > output.txt` to save VTTY output manually.

## What Are Snapshots?

A snapshot is a point-in-time capture of a command's terminal buffer — including all visible text, cursor position, and scrollback history. Snapshots are stored in memory and can be compared to detect changes in terminal output.

## Storing a Snapshot

Capture a snapshot of a running command via the API:

```bash
curl -X POST http://localhost:8080/api/commands/cmd_a1b2c3/snapshots \
  -H "Content-Type: application/json" \
  -d '{"label": "initial-state"}'
```

Response:

```json
{
  "id": "snap_xyz789",
  "label": "initial-state",
  "command_id": "cmd_a1b2c3",
  "created_at": "2025-01-15T10:30:00Z",
  "rows": 24,
  "cols": 80
}
```

You can also take a snapshot from the web UI by right-clicking a terminal pane and selecting **Take Snapshot**, or by pressing `Ctrl+Shift+S` in the focused terminal.

## Listing Snapshots

Retrieve all snapshots for a command:

```bash
curl -s http://localhost:8080/api/commands/cmd_a1b2c3/snapshots | jq .
```

Response:

```json
[
  {
    "id": "snap_xyz789",
    "label": "initial-state",
    "created_at": "2025-01-15T10:30:00Z",
    "rows": 24,
    "cols": 80
  },
  {
    "id": "snap_abc456",
    "label": "after-build",
    "created_at": "2025-01-15T10:35:00Z",
    "rows": 24,
    "cols": 80
  }
]
```

## Computing a Diff

Compare two snapshots to see what changed between them:

```bash
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/snapshots/diff?from=initial-state&to=after-build"
```

Response:

```json
{
  "from": "initial-state",
  "to": "after-build",
  "added": [
    {"row": 5, "text": "Build completed successfully in 12.3s"}
  ],
  "removed": [
    {"row": 3, "text": "Building..."}
  ],
  "changed": [
    {
      "row": 10,
      "old_text": "Tests: 0 passed",
      "new_text": "Tests: 42 passed, 0 failed"
    }
  ]
}
```

### Comparing by Snapshot ID

You can also use snapshot IDs instead of labels:

```bash
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/snapshots/diff?from=snap_xyz789&to=snap_abc456"
```

### Comparing to Live State

Omit the `to` parameter to compare a snapshot to the current terminal state:

```bash
curl -s "http://localhost:8080/api/commands/cmd_a1b2c3/snapshots/diff?from=initial-state"
```

## Deleting Snapshots

Remove a single snapshot:

```bash
curl -X DELETE http://localhost:8080/api/commands/cmd_a1b2c3/snapshots/initial-state
```

Remove all snapshots for a command:

```bash
curl -X DELETE http://localhost:8080/api/commands/cmd_a1b2c3/snapshots
```

## Use Case: Automated Testing

Snapshots are useful for verifying that a command produces expected output. Here's a script that runs a build, captures before/after states, and checks for expected changes:

```bash
#!/usr/bin/env bash
set -euo pipefail

SERVER="http://localhost:8080"
CMD_NAME="build"

# Get the command ID
CMD_ID=$(curl -s "$SERVER/api/commands" | jq -r ".[] | select(.name==\"$CMD_NAME\") | .id")

# Take a "before" snapshot
curl -s -X POST "$SERVER/api/commands/$CMD_ID/snapshots" \
  -H "Content-Type: application/json" \
  -d '{"label": "before-build"}' > /dev/null

# Trigger the build by sending Enter
curl -s -X POST "$SERVER/api/commands/$CMD_ID/input" \
  -H "Content-Type: application/json" \
  -d '{"data": "\n"}' > /dev/null

# Wait for the build to finish
sleep 30

# Take an "after" snapshot
curl -s -X POST "$SERVER/api/commands/$CMD_ID/snapshots" \
  -H "Content-Type: application/json" \
  -d '{"label": "after-build"}' > /dev/null

# Compute the diff
DIFF=$(curl -s "$SERVER/api/commands/$CMD_ID/snapshots/diff?from=before-build&to=after-build")

# Check for "Build completed successfully"
if echo "$DIFF" | jq -r '.added[].text' | grep -q "Build completed successfully"; then
  echo "PASS: Build succeeded"
else
  echo "FAIL: Expected 'Build completed successfully' not found"
  echo "$DIFF" | jq .
  exit 1
fi

# Clean up
curl -s -X DELETE "$SERVER/api/commands/$CMD_ID/snapshots/before-build" > /dev/null
curl -s -X DELETE "$SERVER/api/commands/$CMD_ID/snapshots/after-build" > /dev/null
```

## Use Case: Debugging State Changes

When debugging an interactive command, take snapshots at key points:

```bash
# Navigate to a specific screen in your app
# ... send keystrokes ...

# Capture the state
curl -X POST http://localhost:8080/api/commands/cmd_abc/snapshots \
  -d '{"label": "screen-main-menu"}'

# Navigate deeper
# ... send more keystrokes ...

# Capture again
curl -X POST http://localhost:8080/api/commands/cmd_abc/snapshots \
  -d '{"label": "screen-settings"}'

# Compare the two screens
curl -s "http://localhost:8080/api/commands/cmd_abc/snapshots/diff?from=screen-main-menu&to=screen-settings" | jq .
```

This helps you see exactly what changed on screen between interactions.

## Best Practices

- **Use descriptive labels** — `before-deploy`, `after-migration`, `error-state` are more useful than `snap1`, `snap2`.
- **Clean up after testing** — Delete snapshots when you no longer need them to free memory.
- **Freeze output before snapshotting** — Use the freeze endpoint to pause output and ensure a clean capture:

  ```bash
  curl -X POST http://localhost:8080/api/commands/cmd_abc/freeze
  curl -X POST http://localhost:8080/api/commands/cmd_abc/snapshots -d '{"label": "frozen-state"}'
  curl -X POST http://localhost:8080/api/commands/cmd_abc/thaw
  ```

For the full API reference, see [`../api.md`](../api.md). For sending keystrokes, see [`api-usage.md`](api-usage.md).
