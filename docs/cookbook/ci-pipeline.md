> **Note:** This cookbook is a condensed recipe. For the full guide with detailed explanations, see [CI/CD Pipeline Integration](../how-to-guides/ci-pipeline.md).

# CI Pipeline with vrw

This recipe demonstrates how to integrate vrw into a CI/CD pipeline for running and monitoring terminal-aware build steps, with real-time log access and programmatic control.

## Scenario

You have a CI server that needs to run long-running tests or builds. You want to start the build from a CI script, but allow developers to monitor the output in real time from their browsers.

## Setup

### 1. Start a secure vrw instance on the CI server

```bash
# Start vrw in daemon mode with TLS and auth
vrw --remote --tls --port 8443 --daemon \
  --log-file /var/log/vrw-ci.log \
  --retain-on-exit

# Save the token and certificate for later use
TOKEN=$(cat ~/.config/vrw/token)
echo "Token: $TOKEN"
```

Using `--retain-on-exit` ensures that completed build VTTYs remain accessible for inspection after the build finishes.

### 2. Start a build from the CI script

```bash
#!/bin/bash
# ci-build.sh — Start a build via the vrw API

VRW_URL="https://localhost:8443"
TOKEN=$(cat ~/.config/vrw/token)

# Start the build command
RESPONSE=$(curl -sk -X POST "$VRW_URL/api/commands" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "cmd": "cargo",
    "args": ["test", "--verbose", "--color", "always"],
    "env": {"RUST_LOG": "info", "CI": "true"},
    "retain_on_exit": true
  }')

JOB_ID=$(echo "$RESPONSE" | jq -r '.data.id')
echo "Build started: $JOB_ID"

# Poll for completion
while true; do
  STATUS=$(curl -sk -H "Authorization: Bearer $TOKEN" \
    "$VRW_URL/api/commands" \
    | jq -r ".data[] | select(.id == \"$JOB_ID\") | .status")

  if [ "$STATUS" != "running" ]; then
    EXIT_CODE=$(curl -sk -H "Authorization: Bearer $TOKEN" \
      "$VRW_URL/api/commands" \
      | jq -r ".data[] | select(.id == \"$JOB_ID\") | .exit_code")
    echo "Build finished with exit code: $EXIT_CODE"
    break
  fi
  sleep 5
done

# Retrieve the full output
curl -sk -H "Authorization: Bearer $TOKEN" \
  "$VRW_URL/api/commands/$JOB_ID/vtty/partial?offset=0&limit=10000" \
  | jq -r '.data.content' > build-output.txt

exit ${EXIT_CODE:-1}
```

### 3. Monitor the build from a browser

Developers can navigate to `https://ci-server:8443/admin` in their browsers. After accepting the self-signed certificate and entering the bearer token, they see the real-time terminal output of the running build.

### 4. Retrieve build artifacts after completion

```bash
# Get the last 50 lines of output
curl -sk -H "Authorization: Bearer $TOKEN" \
  "$VRW_URL/api/commands/$JOB_ID/vtty/partial?offset=0&limit=50"

# Take a snapshot before a risky step
curl -sk -X POST -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$VRW_URL/api/commands/$JOB_ID/snapshot" \
  -d '{"name": "before-deploy"}'

# Compare output after the step
curl -sk -X POST -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  "$VRW_URL/api/commands/$JOB_ID/diff" \
  -d '{"name": "before-deploy"}'
```

### 5. Clean up retained VTTYs

```bash
# List all commands (including exited ones)
curl -sk -H "Authorization: Bearer $TOKEN" \
  "$VRW_URL/api/commands" \
  | jq '.data[] | select(.status == "exited")'

# Purge old builds
for ID in $(curl -sk -H "Authorization: Bearer $TOKEN" \
  "$VRW_URL/api/commands" \
  | jq -r '.data[] | select(.status == "exited") | .id'); do
  curl -sk -X DELETE -H "Authorization: Bearer $TOKEN" \
    "$VRW_URL/api/commands/$ID"
  echo "Purged $ID"
done
```

## Tips

- Use `--retain-on-exit` on all CI builds so developers can inspect failed builds after the fact.
- Set `--log-pty-raw` to capture raw PTY output for post-mortem debugging with `ansi-replay`.
- Use certificates to isolate different CI pipelines: `vrw cert generate pipeline-a` then `"certificate": "pipeline-a"` in the spawn request.
- The WebSocket endpoint (`/api/commands/{id}/ws`) provides real-time streaming for custom CI dashboard integrations.

## vrc CI Alternative

If you don't need the web dashboard or remote monitoring, you can use `vrc` for local CI pipelines:

```bash
# Run tests in daemon mode
vrc --daemon --stdout-file /tmp/vrc-ci-out.log --stderr-file /tmp/vrc-ci-err.log \
  -- cargo test --verbose

# Poll for completion
while vrc list 2>/dev/null | grep -q "running"; do sleep 5; done

# Capture output
vrc cat --color-always > test-results.txt

# Check exit status
vrc list | grep -o "exit_code=[0-9]*" | tail -1

# Cleanup
vrc stop
```

For CI with display logging:

```bash
vrc --daemon --log /tmp/vrc-ci.log --display -- npm run test
```

This avoids the overhead of starting an HTTP server and is ideal for local CI runners or single-machine pipelines where remote browser monitoring is not needed.
