# Pair Programming Sessions

Learn how to share live terminal sessions between developers for pair programming — locally on the same network or remotely with TLS.

## How It Works

vrunner's web UI and WebSocket support make it natural for pair programming: one developer starts a vrunner instance with the shared commands, and the other developer connects to the same dashboard. Both see the same terminal output and can type into the focused terminal.

## Local Pair Programming

When two developers are on the same network:

### Developer A (Host)

Start vrunner with remote access enabled:

```bash
vrunner --remote --port 8080 --web \
  --cmd "vim main.py" --name "editor"
```

This binds to `0.0.0.0:8080`, making the dashboard accessible across the LAN.

### Developer B (Guest)

Open the dashboard in their browser:

```
http://192.168.1.100:8080/admin
```

Replace `192.168.1.100` with Developer A's LAN IP address.

Both developers now see the same `vim` session. Click on the terminal to focus it and type. The last person to click has keyboard control.

### Multiple Sessions

Run multiple shared terminals side by side:

```bash
vrunner --remote --port 8080 --web \
  --cmd "vim main.py" --name "editor" \
  --cmd "pytest -f" --name "tests" \
  --cmd "git log --oneline -20" --name "history"
```

Each developer can click into any terminal. Use the sidebar search to quickly find the right session.

## Remote Pair Programming

For remote developers, secure the connection with TLS:

### Step 1: Generate Certificates

```bash
# Generate a shared certificate for the pair session
vrunner cert generate --name pair-session --commands "*"
```

This outputs `pair-session-cert.pem`, `pair-session-key.pem`, and `pair-session-token`.

### Step 2: Start with TLS

**Developer A** starts the instance:

```bash
vrunner --remote --tls --port 8443 \
  --cert /etc/ssl/server-cert.pem \
  --key /etc/ssl/server-key.pem \
  --cmd "vim main.py" --name "editor" \
  --cmd "npm run test" --name "tests"
```

### Step 3: Share the Certificate

Developer A sends `pair-session-cert.pem` and `pair-session-key.pem` to Developer B through a secure channel (Signal, encrypted email, etc.).

### Step 4: Connect

**Developer B** opens the dashboard with their certificate configured in the browser:

```
https://your-server.example.com:8443/admin
```

They can also connect via curl:

```bash
curl --cert pair-session-cert.pem --key pair-session-key.pem \
  https://your-server.example.com:8443/api/commands
```

### Step 5: Use the Token (Alternative)

If certificate configuration in the browser is inconvenient, Developer B can use the bearer token:

```
https://your-server.example.com:8443/admin?token=vr_tok_abcd1234
```

Or via API:

```bash
curl -H "Authorization: Bearer vr_tok_abcd1234" \
  https://your-server.example.com:8443/api/commands
```

## Managing Multiple Pair Sessions

If you have multiple pairs working simultaneously, use certificates to isolate sessions:

```bash
# Pair 1: Alice and Bob
vrunner cert generate --name pair-alice-bob --commands "editor-ab,tests-ab"

# Pair 2: Carol and Dave
vrunner cert generate --name pair-carol-dave --commands "editor-cd,tests-cd"
```

Start one vrunner instance with all commands:

```bash
vrunner --remote --tls --port 8443 \
  --cert server-cert.pem --key server-key.pem \
  --cmd "vim project-a/main.py" --name "editor-ab" \
  --cmd "pytest project-a/" --name "tests-ab" \
  --cmd "vim project-b/main.py" --name "editor-cd" \
  --cmd "pytest project-b/" --name "tests-cd"
```

Each pair uses their certificate and sees only their own terminals.

## Keyboard Conflict Resolution

Since both developers share the same terminal:

- **Visual indicator** — The focused terminal shows which user last clicked it (via a colored border or name tag).
- **Communication** — Use voice chat (or the chat feature if available) to coordinate who has keyboard control.
- **Take turns** — Click the terminal to take control, then click away when done.

## Temporary Sessions

For ad-hoc sessions that don't need certificates:

```bash
# Quick local session (no auth)
vrunner --remote --port 8080 --web --cmd "vim main.py" --name "editor"
```

> **Warning:** Without TLS and certificates, anyone on the network can connect and type into your terminals. Use this only on trusted networks.

## Cleanup After a Session

When the pair programming session is over, clean up:

```bash
# Revoke the session certificate
vrunner cert remove pair-session

# Stop vrunner
vrunner daemon stop --port 8443
```

Delete the certificate files from both developers' machines.

## Tips for Effective Pair Sessions

- **Use the sidebar search** to quickly switch between terminals.
- **Export output** after debugging sessions to save findings.
- **Enable browser notifications** so both developers get alerts if a command exits.
- **Use command-name URLs** like `https://host:8443/admin/editor` for direct links.
- **Record the session** — Use a screen recorder on the dashboard for later review.

For TLS setup details, see [`remote-tls.md`](remote-tls.md). For certificate management, see [`certificates.md`](certificates.md).
