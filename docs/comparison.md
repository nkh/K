# vrw vs Alternatives

A feature comparison between vrw and similar tools for running and managing terminal processes.

## Feature Comparison Matrix

| Feature | vrw | tmux | screen | mprocs | gotty | wetty |
|---------|---------|------|--------|--------|-------|-------|
| **Language** | Rust | C | C | Go | Go | Node.js |
| **Binary Size** | ~5MB static | ~2MB | ~1.5MB | ~3MB | ~8MB + deps | ~40MB + deps |
| **Runtime Deps** | None | libevent, ncurses | ncurses | None | Go stdlib | Node.js |
| **Web Dashboard** | Embedded SPA | No | No | Web UI | Web UI | Web UI |
| **REST API** | 30+ endpoints | No | No | No | Limited | Limited |
| **WebSocket** | Incremental diff | No | No | No | Yes | Yes |
| **TLS** | Built-in (rustls) | No | No | No | Built-in | SSH |
| **Auth** | Bearer + certificates | Socket perms | Socket perms | None | Password | SSH |
| **Per-Command Auth** | Certificate pool | No | No | No | No | No |
| **Daemon Mode** | Double-fork | Server mode | Detach | No | No | No |
| **Multi-Instance** | Yes (registry) | Sessions | Sessions | No | No | No |
| **Config Files** | YAML/TOML/JSON | .tmux.conf | .screenrc | CLI only | CLI only | CLI only |
| **Config Profiles** | Named profiles | No | No | No | No | No |
| **Mouse Support** | Full (CLI + Web) | Yes | Limited | No | No | Yes |
| **Scrollback** | Configurable | Fixed | Fixed | No | No | Fixed |
| | | (default 5000) | | | | |
| **Search** | Built-in (CLI + Web) | No | No | No | No | No |
| **Copy/Paste** | Mouse selection | Yes | Limited | No | No | Yes |
| **Split-Pane** | Ctrl+S toggle | Yes | Yes | No | No | No |
| **Tab Bar** | Optional tab bar | Windows | No | No | No | No |
| **Snapshots/Diffs** | Yes | No | No | No | No | No |
| **Sixel Images** | Yes | No | No | No | No | No |
| **Alt Screen** | Auto-recovery | Yes | Yes | No | No | Yes |
| **Scroll Regions** | Full support | Yes | No | No | No | Yes |
| **Bracketed Paste** | Yes | Yes | Yes | No | No | Yes |
| **Focus Reporting** | Yes | Yes | No | No | No | No |
| **Terminal Bell** | Yes (visual) | No | No | No | No | No |
| **Exit Handlers** | on_exit/on_error | No | No | No | No | No |
| **Retain on Exit** | Yes | No | No | No | No | No |
| **Freeze/Thaw** | SIGSTOP/SIGCONT | No | No | No | No | No |
| **Resize** | CLI + API + WS | Yes | Yes | No | No | No |
| **Custom Terminal Size** | Per-command | Per-window | Per-window | No | No | No |
| **Env Var Layers** | 3-layer | Inherit | Inherit | No | No | No |
| **Certificate Pool** | Named certs | No | No | No | No | No |
| **Hooks** | Configurable | No | No | No | No | No |
| **PTY Raw Log** | With replay tool | No | No | No | No | No |

## When to Choose vrw

**vrw excels when:**

- You need a **web API** to programmatically manage terminal processes from scripts, CI/CD pipelines, or custom dashboards
- You want a **single binary with zero external dependencies** — no Node.js, no Go runtime, no ncurses on the target machine
- You need **per-command access control** via certificates, allowing different clients to interact with specific commands
- You want **real-time browser-based monitoring** without installing anything on client machines — just open a URL
- You are building a **CI/CD system** where a script starts a command in one environment and another monitors it from another
- You want **TLS + auth built in** without reverse proxy configuration
- You need **terminal-aware process management** (full PTY with ANSI, mouse, sixel images)

## When to Choose Alternatives

**Choose tmux when:**
- You need a persistent terminal multiplexer for your own SSH sessions
- You want window/session management within a single terminal
- You prefer an established tool with extensive community support and plugins

**Choose screen when:**
- You need the most compatible terminal multiplexer (available on virtually every Unix system)
- You want simple session detachment

**Choose mprocs when:**
- You want a simple, Go-based multi-process runner for local development
- You don't need web API or remote access

**Choose gotty when:**
- You want to share a single terminal in a browser
- You prefer Go tooling and simple deployment

**Choose wetty when:**
- You want a full terminal emulator in the browser (xterm.js-based)
- You need SSH-based authentication
- You already have a Node.js infrastructure

## Key Architectural Differences

| Aspect | vrw | tmux/screen |
|--------|---------|-------------|
| **Communication** | HTTP/WebSocket | Unix sockets |
| **State** | RESTful (stateless handlers) | In-process (shared memory) |
| **Extensibility** | Add API endpoints + handlers | Write C extensions |
| **Deployment** | Single binary, zero config | Install + configure |
| **Scaling** | Multiple instances | Single instance (multiple sessions) |
| **Remote Access** | Built-in TLS + auth | SSH + tmux proxy |
