# Snapshots and Diffs

Learn how to capture terminal state at specific moments, list saved snapshots, compute differences between them, and use them for automated testing and debugging.

> **This guide covers the vrw HTTP API for snapshots and diffs.** This feature requires vrw (the HTTP server + web dashboard binary). For local-only terminal capture in vrc, use `vrc cat <pid>` to view VTTY output.

## What Are Snapshots?

A snapshot is a point-in-time capture of a command's terminal buffer — including all visible text, cursor position, and scrollback history. Snapshots are stored in memory and can be compared to detect changes in terminal output.

## Storing a Snapshot

Capture a snapshot of a running command via the API:

```bash
curl -X POST http://localhost:9090/api/commands/cmd_a1b2c3/snapshot \
  -H "Content-Type: application/json" \
  -d '{"name": "initial-state"}'
```

Response:

```json
{
  "status": "ok",
  "data": {
    "id": "snap_xyz789",
    "name": "initial-state",
    "command_name": "my-command",
    "command_args": [],
    "pid": 12345,
    "timestamp": "2025-01-15T10:30:00Z",
    "runtime_secs": 30
  }
}
```

You can also take a snapshot from the web UI by right-clicking a terminal pane and selecting **Take Snapshot**.

> **Note:** `Ctrl+Shift+S` / `Alt+S` toggles text selection mode, not snapshot capture. Use the context menu or panel header for snapshots.

## Listing Snapshots

Retrieve all snapshots for a command:

```bash
curl -s http://localhost:9090/api/commands/cmd_a1b2c3/snapshots | jq .
```

## Computing a Diff

Compare a snapshot to the current live terminal state:

```bash
curl -s -X POST http://localhost:9090/api/commands/cmd_a1b2c3/diff \
  -H "Content-Type: application/json" \
  -d '{"name": "initial-state"}' | jq .
```

## Deleting Snapshots

Remove a single snapshot:

```bash
curl -X DELETE http://localhost:9090/api/commands/cmd_a1b2c3/snapshots/initial-state
```

## Use Case: Automated Testing

Snapshots are useful for verifying that a command produces expected output. Here's a script that runs a build, captures before/after states, and checks for expected changes:

```bash
#!/usr/bin/env bash
set -euo pipefail

SERVER="http://localhost:9090"
CMD_ID="cmd_a1b2c3"

# Take a "before" snapshot
curl -s -X POST "$SERVER/api/commands/$CMD_ID/snapshot" \
  -H "Content-Type: application/json" \
  -d '{"name": "before-build"}' > /dev/null

# Trigger the build by sending Enter
curl -s -X POST "$SERVER/api/commands/$CMD_ID/keys" \
  -H "Content-Type: application/json" \
  -d '{"data": "\n"}' > /dev/null

# Wait for the build to finish
sleep 30

# Take an "after" snapshot
curl -s -X POST "$SERVER/api/commands/$CMD_ID/snapshot" \
  -H "Content-Type: application/json" \
  -d '{"name": "after-build"}' > /dev/null

# Compute the diff (compare snapshot to live state)
DIFF=$(curl -s -X POST "$SERVER/api/commands/$CMD_ID/diff" \
  -H "Content-Type: application/json" \
  -d '{"name": "before-build"}')

# Check for "Build completed successfully"
if echo "$DIFF" | jq -r '.data.added[].text' 2>/dev/null | grep -q "Build completed successfully"; then
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
curl -X POST http://localhost:9090/api/commands/cmd_abc/snapshot \
  -d '{"name": "screen-main-menu"}'

# Navigate deeper
# ... send more keystrokes ...

# Capture again
curl -X POST http://localhost:9090/api/commands/cmd_abc/snapshot \
  -d '{"name": "screen-settings"}'

# Compare the earlier snapshot to live state
curl -s -X POST "http://localhost:9090/api/commands/cmd_abc/diff" \
  -d '{"name": "screen-main-menu"}' | jq .
```

This helps you see exactly what changed on screen between interactions.

## Best Practices

- **Use descriptive names** — `before-deploy`, `after-migration`, `error-state` are more useful than `snap1`, `snap2`.
- **Clean up after testing** — Delete snapshots when you no longer need them to free memory.
- **Freeze output before snapshotting** — Use the freeze endpoint to pause output and ensure a clean capture:

  ```bash
  curl -X POST http://localhost:9090/api/commands/cmd_abc/freeze
  curl -X POST http://localhost:9090/api/commands/cmd_abc/snapshot -d '{"name": "frozen-state"}'
  curl -X POST http://localhost:9090/api/commands/cmd_abc/thaw
  ```

For the full API reference, see [`../api.md`](../api.md). For sending keystrokes, see [`api-usage.md`](api-usage.md).