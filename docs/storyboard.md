# vrw Introduction — Video Storyboard

A 3-minute video introducing vrw's key features and workflow. Target audience: developers and DevOps engineers looking for a web-first terminal management tool.

---

## Scene 1: The Problem (0:00 – 0:25)

**Visual**: Split screen showing a developer frustrated with multiple terminal tabs. They have a frontend dev server, backend API, database, and test runner all in separate terminal windows. The screen is cluttered.

**Narrator**: "When you're developing multiple services, your terminal becomes a mess. You juggle tabs, lose track of which process is which, and have no way to share terminal output with your team."

**Visual**: Developer accidentally closes the wrong terminal, losing build output. They groan.

**Text overlay**: "What if you could manage all your terminals from a single browser tab?"

---

## Scene 2: Introducing vrw (0:25 – 0:55)

**Visual**: Clean terminal showing:

```bash
$ vrw --daemon
vrw started (PID 12345)
Listening on http://127.0.0.1:9090
```

**Narrator**: "Meet vrw — a virtual terminal runner with a web-first control plane. It runs your commands in pseudo-terminals and exposes them through a REST API and built-in admin dashboard."

**Visual**: Browser opens to `http://localhost:9090/admin`. Clean, dark-themed dashboard with an empty command list.

**Visual**: Terminal showing API spawn:

```bash
$ curl -X POST http://localhost:9090/api/commands \
  -d '{"cmd": "htop"}'
{"status":"ok","data":{"id":"550e8400-..."}}
```

**Visual**: Dashboard updates in real-time showing the htop session with live terminal output.

**Narrator**: "Commands can be started from the CLI, the API, or the web UI itself. Once running, you can monitor and control them from any interface."

---

## Scene 3: Multi-Service Orchestration (0:55 – 1:30)

**Visual**: Rapid succession — three API calls spawn three services:

```bash
# Frontend
curl -X POST http://localhost:9090/api/commands \
  -d '{"cmd":"npm","args":["run","dev"]}'

# Backend
curl -X POST http://localhost:9090/api/commands \
  -d '{"cmd":"cargo","args":["run"]}'

# Database
curl -X POST http://localhost:9090/api/commands \
  -d '{"cmd":"docker","args":["compose","up"]}'
```

**Visual**: Dashboard sidebar populates with all three commands. The main pane shows live terminal output. Click between commands — output switches instantly.

**Narrator**: "Run all your services from one place. The dashboard streams terminal output in real-time using an incremental diff protocol — only changed cells are transmitted, making it bandwidth-efficient even over slow connections."

**Visual**: Zoom into the WebSocket diff visualization — a grid of cells with only a few highlighted in yellow (changed cells).

**Visual**: Split-pane view (`Ctrl+S`) showing two terminals side by side.

**Narrator**: "Use split-pane mode to monitor two services simultaneously. Or use the tab bar to quickly switch between any number of commands."

---

## Scene 4: Remote Access & Security (1:30 – 2:05)

**Visual**: Terminal showing:

```bash
$ vrw --remote --tls --port 443 --daemon
```

**Visual**: A laptop on a different network opens a browser to `https://server:443/admin`. The padlock icon shows it's using TLS. A token prompt appears.

**Narrator**: "Need to access your terminals remotely? vrw supports TLS encryption and bearer token authentication out of the box. Certificates are auto-generated — just add `--remote --tls`."

**Visual**: Certificate management screen showing `vrw cert generate frontend-team`. Two different browser windows connect with different tokens, each seeing only their own commands.

**Narrator**: "Named certificates provide per-command access isolation. Give the frontend team their own token, and they can only interact with their own commands."

---

## Scene 5: The CLI & Interactive Display (2:05 – 2:40)

**Visual**: Full-screen terminal showing:

```bash
$ vrw --display-all --tabs -- cargo test
```

**Visual**: Terminal renders the test output with full ANSI colors. Tab bar at the top shows all commands. `Ctrl+Right` switches to the next command. `Ctrl+F` opens a search bar. Mouse selection copies text to clipboard.

**Narrator**: "Prefer the terminal? The interactive display mode gives you a full TUI with tabs, search, copy/paste, split-pane, and scrollback — all without leaving your terminal."

**Visual**: `Ctrl+S` activates split-pane. Left pane shows `cargo test`, right pane shows `npm run dev`.

**Visual**: Context menu on right-click showing Kill, Purge, Copy ID, Restart actions.

---

## Scene 6: API-First Integration (2:40 – 3:00)

**Visual**: A CI/CD pipeline diagram. A GitHub Actions workflow triggers a vrw API call to start a build. The build output streams to a monitoring dashboard.

```bash
JOB_ID=$(curl -s -X POST https://ci-server/api/commands \
  -d '{"cmd":"./run-tests.sh","retain_on_exit":true}' \
  | jq -r '.data.id')
```

**Narrator**: "vrw is API-first. Start commands from CI scripts, monitor them from dashboards, control them from automation — all through a clean REST API with 30+ endpoints."

**Visual**: Fade to logo + tagline.

**Text overlay**:
> **vrw**
> A virtual terminal runner and process orchestrator with a web-first control plane.
>
> github.com/nkh/K

---

## Production Notes

- **Total runtime**: ~3 minutes
- **Pacing**: Fast cuts for API calls (0.5s each), slower pans for dashboard demos (2-3s)
- **Color scheme**: Dark terminal background, green/cyan ANSI colors, blue UI accents
- **Screen recording**: Use `asciinema` for terminal portions, browser DevTools recorder for web UI
- **Audio**: Calm electronic background music, clear narration
- **Aspect ratio**: 16:9 (1920x1080)
