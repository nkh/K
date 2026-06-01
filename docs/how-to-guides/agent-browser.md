# Agent Browser — Automated Screenshots and UI Testing

This document describes how we use **agent-browser** to automate screenshot capture and UI interaction for the vrw web dashboard. agent-browser is a headless Chromium CLI built by Vercel that lets you programmatically control a browser from the command line.

## What is agent-browser?

agent-browser is a headless browser automation tool maintained by Vercel Labs (https://github.com/vercel-labs/agent-browser). It wraps Playwright under the hood but exposes a simpler CLI interface designed for AI agents and automation scripts. It maintains a persistent browser session across commands, so every invocation operates on the same page state until you navigate away or close the session.

**Key properties:**

- **Stateful** — the browser session persists between commands. Clicks, form fills, and navigation all happen on the same page context.
- **Ref-based interaction** — `snapshot -i` returns an accessibility tree where each interactive element gets a stable ref (`@e1`, `@e2`, ...). You interact with elements by ref.
- **CLI-first** — no JavaScript or Python needed. Every operation is a shell command.
- **Linux/macOS** — primary platforms. On ARM64 Linux, you may need to use the full path to the binary.

## Installation

```bash
npm install -g agent-browser
agent-browser install
# If missing Chromium deps:
agent-browser install --with-deps
```

Verify:

```bash
agent-browser open https://example.com
# ✓ example.com
```

## Core Workflow

The fundamental loop is: **navigate → snapshot → interact → re-snapshot**.

```bash
# 1. Open a page
agent-browser open http://127.0.0.1:8765/

# 2. Snapshot the DOM to discover interactive elements
agent-browser snapshot -i

# 3. Interact using the refs from the snapshot
agent-browser click @e3
agent-browser fill @e5 "hello world"
agent-browser press Enter

# 4. Re-snapshot after DOM changes to get fresh refs
agent-browser snapshot -i
```

Refs are stable per page load but change after navigation or significant DOM mutations. Always re-snapshot after a page navigation or when elements appear/disappear.

## Screenshots

```bash
# Full viewport screenshot to stdout (base64)
agent-browser screenshot

# Save to file
agent-browser screenshot output.png

# Full-page screenshot (scrolls and captures everything)
agent-browser screenshot --full output.png
```

### Screenshots of specific elements

agent-browser does not have a built-in "screenshot this element only" command — the `screenshot` command always captures the full viewport. To capture a specific element, use one of these approaches:

**Approach 1 — Scope the snapshot tree (doesn't crop the image):**

```bash
agent-browser snapshot -s "#my-element"
```

This filters the accessibility tree output to only that subtree, but the screenshot still captures the full viewport.

**Approach 2 — Get bounding box + crop with ImageMagick:**

```bash
# Get the element's coordinates
agent-browser get box @e5
# Output: { x: 100, y: 200, width: 300, height: 150 }

# Take full viewport screenshot, then crop
agent-browser screenshot full.png
convert full.png -crop 300x150+100+200 element.png
```

**Approach 3 — Use JavaScript evaluation:**

```bash
agent-browser eval "
  const el = document.querySelector('#my-element');
  const rect = el.getBoundingClientRect();
  JSON.stringify({x: rect.x, y: rect.y, w: rect.width, h: rect.height});
"
```

Then crop as shown above.

## Complete Command Reference

### Navigation

| Command | Description |
|---------|-------------|
| `agent-browser open <url>` | Navigate to URL |
| `agent-browser back` | Go back |
| `agent-browser forward` | Go forward |
| `agent-browser reload` | Reload page |
| `agent-browser close` | Close browser |

### Snapshot (DOM discovery)

| Command | Description |
|---------|-------------|
| `agent-browser snapshot` | Full accessibility tree |
| `agent-browser snapshot -i` | Interactive elements only (recommended) |
| `agent-browser snapshot -c` | Compact output |
| `agent-browser snapshot -d 3` | Limit depth to 3 |
| `agent-browser snapshot -s "#main"` | Scope to CSS selector |

### Interaction (using refs from snapshot)

| Command | Description |
|---------|-------------|
| `agent-browser click @e1` | Click element |
| `agent-browser dblclick @e1` | Double-click |
| `agent-browser fill @e2 "text"` | Clear input and type |
| `agent-browser type @e2 "text"` | Type without clearing |
| `agent-browser press Enter` | Press a key |
| `agent-browser press Control+a` | Key combination |
| `agent-browser hover @e1` | Hover over element |
| `agent-browser check @e1` | Check checkbox |
| `agent-browser uncheck @e1` | Uncheck checkbox |
| `agent-browser select @e1 "value"` | Select dropdown option |
| `agent-browser scroll down 500` | Scroll by pixels |
| `agent-browser rightclick @e1` | Right-click element |
| `agent-browser focus @e1` | Focus element |

### Information

| Command | Description |
|---------|-------------|
| `agent-browser get text @e1` | Get element text |
| `agent-browser get html @e1` | Get innerHTML |
| `agent-browser get value @e1` | Get input value |
| `agent-browser get attr @e1 href` | Get attribute |
| `agent-browser get title` | Page title |
| `agent-browser get url` | Current URL |
| `agent-browser get count ".item"` | Count matching elements |
| `agent-browser get box @e1` | Bounding box (x, y, width, height) |

### Wait

| Command | Description |
|---------|-------------|
| `agent-browser wait @e1` | Wait for element to appear |
| `agent-browser wait 2000` | Wait milliseconds |
| `agent-browser wait --text "Success"` | Wait for text to appear |
| `agent-browser wait --url "/dashboard"` | Wait for URL pattern |
| `agent-browser wait --load networkidle` | Wait for network idle |

### Screenshots and Recording

| Command | Description |
|---------|-------------|
| `agent-browser screenshot` | Screenshot to stdout |
| `agent-browser screenshot path.png` | Save to file |
| `agent-browser screenshot --full` | Full page scroll |
| `agent-browser pdf output.pdf` | Save as PDF |
| `agent-browser record start demo.webm` | Start video recording |
| `agent-browser record stop` | Stop and save video |

### Browser Settings

| Command | Description |
|---------|-------------|
| `agent-browser set viewport 1920 1080` | Set viewport size |
| `agent-browser set device "iPhone 14"` | Emulate device |
| `agent-browser set media dark` | Emulate dark color scheme |
| `agent-browser set headers '{"X-Key":"v"}'` | Extra HTTP headers |
| `agent-browser set credentials user pass` | HTTP basic auth |

### Tabs and Sessions

| Command | Description |
|---------|-------------|
| `agent-browser tab new <url>` | New tab |
| `agent-browser tab 2` | Switch to tab |
| `agent-browser tab close` | Close tab |
| `agent-browser --session test1 open ...` | Isolated named session |

### JavaScript and State

| Command | Description |
|---------|-------------|
| `agent-browser eval "document.title"` | Run JavaScript |
| `agent-browser state save auth.json` | Save session state |
| `agent-browser state load auth.json` | Load saved state |

## Real-World Example: Capturing All vrw Web UI Screenshots

Below is the exact sequence used to generate all 26 screenshots for the vrw web dashboard documentation. This serves as a practical reference for how to combine snapshot, interaction, and screenshot commands.

### Setup: Start vrw and spawn commands

First, start a vrw daemon with several commands running so the UI has content to display:

```bash
# Start vrw in daemon mode
vrw --daemon --port 8765 -- bash -c "while true; do echo idle; sleep 60; done"

# Spawn commands
vrw --target 7775 spawn -- bash -c "while true; do echo '=== bash logger ==='; date; uptime; sleep 2; done"
vrw --target 7775 spawn -- watch -n 1 'uptime; free -h; df -h /'
vrw --target 7775 spawn -- bash -c "for i in \$(seq 1 100); do echo \"line \$i: \$(date)\"; sleep 1; done"
vrw --target 7775 spawn -- bash -c "while true; do echo '=== sysinfo ==='; uname -a; uptime; free -h; echo; sleep 3; done"
vrw --target 7775 spawn -- bash -c "while true; do echo '=== network ==='; ss -tlnp 2>/dev/null; echo; sleep 3; done"
```

### Open the browser and take the first snapshot

```bash
agent-browser open http://127.0.0.1:8765/
agent-browser set viewport 1920 1080
agent-browser wait --load networkidle
```

### Discovery snapshot — getting the ref map

```bash
agent-browser snapshot -i
```

This returns the full interactive element map. Here is the output we got, annotated with what each element is:

```
- button "☰" [ref=e1]                          ← sidebar toggle (collapse/expand)
- button "◀" [ref=e2]                          ← previous panel tab
- button "▶" [ref=e3]                          ← next panel tab
- button "+ Panel" [ref=e4]                     ← add a new panel / instance
- button "🔍" [ref=e5]                          ← global search toggle
- combobox "Theme" [ref=e6]: Auto               ← theme switcher (Auto/Dark/Light/Grey)
- button "🔔" [ref=e7]                          ← notifications
- button "Logs" [ref=e8]                        ← toggle log viewer overlay
- button "Status" [ref=e9]                       ← toggle status bar
- textbox "Token:" [ref=e10]                    ← auth token input
- button "Set" [ref=e11]                        ← submit auth token
- button "Docs" [ref=e12]                        ← external docs link
- button "?" [ref=e13]                           ← keyboard shortcuts modal
- generic "Commands" [ref=e14]                   ← sidebar: Commands tab
- generic "Spawn" [ref=e15]                      ← sidebar: Spawn tab
- generic "Templates" [ref=e16]                  ← sidebar: Templates tab
- generic "Certs" [ref=e17]                       ← sidebar: Certificates tab
- textbox "Filter..." [ref=e23]                   ← command list filter input
- button "Kill All" [ref=e24]                    ← kill all running commands
- button "Command bash" [ref=e25]                 ← sidebar: first command entry
  - button "✕" [ref=e41]                         ←   close/kill this command
  - button "☆" [ref=e42]                         ←   pin command to top
- button "Command bash" [ref=e26]                 ← sidebar: second command entry
  - button "✕" [ref=e43]
  - button "☆" [ref=e44]
- button "Command bash" [ref=e27]                 ← sidebar: third command entry
  - button "✕" [ref=e45]
  - button "☆" [ref=e46]
- button "Command watch" [ref=e28]               ← sidebar: watch command entry
  - button "✕" [ref=e47]
  - button "☆" [ref=e48]
- button "Panel options for Local" [ref=e22]     ← panel header controls area
  - button "↻" [ref=e29]                         ←   refresh terminal output
  - button "⚙" [ref=e30]                         ←   settings
  - button "A-" [ref=e31]                         ←   decrease font size
  - button "A+" [ref=e32]                         ←   increase font size
  - spinbutton "Terminal rows" [ref=e49]: 24     ←   rows resize input
  - spinbutton "Terminal columns" [ref=e50]: 80   ←   columns resize input
  - button "Resize" [ref=e51]                     ←   apply resize
  - combobox "Buffer" [ref=e33]: Current          ←   main/alt terminal buffer
  - textbox "Send keys..." [ref=e34]               ←   send keystrokes input
  - button "Send" [ref=e35]                        ←   submit keys
  - button "?" [ref=e36]                           ←   special keys help popup
  - button "Copy" [ref=e37]                        ←   copy terminal content
  - button "⤓" [ref=e38]                           ←   download terminal content
  - button "📷" [ref=e39]                           ←   capture terminal screenshot
  - button "◐" [ref=e40]                           ←   fullscreen toggle
```

### Capturing each screenshot

**01 — Overview (default view after load):**

```bash
agent-browser screenshot 01-overview.png
```

No interaction needed. The default view after loading shows the sidebar with commands and the terminal area.

**02 — Top bar:**

```bash
agent-browser screenshot 02-topbar.png
```

The top bar is always visible. This captures it along with the rest of the UI, which is fine since the doc crops or references it.

**03 — Sidebar: Commands tab:**

```bash
agent-browser click @e14             # click "Commands" tab in sidebar
agent-browser wait 500
agent-browser screenshot 03-sidebar-commands.png
```

**04 — Sidebar: Spawn tab:**

```bash
agent-browser click @e15             # click "Spawn" tab
agent-browser wait 500
agent-browser screenshot 04-sidebar-spawn.png
```

**05 — Sidebar: Templates tab:**

```bash
agent-browser click @e16             # click "Templates" tab
agent-browser wait 500
agent-browser screenshot 05-sidebar-templates.png
```

**06 — Sidebar: Certificates tab:**

```bash
agent-browser click @e17             # click "Certs" tab
agent-browser wait 500
agent-browser screenshot 06-sidebar-certs.png
```

**07 — Panel header controls:**

```bash
agent-browser click @e14             # back to Commands tab
agent-browser wait 500
agent-browser click @e22             # click "Panel options for Local"
agent-browser wait 500
agent-browser screenshot 07-panel-header.png
```

**08 — Terminal view (showing a running command):**

```bash
agent-browser click @e25             # click first "Command bash" to select it
agent-browser wait 1500              # wait for terminal output to render via WebSocket
agent-browser screenshot 08-terminal-view.png
```

**09 — Send keys area (with active terminal):**

```bash
agent-browser click @e28             # click "Command watch" to show a different terminal
agent-browser wait 1500
agent-browser screenshot 09-send-keys.png
```

**10 — Bottom bar / Status:**

```bash
agent-browser click @e9              # toggle "Status" bar open
agent-browser wait 500
agent-browser screenshot 10-bottombar.png
agent-browser click @e9              # toggle closed
agent-browser wait 500
```

**11 — Log viewer:**

```bash
agent-browser click @e8              # toggle "Logs" overlay open
agent-browser wait 1000
agent-browser screenshot 11-log-viewer.png
agent-browser click @e8              # toggle closed
agent-browser wait 500
```

**12 — Keyboard shortcuts modal:**

```bash
agent-browser click @e13             # click "?" button — opens shortcuts modal
agent-browser wait 1000
agent-browser screenshot 12-keyboard-shortcuts.png
agent-browser press Escape           # close modal
agent-browser wait 500
```

**13 — Special keys help popup:**

```bash
agent-browser click @e36             # click "?" next to "Send keys..." input
agent-browser wait 1000
agent-browser screenshot 13-special-keys-help.png
agent-browser press Escape
agent-browser wait 500
```

**14 — Global search:**

```bash
agent-browser click @e5              # click "🔍" search toggle
agent-browser wait 1000
agent-browser screenshot 14-global-search.png
agent-browser press Escape
agent-browser wait 500
```

**15 — Dark theme:**

```bash
agent-browser select @e6 "Dark"    # change theme combobox to "Dark"
agent-browser wait 1500              # wait for CSS transition to complete
agent-browser screenshot 15-theme-dark-overview.png
```

**16 — Light theme:**

```bash
agent-browser select @e6 "Light"
agent-browser wait 1500
agent-browser screenshot 16-theme-light-overview.png
```

**24 — Grey theme:**

```bash
agent-browser select @e6 "Grey"
agent-browser wait 1500
agent-browser screenshot 24-grey-theme-overview.png

# Restore to auto
agent-browser select @e6 "Auto"
agent-browser wait 500
```

**17 — Exited command (sidebar):**

```bash
# First, kill a command via the UI to trigger exited state
agent-browser click @e46             # click "✕" on "Command watch" to kill it
agent-browser wait 1500              # wait for WebSocket update
agent-browser screenshot 17-exited-command-sidebar.png
```

**18 — Exited command (terminal banner):**

```bash
agent-browser click @e31             # click the now-exited "watch" entry
agent-browser wait 1000
agent-browser screenshot 18-exited-banner.png
```

**19 — Welcome / empty panel:**

```bash
agent-browser click @e4              # click "+ Panel" — adds a new empty panel
agent-browser wait 1000
agent-browser click @e2              # navigate to the new empty panel with "◀"
agent-browser wait 500
agent-browser screenshot 19-welcome-panel.png
```

**20 — Add panel modal:**

```bash
agent-browser screenshot 20-add-panel-modal.png    # the add-instance-panel form
```

**21 — Context menu:**

```bash
# Context menus are triggered by right-click, but may not always render in headless.
# Fallback: dispatch via JavaScript
agent-browser eval "
  document.querySelectorAll('button').forEach(btn => {
    if (btn.textContent.includes('watch') || btn.textContent.includes('bash')) {
      btn.dispatchEvent(new MouseEvent('contextmenu', {bubbles: true, cancelable: true}));
    }
  });
"
agent-browser wait 500
agent-browser screenshot 21-context-menu.png
```

**22 — Collapsed sidebar:**

```bash
agent-browser click @e1              # hamburger toggle — collapse sidebar
agent-browser wait 500
agent-browser screenshot 22-collapsed-sidebar.png
agent-browser click @e1              # restore sidebar
agent-browser wait 500
```

**23 — Resize controls:**

```bash
agent-browser click @e22             # open panel options (shows resize inputs)
agent-browser wait 500
agent-browser screenshot 23-resize-controls.png
```

**25 — Numbered overview:**

```bash
agent-browser click @e30             # select a command for context
agent-browser wait 500
agent-browser screenshot 25-overview-numbered.png
```

### Hero screenshot (vrw-5-commands.png) with dark theme and 5 running commands

```bash
# Switch to dark theme for maximum visual impact
agent-browser select @e6 "Dark"
agent-browser wait 1500
agent-browser screenshot vrw-5-commands.png
```

### Cleanup

```bash
agent-browser close
pkill -f "vrw"    # stop the daemon
```

## Tips and Gotchas

1. **Always re-snapshot after navigation.** Refs change when the page reloads or when significant DOM changes occur (modals opening, elements appearing/disappearing). If a click fails with "Unknown ref", take a fresh snapshot.

2. **Wait for transitions.** The vrw UI uses CSS transitions for theme changes and WebSocket-driven updates for terminal content. Use `wait 1500` after theme switches and `wait 1000-1500` after selecting commands.

3. **Refs are scoped per snapshot.** The `@e1` from one snapshot is not necessarily the same element as `@e1` from a later snapshot. Never hardcode refs across multiple snapshot cycles.

4. **Viewport size matters.** Screenshots capture exactly the viewport. Use `set viewport 1920 1080` at the start for consistent, high-resolution captures.

5. **Headless vs headed.** Use `--headed` flag when debugging to see the actual browser window. Without it, everything runs in headless mode (no display needed).

6. **Session isolation.** Use `--session name` to run multiple independent browser instances in parallel. Each session has its own cookies, storage, and page state.

7. **Video recording.** The `record start/stop` commands capture WebM video. Start recording, perform a sequence of actions, then stop — useful for creating demos or debugging CI failures.

8. **State persistence.** `state save/load` serializes cookies and localStorage to JSON. Useful for reusing a login session across separate agent-browser invocations without repeating authentication.

9. **Error handling.** If a command times out, the CLI returns a non-zero exit code. Use the `--timeout` flag to increase the default timeout for slow pages.

10. **Network control.** `network route` can intercept, block, or mock HTTP requests. Useful for testing error states or offline behavior in the UI.
