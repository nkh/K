# CI/CD Pipeline Integration

Learn how to integrate vrunner into your CI/CD pipeline to start secure instances, run builds, monitor progress from a browser, retrieve artifacts, and clean up automatically.

## Overview

vrunner fits naturally into CI pipelines as a persistent command runner that provides:

- A **web dashboard** to monitor builds in real time from any browser.
- A **REST API** to programmatically control and query command state.
- **Persistent logs** for post-build analysis and artifact retrieval.

## Step 1: Start a Secure vrunner Instance

Start vrunner at the beginning of your pipeline. Use `--daemon` to background it and `--tls` to secure access:

```yaml
# .gitlab-ci.yml (GitLab example)
stages:
  - test
  - cleanup

variables:
  VRUNNER_PORT: "9090"
  VRUNNER_LOG: "/tmp/vrunner-ci.log"

test:
  stage: test
  before_script:
    # Start vrunner as a daemon with TLS
    - |
      vrunner --daemon \
        --port $VRUNNER_PORT \
        --tls \
        --cert /etc/ssl/ci-cert.pem \
        --key /etc/ssl/ci-key.pem \
        --log $VRUNNER_LOG
  script:
    # Spawn test commands
    - |
      curl -sk -X POST https://localhost:$VRUNNER_PORT/api/commands \
        -H "Content-Type: application/json" \
        -d '{"command": "npm run test:unit", "name": "unit-tests"}'
    - |
      curl -sk -X POST https://localhost:$VRUNNER_PORT/api/commands \
        -H "Content-Type: application/json" \
        -d '{"command": "npm run test:integration", "name": "integration-tests"}'
    # Wait for all commands to finish
    - |
      while true; do
        RUNNING=$(curl -sk https://localhost:$VRUNNER_PORT/api/commands | jq '[.[] | select(.status=="running")] | length')
        if [ "$RUNNING" -eq 0 ]; then break; fi
        sleep 5
      done
    # Check results and retrieve logs
    - |
      for CMD_NAME in unit-tests integration-tests; do
        CMD_ID=$(curl -sk https://localhost:$VRUNNER_PORT/api/commands | jq -r ".[] | select(.name==\"$CMD_NAME\") | .id")
        STATUS=$(curl -sk "https://localhost:$VRUNNER_PORT/api/commands/$CMD_ID" | jq -r '.exit_code')
        curl -sk "https://localhost:$VRUNNER_PORT/api/commands/$CMD_ID/logs" > "logs-${CMD_NAME}.txt"
        if [ "$STATUS" != "0" ]; then
          echo "FAILED: $CMD_NAME (exit code $STATUS)"
          exit 1
        fi
      done
  artifacts:
    paths:
      - logs-*.txt
    when: always

cleanup:
  stage: cleanup
  when: always
  script:
    - vrunner daemon stop --port $VRUNNER_PORT || true
```

## Step 2: Start Builds from CI Scripts

Spawn commands directly from your pipeline script:

```bash
# Run lint, build, and test in parallel
for JOB in lint build test; do
  curl -sk -X POST https://localhost:9090/api/commands \
    -H "Content-Type: application/json" \
    -d "{\"command\": \"npm run $JOB\", \"name\": \"$JOB\"}"
done
```

## Step 3: Monitor from a Browser

During the pipeline run, open the dashboard in your browser:

```
https://ci-runner.example.com:9090/admin
```

You can watch each command's output in real time, search terminals, and check status badges. This is especially useful for:

- **Debugging flaky tests** — Watch output live instead of waiting for the pipeline to finish.
- **Long-running builds** — Monitor progress without polling logs.
- **Parallel job coordination** — See all jobs side by side.

## Step 4: Retrieve Artifacts

After commands complete, download their output:

```bash
# Get plain text logs
curl -sk https://localhost:9090/api/commands/cmd_abc123/logs > build-output.txt

# Get ANSI-formatted logs (preserves colors)
curl -sk "https://localhost:9090/api/commands/cmd_abc123/logs?format=ansi" > build-colored.txt

# Search for specific patterns
curl -sk "https://localhost:9090/api/commands/cmd_abc123/logs?search=error" > errors.txt

# Export terminal as HTML for a build report
curl -sk https://localhost:9090/api/commands/cmd_abc123/vtty/html > build-report.html
```

## Step 5: Cleanup

Always stop the vrunner instance at the end of the pipeline, even if earlier steps failed:

```yaml
after_script:
  - vrunner daemon stop --port $VRUNNER_PORT || true
```

The `|| true` ensures cleanup runs even if vrunner has already stopped or was never started.

## GitHub Actions Example

```yaml
name: Build and Test
on: [push]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install vrunner
        run: curl -fsSL https://get.vrunner.dev | sh

      - name: Start vrunner
        run: |
          vrunner --daemon --port 9090 --log /tmp/vrunner.log

      - name: Run tests
        run: |
          curl -s -X POST http://localhost:9090/api/commands \
            -H "Content-Type: application/json" \
            -d '{"command": "npm run test", "name": "tests"}'
          # Poll for completion
          while true; do
            STATUS=$(curl -s http://localhost:9090/api/commands | jq -r '.[0].status')
            [ "$STATUS" != "running" ] && break
            sleep 3
          done

      - name: Collect logs
        if: always()
        run: |
          CMD_ID=$(curl -s http://localhost:9090/api/commands | jq -r '.[0].id')
          curl -s "http://localhost:9090/api/commands/$CMD_ID/logs" > test-results.log

      - name: Cleanup
        if: always()
        run: vrunner daemon stop --port 9090 || true

      - name: Upload logs
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: test-logs
          path: test-results.log
```

## Best Practices

- **Always use `--daemon`** in CI to prevent the runner from blocking the pipeline.
- **Use `when: always`** on cleanup steps to ensure the daemon is stopped even on failure.
- **Secure with `--tls`** if the CI runner is accessible from the network.
- **Use `--no-env`** to isolate spawned commands from CI environment variables if they may interfere.
- **Set resource limits** in your config to prevent commands from consuming excessive memory or CPU.

For API details, see [`../reference/api.md`](../reference/api.md). For configuration options, see [`configuration-profiles.md`](configuration-profiles.md).
