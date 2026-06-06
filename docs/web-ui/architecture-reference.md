# vrw Web UI Architecture Reference & Improvement Proposals

> Panel Architecture | WebSocket Management | Multi-Instance
>
> June 2026

---

## 1. UI Architecture Overview

The vrw Web UI is a single-page application that runs entirely in the browser, served as a static HTML page (`index.html`) with an inline JavaScript application (`app.js`, approximately 6336 lines) and a CSS stylesheet (`style.css`). The UI follows a panel-based architecture where terminal displays are independent containers that can each connect to any command on any server instance through dedicated WebSocket connections. The design philosophy emphasizes decoupling: panels are pure display units with no mandatory ties to server connections, and server connections are managed independently as reusable resources available to all panels.

The application state is centralized in a global state object that tracks all panels, connections, WebSocket instances, update modes, theme preferences, and cached terminal data. Per-panel state (font size, theme, selection mode, WebSocket, poll timer) is stored on individual panel objects within the `state.panels` array. Persistent preferences (font size, theme, panel layout, tokens) are saved to `localStorage`. The UI supports multiple server instances simultaneously, with commands from all servers appearing in a unified sidebar list that can be filtered by server or viewed alphabetically.

The UI is organized into 13 distinct functional areas, each with specific responsibilities and well-defined interaction patterns. The following sections describe each area in detail, listing its elements, purpose, and how it interacts with other areas.

---

## 2. Top Bar

The top bar is the primary navigation and global control strip that spans the full width of the application window. It provides access to every major UI function without requiring the sidebar, making it the single most important element for keyboard-driven workflows. The bar is divided into two logical groups: a left group containing navigation and panel controls, and a right group containing global settings, authentication, and informational toggles.

| Element | Description |
|---------|-------------|
| Hamburger Button | Toggles the sidebar between expanded and collapsed states. On screens narrower than 768 pixels, the sidebar auto-collapses on load. |
| Prev/Next Command | Navigates through commands on the same server as the focused panel, in chronological (spawn) order. Wraps around at boundaries. Useful when the sidebar is hidden. If the panel has no server connection, falls back to all commands in spawn order. |
| + Panel Button | Creates a new empty panel. Panels are decoupled from server connections; the user can later connect a command from the sidebar to any panel. |
| Global Search | Opens a search overlay that searches across all command output on all connected servers. When opened, all panel VTTY updates are paused (WS and poll) so text does not change while searching. An optional "Freeze cmds" checkbox sends SIGSTOP to all running commands. Clicking a result loads that command into the focused panel, which remains frozen while other panels thaw. A "VTTY frozen" indicator appears on the frozen panel; clicking it resumes updates. |
| Theme Toggle | Cycles the global theme through Auto (OS preference), Grey, and Dark. Stored in localStorage. |
| Sound Toggle | Enables or disables browser notification sounds when commands exit. |
| Logs Button | Toggles between the terminal view and the log streaming view. The log viewer has its own WebSocket for real-time streaming. |
| Status Button | Toggles the bottom status bar visibility. When active, the button is highlighted. |
| Token Input | Accepts a Bearer authentication token. Saved to localStorage and applied to all API and WebSocket requests. |
| Docs Button | Opens an embedded documentation viewer (rendered in the content area). |
| Shortcuts Button | Displays a modal overlay listing all keyboard shortcuts. |

**Interactions:** The top bar interacts with the Sidebar (toggling its visibility), Panel Container (adding panels, navigating commands), Log Viewer (switching views), Bottom Bar (toggling visibility), and Global Search Overlay (opening/closing). Theme changes propagate to all panels and the shared toolbar. The token input feeds into the central auth state used by every API call and WebSocket connection in the application.

---

## 3. Sidebar

The sidebar is the primary context panel for server management, command browsing, and command spawning. It is divided into a tab bar with five tabs (Servers, Spawn, Templates, Envs, Certs) and a content area that swaps based on the active tab. The sidebar is resizable via a drag handle on its right edge and can be collapsed to zero width. When multiple server instances are connected, the sidebar provides a sort bar to group commands by server or view them in a flat alphabetical list.

| Element | Description |
|---------|-------------|
| Tab Bar (5 tabs) | Servers (command list), Spawn (new command form), Templates (saved command templates), Envs (environment presets), Certs (certificate list). Envs and Certs are kept in the sidebar because they serve the spawn workflow — environments activate preconfigured panel sets, and certificates are selected during spawning. Tabs are shown/hidden based on server reachability. |
| Command Filter | Real-time text filter for the command list. Filters by command name, arguments, and PID. Also scopes the Kill All button — when a filter is active, Kill All only affects matching commands. |
| + Server Button | Opens the Add Server modal to register a new remote instance connection. The modal includes an "Open pane connected to this server" checkbox (checked by default) that auto-creates a panel showing the server's main command (spawn_order 0) or first spawned command. |
| Kill All Button | Kills running commands across connected servers, **respecting the current filter**. When the filter is empty, kills all running commands (original behavior). When a filter is active, only kills commands matching name/args/PID. Only visible when at least one server is reachable and has running commands. |
| Command List | Displays commands grouped by server instance. Each item shows: kill button, keep/unkeep button (star), pin button (target circle), grab handle (for reorder), command name, exit badge, runtime, resource badges, and PID. |
| Sort Bar | When multiple instances are connected, shows tabs: All (alphabetical), plus one tab per instance label. Clicking an instance tab filters to that server's commands. |
| Instance Headers | Visual separators in the command list showing the server label, URL, and reachable/disconnected status. Each header includes a close button (`✕`) to disconnect from that server. |
| Disconnected Banner | Warning banner inserted at the top of the sidebar when one or more servers become unreachable. |
| Command Reorder | Commands can be reordered within each server group by dragging the grab handle (`⠇`). Custom order is persisted in localStorage (`vrw_cmd_order`). |

**Interactions:** The sidebar is the primary input surface for command selection. Clicking a command item calls `selectCommand()`, which routes the command's VTTY output to the currently focused panel and starts a per-panel WebSocket connection. The Kill All button sends kill requests scoped by the active filter to all connected servers in parallel. The Spawn tab interacts with the Panel Container through the instance dropdown, which has been fixed via `_userSpawnInstUrl` to avoid resetting during sidebar rebuilds. The Templates tab stores command templates in localStorage and provides one-click spawning. The Envs tab allows activating preconfigured workspace environments. Server headers include close buttons that call `disconnectServer()`, which removes the connection and disconnects any panels viewing commands from that server (panels retain their last VTTY state).

---

## 4. Spawn Form (Sidebar Tab)

The Spawn form allows users to create new command processes on any connected server instance. It is fully decoupled from the focused panel — the Target Instance dropdown retains its selection across sidebar rebuilds and panel focus changes. The form provides fields for the target server, command path, arguments, per-command environment variables, working directory, terminal dimensions, optional TLS certificate selection, retain-on-exit toggle, and an "Open in new panel" option. The form supports Enter-key submission from input fields and Ctrl+Enter from the environment variables textarea.

| Element | Description |
|---------|-------------|
| Target Instance Select | Dropdown of all connected servers. Decoupled from the focused panel — selection persists across sidebar rebuilds and panel focus changes. Defaults to the first connection. |
| Command Input | The executable path (e.g., `/usr/bin/htop`). Required field. |
| Arguments Input | Space-separated arguments. Supports quoted strings with double and single quotes, and backslash escapes. |
| Environment Variables | Multi-line textarea for per-command `KEY=VALUE` pairs. Empty lines and `#` comments are ignored. Values may contain `=` signs (split on first `=` only). Merged on top of config-level env vars. |
| Working Directory | Optional directory to set as the command's cwd. |
| Rows/Cols Inputs | Terminal dimensions. Blank uses server defaults. |
| Auto-fit Button | Calculates optimal rows and cols from the focused panel's VTTY container size and current font size. |
| Certificate Select | Dropdown populated from all connected servers' certificate lists. |
| Retain on Exit | Checkbox to keep the terminal buffer after the command exits. |
| Open in New Panel | Checkbox (default: checked). When enabled, spawn creates a new panel for the command instead of taking over the focused panel. |
| Spawn Button | Submits the form via POST to the selected instance's `/api/commands` endpoint. |

**Interactions:** On successful spawn, if "Open in new panel" is checked (default), a new panel is created and focused for the spawned command. If unchecked, the traditional behavior applies: the old panel WebSocket is disconnected, the terminal cache is cleared, and the new command's VTTY is loaded in the focused panel. Both paths trigger a `loadCommands()` call which rebuilds the sidebar command list. The Target Instance dropdown is populated by `updateInstanceDropdown()`, which runs after every sidebar rebuild. The spawn instance selection is fully decoupled from the focused panel via `_userSpawnInstUrl`.

---

## 5. Templates Tab (Sidebar)

The Templates tab provides a mechanism for saving, organizing, and reusing command configurations. Users can save frequently-used command/argument combinations as named templates stored in localStorage. This avoids re-typing complex commands and provides a quick-launch workflow for common development tasks. Templates can be spawned directly from the template list with a single click.

| Element | Description |
|---------|-------------|
| Template List | Displays saved templates with name, command, and arguments. Each template has Edit and Delete buttons. |
| Add Template Form | Inline form with fields for template name, command, and arguments. Hidden by default, toggled by the + Add button. |
| Spawn from Template | Each template item has a Spawn button that pre-fills the spawn form and immediately executes the command. |

**Interactions:** Templates are stored entirely in localStorage (key: `vrw_templates`). The spawn-from-template flow pre-fills the Spawn form fields and calls `spawnCommand()`. Templates do not persist across different browser profiles or devices. There is no server-side template storage or sharing mechanism.

---

## 6. Environments Tab (Sidebar)

The Environments tab provides a mechanism for activating preconfigured workspace environments. Each environment defines a named set of panels, server connections, and commands to spawn. Environments are loaded from the server's configuration file (`[[environments]]` sections in TOML config) and exposed via the `/api/environments` API endpoint. Selecting an environment from the list triggers `activateEnvironment()`, which sets up all defined panels, connects to specified servers, and spawns the configured commands.

| Element | Description |
|---------|-------------|
| Environment List | Displays named workspace environments. Each entry shows the environment name and a summary of its configuration (panel count, server count, command count). |
| Activate Button | Clicking an environment entry activates it, creating all defined panels, connecting servers, and spawning commands. |

**Interactions:** Environment data is fetched from the server's `/api/environments` endpoint during initialization and when the Envs tab is selected. The `activateEnvironment()` function iterates over the environment's definition: it creates the required number of panels, adds server connections (idempotent via `addConnection()`), and spawns commands on the specified instances. Multiple environments can be configured in the server's TOML config file, and each can be auto-started by the server on boot.

---

## 7. Certificates Tab (Sidebar)

The Certificates tab displays TLS certificates available across all connected server instances. Certificates are used when spawning commands that require mTLS authentication to remote services. Each certificate entry shows its name and a token preview. The certificate list is also used to populate the Spawn form's certificate dropdown.

| Element | Description |
|---------|-------------|
| Certificate List | Grouped by server instance label. Each entry shows the certificate name and a truncated token preview. |
| Per-Server Sections | Each connected server has a labeled section with its own list of certificates. |

**Interactions:** Certificates are fetched from each server's `/api/certificates` endpoint during initialization and whenever a new server is added. The `updateCertDropdown()` function synchronizes this data with the Spawn form's certificate dropdown. When a server becomes unreachable, its certificates remain visible from the last successful fetch but are marked as potentially stale.

---

## 8. Shared Toolbar

The shared toolbar is a horizontal control strip positioned above the panel container. It acts on whichever panel is currently focused, providing terminal resize controls, font size adjustment, buffer switching, refresh throttling, key sending, layout toggling, selection mode, and panel theme controls. The toolbar is hidden when the welcome screen is displayed and shown when at least one panel is visible.

| Element | Description |
|---------|-------------|
| Restart Button | Restarts the command currently displayed in the focused panel. Only visible when a command is selected. |
| Resources Toggle | Toggles visibility of CPU/memory resource badges in the sidebar command list and the toolbar. |
| Resource Badge | Shows CPU percentage and memory usage for the focused panel's command. |
| Instance URL | Shows the server URL for the focused panel's selected command. |
| Font Size Controls | A- / size label / A+ buttons that adjust the focused panel's font size (8-28px range). |
| Resize Controls | Rows/Cols number inputs and a Resize button. Changes are sent to the server via `/api/commands/:id/resize`. The inputs are automatically synced with the server's actual terminal dimensions as VTTY metadata is received. Manual edits are preserved and not overwritten by server sync. |
| Max Fit Button | Toggle that calculates the maximum terminal dimensions to fill the panel at the current font size, then restores on second click. State is tracked per-panel. Cannot resize exited commands (the server rejects the resize). After successful resize, the cell grid is invalidated and a fresh VTTY render is requested. |
| Max Font Button | Toggle that calculates the largest font size that still fits the current terminal dimensions in the panel, then restores on second click. State is tracked per-panel. On restore, state is cleaned up so re-activation works correctly. Skips activation if the computed font size equals the current size. |
| Buffer Select | Dropdown to switch between Current, Main, and Alt terminal buffers. |
| Refresh Throttle | Adjustable client-side throttle (0-2000ms in 100ms steps) that limits how often VTTY updates are applied to the DOM. |
| Send Keys | Text input and Send button for sending keystrokes to the focused panel's command. Supports special key sequences. |
| Layout Toggle | Switches panel arrangement between horizontal (side-by-side) and vertical (stacked). Only visible with 2+ panels. |
| Selection Mode | Toggles text selection mode on the focused panel. When active, mouse events are not forwarded to the PTY. |
| Copy Button | Copies the selected terminal text to clipboard. |
| Export Button | Exports the focused panel's terminal content as a text file. |
| Screenshot Button | Downloads a PNG screenshot of the focused panel's terminal. |
| Panel Theme Button | Cycles the focused panel's VTTY area theme through Inherit, Light, and Dark. Independent of the global theme. |

**Interactions:** The shared toolbar reads and writes state on the focused panel object. Every button action targets `state.panels[activePanelId]` rather than global state. The toolbar is synchronized by `updateSharedToolbar()`, which is called whenever focus changes, command selection changes, or font/theme changes. The Max Fit and Max Font toggles maintain per-panel state in the `_maxFitState` and `_maxFontState` dictionaries, keyed by panel ID, ensuring that toggling on one panel does not affect another.

---

## 9. Panel Container

The panel container is the flexbox parent that holds all terminal panels. Its `flex-direction` property determines whether panels are arranged horizontally (side-by-side, `flex-direction: row`) or vertically (stacked, `flex-direction: column`). The container hosts the panel rendering logic, including DOM caching for fast panel switches, terminal scroll detection, and drop-target initialization for command drag-and-drop.

| Element | Description |
|---------|-------------|
| Flex Direction | Controlled by `state.panelLayout`, persisted in localStorage. Toggled via the Layout button in the shared toolbar. |
| Panel Resize Handles | Draggable dividers between panels that allow resizing the proportion of space each panel occupies. |
| Welcome State | When no commands exist and no server is reachable, the container shows a welcome card with the vrw logo and a "not running" message. |

**Interactions:** The panel container is rebuilt by `renderPanels()` whenever the panel count or IDs change, or when transitioning between welcome and panel views. To avoid flickering, the function uses a fast-path check: if panel count and IDs are unchanged, it only updates the flex direction without rebuilding DOM. During rebuilds, terminal DOM content is cached into DocumentFragments and restored after the rebuild, preserving scroll positions. The container also initializes drop targets so commands from the sidebar can be dragged onto panels.

---

## 10. Individual Panel

Each panel is an independent terminal display unit with its own WebSocket connection, font size, theme, selection mode, and command selection state. Panels are completely decoupled from server connections: a panel can exist without any connected server, displaying a placeholder message until the user selects a command. When a command is selected, the panel establishes its own WebSocket to stream VTTY updates, maintains its own DOM cache, and tracks its own scroll position independently of other panels.

| Element | Description |
|---------|-------------|
| Panel Header | Contains: drag handle (when multiple panels), command full name, command arguments, panel label, and a close button (when multiple panels). Right-click opens the panel context menu. |
| Command Info | The command's full path and arguments, updated by `updatePanelCommandInfo()` when selection changes. |
| VTTY Container | The main terminal display area. Contains: `<pre>` element for VTTY HTML output, search bar, exited banner, cursor indicator, copy feedback, and scroll-to-bottom button. |
| Search Bar | In-terminal search with match count display, next/prev navigation, and close button. Toggled by Ctrl+F. |
| Exited Banner | Shown when a command has exited. Displays exit code with color-coded badge (green for 0, red for non-zero). |
| Scroll-to-Bottom Button | Floating button that appears when the user scrolls up from the bottom of the terminal output. |
| Disconnected Overlay | Semi-transparent overlay shown when the panel's selected command belongs to an unreachable server instance. |
| Alt Screen Badge | Small indicator when the terminal is in alternate screen mode (e.g., running htop). |

**Interactions:** Panels interact with nearly every other UI area. Clicking a panel focuses it, which updates the shared toolbar, syncs global state, and determines which panel receives commands from sidebar clicks. Panel WebSocket connections are managed independently: `connectPanelWs(panelId)` opens a dedicated WebSocket, and `disconnectPanelWs(panelId)` closes it. The per-panel WebSocket handles `vtty_full`, `vtty_diff`, `vtty_dirty`, `command_ended`, and `pong` messages, routing all VTTY updates to that panel's DOM elements. Commands can be dragged from the sidebar onto a panel to switch the panel's displayed command.

---

## 11. Bottom Bar (Status Bar)

The bottom bar is a hidden-by-default status strip that displays contextual information about the focused panel's terminal session. It shows the selected command's name, arguments, PID, cursor position, terminal dimensions, scrollback indicator, update mode, poll interval, refresh throttle, WebSocket latency, and connection status. The bar is toggled by the Status button in the top bar.

| Element | Description |
|---------|-------------|
| Command Label | Shows the focused command's full name, arguments, and PID in a compact format. |
| Cursor Position | Displays the terminal cursor row/column coordinates. |
| Terminal Dimensions | Shows the current rows x cols dimensions. |
| Scrollback Indicator | Yellow "SCROLLBACK" warning when the user is viewing historical scrollback rather than live output. |
| Update Mode Select | Switches between Push (WebSocket) and Poll (HTTP interval) update modes. |
| Poll Interval Input | Configurable poll interval in milliseconds (50-5000ms range). Only visible in Poll mode. |
| Refresh Throttle Input | Client-side DOM update throttle in milliseconds (0-2000ms, 100ms steps). |
| WS Quality Indicator | Shows WebSocket round-trip latency with color coding: green (<50ms), yellow (<200ms), red (>200ms). |
| Connection Status | Textual status: "Connected", "Connected (XXms)", "WS Disconnected", "WS Error", "Command ended". |

**Interactions:** The bottom bar is updated by multiple subsystems: `updateBottomBarLabel()` on command selection, `updateVttyMetadataFromHttp()` on VTTY fetches (cursor position, dimensions, scrollback), `updateWsQualityIndicator()` on ping/pong responses, and `updateDisconnectedUI()` on server reachability changes. The WS quality indicator reads from the focused panel's `wsLatency` field. The update mode select triggers `startUpdateMode()` or `stopUpdateMode()` which respectively starts or stops the WebSocket/poll cycle for the focused panel.

---

## 12. Log Viewer

The log viewer is an alternative view to the terminal display that shows the server's application logs in a scrollable text area. It supports real-time log streaming via a dedicated WebSocket connection, full-text search, and manual refresh. The log viewer shares the content area with the terminal panels and the docs viewer, with only one visible at a time.

| Element | Description |
|---------|-------------|
| Log Toolbar | Contains: search input, clear search button, refresh button, transport indicator (HTTP/WS), and log count. |
| Log Content | A scrollable `<div>` that displays log entries. Updated via HTTP fetch on initial load and WebSocket streaming thereafter. |
| Transport Indicator | Shows whether log data is being received via HTTP (initial load) or WebSocket (streaming). |

**Interactions:** The log viewer has its own WebSocket (`logWs`) separate from the panel VTTY WebSockets. When the user switches to the log view, `connectLogWs()` opens a WebSocket to `/api/logs/ws`. When switching away, `disconnectLogWs()` closes it. The search input triggers `searchLogs()` which sends a query to `/api/logs/search`. VTTY updates that arrive while the log viewer is active are buffered in `_pendingVttyData` and applied when the user returns to the terminal view.

---

## 13. Global Search Overlay

The global search overlay is a modal dialog that allows searching across all command output on all connected servers simultaneously. When the overlay opens, all panel VTTY updates (WebSocket push and HTTP poll) are paused to prevent terminal content from shifting while the user reads search results. An optional "Freeze cmds" checkbox sends SIGSTOP to all running commands, halting their execution entirely.

| Element | Description |
|---------|-------------|
| Search Input | Full-text query input. Supports Enter-key submission. |
| Freeze Commands Checkbox | Optional toggle that freezes (SIGSTOP) all running commands across all servers. Prevents any text changes in terminal buffers. Unchecking thaws them. |
| Search/Close Buttons | Execute the search or dismiss the overlay. Closing thaws all panels and commands. |
| Results Area | Scrollable container displaying search results grouped by command. Each result shows the command name, matching line, and line number. Clickable — clicking a result loads that command into the focused panel. |

**Freeze/thaw behavior:** When the search overlay opens, every panel's WebSocket connection is disconnected and poll timers are stopped, tracked in `_searchFrozenPanelIds`. When closed, all panels resume their update mode. If the user clicks a search result, the focused panel loads the selected command but remains frozen (its WS/poll stays stopped). All other panels thaw. A "VTTY frozen (click to unfreeze)" indicator bar appears at the bottom of the frozen panel; clicking it resumes updates for that panel.

**Interactions:** The overlay is opened by the search button in the top bar. Clicking outside the overlay or pressing Escape closes it and thaws all panels/commands. Search results are fetched from a single server endpoint; multi-server search requires the server to aggregate results from peers.

---

## 14. Context Menus

Context menus provide quick access to command and panel management actions via right-click. There are two distinct context menus: one for sidebar command items and one for panel headers. Both menus are dynamically created and positioned at the click location, then destroyed when closed. They use event delegation rather than inline onclick handlers to prevent XSS.

| Element | Description |
|---------|-------------|
| Command Context Menu | Items: View Terminal, Copy URL, Keep/Unkeep (alive commands), Pause/Resume, Restart, Kill (alive commands), Purge (exited commands). |
| Panel Context Menu | Items: Copy URL (command URL or instance URL), Pause/Resume (if command selected), Restart, Kill, Remove Panel (if 2+ panels). |

**Interactions:** Context menu actions directly call the corresponding API functions (`killCommand`, `togglePauseCmd`, `restartCommandById`, etc.) and then trigger `loadCommands()` to refresh the sidebar. The menu is positioned to avoid overflowing the viewport and is automatically dismissed on any click outside the menu, on Escape, or on scroll.

---

## 15. Modals and Overlays

The application uses several modal dialogs for user input that requires focused attention. Modals trap keyboard focus within their content (accessibility: Tab/Shift+Tab cycles through focusable elements), have a semi-transparent backdrop, and close on Escape or backdrop click. The existing modals are: Add Server & Panel, Add Server (connection only), and Keyboard Shortcuts help.

| Element | Description |
|---------|-------------|
| Add Server & Panel Modal | Fields: Server URL, Server Label (optional), Auth Token (optional), Split Direction (auto/horizontal/vertical). Creates a new connection and an empty panel pre-linked to that server. |
| Add Server Modal | Fields: Server URL, Label (optional), Auth Token (optional). Adds a connection without creating a panel. Commands appear in the sidebar for all panels. |
| Keyboard Shortcuts Modal | Displays a table of all keyboard shortcuts with descriptions. Non-interactive informational overlay. |
| Command Picker | Shown when a URL path matches multiple commands. Lists matching commands with running status, runtime, and PID. Click to select. |

**Interactions:** Modals use the `trapFocus()` mechanism to cycle Tab/Shift+Tab focus within the modal's focusable elements. On close, `releaseCurrentFocusTrap()` restores focus to the previously focused element. The Add Server & Panel modal calls `addConnection()` (idempotent) and `addPanelDirect()`, then sets the new panel's `selectedInstUrl` to the entered server URL. The Command Picker is triggered by URL routing when `/command-name` matches multiple running commands.

---

## 16. Cross-Area Interaction Map

The following table summarizes the key interaction pathways between UI areas. Each row describes a triggering action in one area and the resulting effects in other areas. Understanding these pathways is essential for diagnosing bugs that manifest as unexpected state changes across the UI.

| Trigger Area | Action | Affected Areas | Effect |
|-------------|--------|---------------|--------|
| Top Bar | Click + Panel | Panel Container | Creates empty panel, re-renders layout |
| Top Bar | Toggle Theme | All Panels, Shared Toolbar | Updates CSS variable, syncs toolbar |
| Top Bar | Click Logs | Log Viewer, Panel WS | Disconnects panel WS, starts log WS |
| Sidebar | Click command | Focused Panel, Shared Toolbar | Selects command, starts panel WS |
| Sidebar | Click Kill All | All Servers, All Panels | Sends kill to all, rebuilds sidebar |
| Sidebar | Click + Server | Connections, Command List | Adds connection, reloads commands |
| Spawn Form | Submit spawn | New Panel (or Focused Panel) | Spawns command, creates/focuses panel |
| Shared Toolbar | Click Max Fit | Focused Panel | Resizes terminal, updates button state |
| Shared Toolbar | Click Layout | Panel Container | Switches flex-direction, toggles layout |
| Panel | Click/focus | Shared Toolbar, Bottom Bar | Syncs toolbar/bottom bar to panel state |
| Panel | Right-click header | Context Menu | Shows panel context menu |
| Bottom Bar | Change Update Mode | Focused Panel WS | Switches between WS push and HTTP poll |
| Context Menu | Kill command | Sidebar, Panel | Kills command, clears panel if selected |
| Environments Tab | Activate environment | Panels, Connections, Commands | Creates panels, connects servers, spawns commands |

---

## 17. Twenty-Five UI Improvement Proposals

The following proposals address usability gaps, missing features, and user experience issues identified through analysis of the current Web UI implementation. Each proposal includes a description of the problem and a suggested approach. Improvements are numbered for reference but are not prioritized; implementation order should be driven by user impact and development effort.

### 1. Fix Spawn Instance Auto-Reset Bug (DONE)

The Target Instance dropdown in the Spawn form resets to the globally focused panel's server every time the sidebar rebuilds. This causes commands to be spawned on the wrong server. Fix: track the spawn instance selection independently from `state.selectedInstUrl` via `_userSpawnInstUrl`, and never overwrite it in `updateInstanceDropdown()`.

### 2. Panel Connection Indicator

Show a small colored dot or icon in each panel header indicating its WebSocket connection status (green = connected, yellow = reconnecting, red = disconnected). Currently, only the focused panel's status is visible in the bottom bar.

### 3. Drag-and-Drop Command Assignment

Allow users to drag commands from the sidebar onto specific panels. While the infrastructure for this partially exists (`initPanelDropTargets`, `onCmdDragStart`), the drop handler does not properly assign the command to the target panel's VTTY display.

### 4. Tab Completion in Spawn Form

Add tab completion to the Command input field in the Spawn form. Query the server's PATH directories or a configurable completions list and show a dropdown of matching executables.

### 5. Panel Split Presets

Add quick layout presets like "2x2 Grid", "1+2 Horizontal", and "1 Top + 2 Bottom" accessible from a dropdown in the shared toolbar. Currently only horizontal/vertical toggle is available.

### 6. Per-Panel Command History

Maintain a per-panel history of recently viewed commands. Add back/forward buttons in the panel header to navigate through the history, similar to a browser's navigation.

### 7. Command Output Search Across Panels

Extend the Global Search to highlight matches within individual panel terminals. When a search result is clicked, switch to the correct panel and scroll to the matching line.

### 8. Notification Center for Command Events

Replace the single browser notification with an in-app notification toast system. Show toasts for: command started, command exited, command killed, server disconnected, server reconnected. Stack notifications in a corner with auto-dismiss.

### 9. Customizable Panel Titles

Allow users to rename panels by double-clicking the panel header. The custom label would be saved in localStorage and displayed alongside the command name.

### 10. Command Grouping and Workspaces

Implement named workspaces that save and restore panel layouts, command selections, and server connections. Users can switch between workspaces (e.g., "Development", "Monitoring", "CI") with one click.

### 11. Spawn Form Command History

Add a dropdown or autocomplete to the Spawn form's Command field that shows recently used commands. Store the history in localStorage with a configurable max size (e.g., 20 entries).

### 12. Keyboard Shortcut Customization

Allow users to remap keyboard shortcuts through a settings interface. Store custom bindings in localStorage. Provide sensible defaults and an option to reset to defaults.

### 13. Responsive Multi-Panel Layout

On smaller screens, automatically switch to a tabbed panel interface instead of side-by-side panels. Each panel becomes a tab, and the user switches between them. This would make the UI usable on tablets and small laptop screens.

### 14. Server Health Dashboard

Add a small health indicator for each connected server in the sidebar header showing uptime, command count, and aggregate resource usage. Color-code based on health (green/yellow/red).

### 15. Terminal Copy-on-Select

Automatically copy selected terminal text to the clipboard when the mouse button is released, similar to terminal emulators like xterm. Currently, the user must click a separate Copy button.

### 16. Split Terminal within a Panel

Allow splitting a single panel into two sub-panels showing different buffers (e.g., Main and Alt, or two different commands) within the same panel container. This provides tmux-like functionality without requiring tmux.

### 17. Environment Configuration Presets (DONE)

Implement a mechanism to define named environment presets (config files) that specify a set of panels, their assigned server connections, and commands to spawn. Users can select a preset from the UI or pass it on the command line. Implemented via `[[environments]]` TOML configuration, `/api/environments` API endpoint, and the Envs sidebar tab.

### 18. Command Pinning with Auto-Restart

Extend the existing pin feature to optionally auto-restart pinned commands when they exit. This would enable a "watchdog" mode for long-running services where the user wants automatic recovery.

### 19. Timestamp and Scrollback Navigation

Add visible timestamps to terminal scrollback entries and a timestamp-based jump control in the bottom bar. This would help users navigate to specific points in a command's output history.

### 20. Unified Command Manager Dialog

Create a full-screen command management dialog accessible from the top bar that shows all commands across all servers in a sortable/filterable table with columns for name, PID, status, CPU, memory, runtime, server, and actions (kill, pause, restart, keep, purge).

### 21. Panel Minimize to Sidebar

Instead of closing a panel, allow minimizing it to a small icon in the sidebar or bottom bar. Clicking the icon restores the panel with its full state preserved (VTTY cache, scroll position, WebSocket).

### 22. Gradient Status Bar

Replace the plain bottom bar with a gradient background that subtly shifts color based on server health. Add a mini terminal preview showing the last few lines of output in a truncated format.

### 23. Onboarding Tutorial Overlay

For first-time users, show a step-by-step overlay tutorial that highlights UI elements and explains their purpose. The tutorial should be dismissible and not reappear after completion (tracked via localStorage).

### 24. Command Argument Builder

Replace the plain-text Arguments input in the Spawn form with a structured argument builder that supports key-value flags, file selection, and argument templates. Show a preview of the assembled command before spawning.

### 25. WebSocket Reconnection Visual Feedback

When a panel's WebSocket disconnects and enters the reconnection loop, show a pulsing animation on the panel border and a countdown timer showing when the next reconnection attempt will occur.

### 26. Multi-Cursor Terminal Search

Enhance the in-terminal search (Ctrl+F) to show all matches highlighted simultaneously with a match count navigator (1/15, 2/15, etc.). Support regex patterns and case-sensitive toggle.
