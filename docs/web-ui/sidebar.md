# Sidebar

The sidebar provides the primary navigation for managing commands. It contains four tabs: Servers, Templates, Certs, and Groups. The Certs tab remains in the sidebar (rather than the main toolbar) because it serves the command spawn workflow: certificates are selected during command spawning from the Spawn form.

The sidebar width can be adjusted by dragging the resize handle on its right edge (minimum 150px, maximum 600px). It can also be fully collapsed via the toggle button in the top bar.

### Sort Bar (multi-instance)
When multiple vrw instances are connected, a sort bar appears above the command list with options to sort commands: **All** (alphabetical across instances) or by individual instance label. Clicking an instance name filters the list to show only that instance's commands.

### Server Headers
When commands are grouped by instance, each group has a header showing the server label and a **close button** (`✕`) on the right side. Clicking the close button disconnects from that server, removing its commands from the sidebar. If any panels are actively displaying commands from that server, a confirmation dialog warns about the disconnect; the panels retain their last VTTY state after disconnection.

## Commands Tab

![Commands tab](screenshots/03-sidebar-commands.png)

### Filter Input
A text filter that narrows the command list in real time. Matches against command names, arguments, and PIDs. As you type, only matching commands remain visible. The filter also scopes the **Kill All** button — when a filter is active, Kill All only affects matching commands.

### Kill All Button
Terminates running commands across all instances, **respecting the current filter**. This button uses the danger style (red background) to indicate its destructive nature. Its behavior depends on the filter state:

- **Filter empty**: Kills all running commands on all reachable servers (same as before).
- **Filter active**: Only kills running commands whose name, arguments, or PID match the filter text. The confirmation dialog indicates the scope: "Kill N matching command(s)? (filter: "...")".

### Command List Items
Each command in the list uses a two-row layout with compact spacing. The first row shows the primary identification and action buttons; the second row shows live status information:

**Row 1 (name row):**

| Element | Description |
|---------|-------------|
| **Kill button** (`✕`) | Kills the individual command. Disabled when the server is unreachable |
| **Keep button** (`★`/`☆`) | Toggles retain-on-exit for the command. Active (filled) means the terminal is kept after the command exits |
| **Pin button** (`◉`/`◎`) | Pins/unpins a command to keep it at the top of the list. Uses a distinct symbol to differentiate from Keep |
| **Grab handle** (`⠇`) | Drag handle for reordering commands within the sidebar. Drag to a new position to set a custom sort order (persisted in localStorage) |
| **Command name** | The name of the running command, truncated with ellipsis if too long |
| **Certificate badge** | Shows the bound certificate name or `--` if none |
| **Exit badge** | Shows the exit code (green for 0, red for non-zero) |

**Row 2 (detail row):**

| Element | Description |
|---------|-------------|
| **Runtime** | Shows elapsed time (e.g., `5m 30s`) for running commands |
| **CPU usage** | Real-time CPU usage percentage (e.g., `12.5%`) |
| **Memory** | Real-time memory usage (e.g., `45.2MB`) |
| **PID** | Process ID of the child command |
| **Status** | `PAUSED` for frozen/paused commands |

Detail items are separated by `|` and rendered in a smaller, muted font. The detail row is indented to align with the command name. Resource data (CPU, memory) is polled from the server every 2 seconds via `/api/commands/{id}/resources`. CPU and memory values are shown without labels since the `%` and `MB` units make them self-explanatory.

The filter and kill-all toolbar at the top of the commands tab is automatically hidden when no server is reachable or when there are no running commands.

Clicking a command selects it and displays its terminal output in the main panel area. Right-clicking opens a context menu with additional actions.

### Command Reorder (drag-and-drop)
Commands can be reordered within the sidebar by dragging the **grab handle** (`⠇`) on the left side of each command item. The custom order is persisted in localStorage per server instance (`vrw_cmd_order`). When a custom order exists, commands are displayed in that order (with pinned commands still appearing first in a separate section). Dragging a command to a position above or below another command sets the insertion point (indicated by an accent-colored top/bottom border). Commands can only be reordered within the same server instance.

### Command States
- **Running**: Green status dot, normal background, runtime badge visible
- **Frozen/Paused**: Yellow status dot, `PAUSED` badge, tinted background
- **Exited**: Red status dot, red-tinted background, exit code badge, reduced opacity

See [Command States](./command-states.md) for detailed screenshots and explanations.

## Spawn Tab

![Spawn tab](screenshots/04-sidebar-spawn.png)

The spawn form allows you to create new commands with full control over all options. The form is decoupled from the currently focused panel — the Target Instance dropdown remembers your selection independently, so navigating between panels never resets where commands will be spawned.

### Target Instance Dropdown
Placed at the top of the form so the target server is selected first. The dropdown is fully decoupled from the focused panel's server connection: once you select an instance (or let it default to the first connection), it retains that selection across sidebar rebuilds, panel focus changes, and command spawns. This fixes a bug where the spawn instance would silently reset to whichever panel was focused.

### Command Field
The executable to run (e.g., `/usr/bin/htop`, `bash`, `npm`). Pressing Enter in this field triggers the spawn action.

### Arguments Field
Space-separated arguments passed to the command. Supports quoted strings with double quotes and single quotes. For complex arguments: `-c "echo hello; echo world"` or `--name 'my value'`. Backslash escapes are also supported. Pressing Enter triggers the spawn action.

### Environment Variables Field
A multi-line textarea for specifying per-command environment variables. Each line should be in `KEY=VALUE` format. Empty lines and lines starting with `#` are ignored as comments. Values may contain `=` signs (only the first `=` is treated as the separator). These environment variables are merged on top of the config-level `[environment]` variables, with per-command values taking precedence. This mirrors the env var support available in config templates and environment definitions. Pressing Ctrl+Enter in the textarea triggers the spawn action.

### Working Directory Field
The directory in which the command will be executed. Defaults to vrw's working directory if left empty. The server validates that the directory exists before spawning.

### Rows / Cols Fields
Set the initial terminal dimensions for the new command. Leave empty to use the server defaults (typically 24 rows × 80 cols).

### Auto-fit Button
Calculates the optimal terminal dimensions based on the current panel container size. It estimates character dimensions from the current font size and computes how many rows and columns fit. The result is shown in the **Auto-fit hint** below the fields.

### Certificate Dropdown
Select an optional named certificate to bind to the command. Certificate-bound commands require the matching certificate for API access. See the Certificates tab for managing certificates.

### Retain on Exit Checkbox
When checked, the terminal buffer is kept after the command exits, allowing you to review the final output.

### Open in New Panel Checkbox
When checked (default), spawning a command creates a new panel for it instead of taking over the currently focused panel. This decouples the spawn action from your current workspace — you can spawn commands on any instance without disturbing the view you're watching. The new panel displays the spawned command (or the main/oldest command if the spawn was part of a multi-command workflow). When unchecked, the traditional behavior applies: the spawned command takes over the focused panel's terminal view.

### Spawn Command Button
Submits the form and creates the new command. The button uses the primary style (green background) to indicate it's a creation action.

## Templates Tab

![Templates tab](screenshots/05-sidebar-templates.png)

Templates let you save and reuse common command configurations. Instead of re-typing the same command and arguments each time, you can save a template and spawn from it with one click. Templates can come from two sources: the server's configuration file (read-only in the web UI) and user-created templates saved in the browser's localStorage.

### Template Sources

Templates are displayed in two sections:

- **From config**: Templates defined in the server's configuration file under `[[templates]]` sections. These are read-only and cannot be deleted from the web UI. They are marked with a "config" badge.
- **Custom**: Templates created by the user via the + Add form. These are stored in the browser's localStorage and can be deleted from the web UI.

### Config File Templates

Templates can be defined in the vrw configuration file (TOML format). Each template specifies a name, command, and optional arguments, environment variables, working directory, certificate, and terminal dimensions.

**Example — Simple template:**
```toml
[[templates]]
name = "Dev Server"
cmd = "npm"
args = "run dev"
```
This template spawns `npm run dev` with default settings.

**Example — Template with environment variables and working directory:**
```toml
[[templates]]
name = "Backend API"
cmd = "cargo"
args = "run"
workdir = "/home/user/api"
env = ["DATABASE_URL=postgres://localhost/mydb", "RUST_LOG=debug", "PORT=3000"]
```
This template spawns the Rust backend with custom environment variables and a specific working directory.

**Example — Template with certificate and custom terminal size:**
```toml
[[templates]]
name = "Production Logs"
cmd = "tail"
args = "-f /var/log/app.log"
certificate = "prod-access"
rows = 50
cols = 120
```
This template spawns a log follower bound to the "prod-access" certificate with a wide terminal.

**Example — Multiple templates for different services:**
```toml
[[templates]]
name = "Frontend"
cmd = "npm"
args = "run dev"
workdir = "/home/user/frontend"

[[templates]]
name = "Backend"
cmd = "cargo"
args = "run"
workdir = "/home/user/api"
env = ["DATABASE_URL=postgres://localhost/mydb"]

[[templates]]
name = "Database Monitor"
cmd = "psql"
args = "-U admin -d mydb"
workdir = "/home/user"
```
Clicking any of these templates immediately spawns the command on the server instance currently selected in the Spawn tab's Target Instance dropdown.

### User Templates

Users can create custom templates directly in the web UI by clicking the **+ Add** button. The form has three fields:

- **Template name**: A display name for the template (e.g., "Run tests")
- **Command**: The executable to run (e.g., `pytest`)
- **Arguments**: Optional arguments (e.g., `tests/ -v --tb=short`)

User templates are saved in the browser's localStorage under the key `vrw_user_templates`. They persist across page reloads but are specific to the browser — they are not shared with other users or other browsers.

### Template Card Actions
Each template card can be clicked to immediately spawn that command with the saved configuration. The spawn targets the server instance currently selected in the Spawn tab's Target Instance dropdown. User-created templates also have a **Delete** button (`✕`) to remove them. Server-provided templates (from the configuration file) cannot be deleted from the web UI.

## Environments Tab

The Environments tab provides a mechanism for activating preconfigured workspace environments. Each environment defines a named set of panels, server connections, and commands to spawn. Environments are loaded from the server's configuration file (`[[environments]]` sections in TOML config).

### Environment List
Displays named workspace environments with a summary of their configuration (panel count, server count, command count). Clicking an environment entry activates it, creating all defined panels, connecting servers, and spawning the configured commands.

### Auto-Start
Environments can be marked for automatic activation on server boot via the `auto_start = true` flag in the configuration. Auto-started environments are immediately displayed by the UI when the page loads.

## Certificates Tab

![Certificates tab](screenshots/06-sidebar-certs.png)

Certificates enable per-command access control. Each certificate is a named token that can be bound to commands at spawn time. API requests must present the matching certificate to interact with certificate-bound commands.

### Certificate List
Shows all configured certificates with their names. Certificates can also be managed via the CLI (`vrw cert` subcommand).
