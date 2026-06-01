# Top Bar

The top bar contains global controls that affect the entire web UI. It is divided into three groups: left controls, center controls, and right controls.

![Top bar](screenshots/02-topbar.png)

## Left Group

### Toggle Sidebar (`☰`)
Collapses or expands the sidebar. The sidebar state persists across page reloads. When collapsed, the sidebar width becomes zero and the terminal area expands to fill the space.

### + Panel
Opens a dialog to add a new instance panel. Each panel connects to a different vrw instance, allowing you to monitor commands from multiple servers simultaneously.

### Search Output (`🔍`)
Opens the global search overlay, which searches across all command output buffers. See [Global Search](./global-search.md) for details.

## Center Group

### Font Size Controls (`A-` / `10px` / `A+`)
Adjusts the global terminal font size. The range is 8px to 28px, and the selected size is saved to `localStorage`. This affects all terminal panels unless overridden by per-panel font controls.

### Terminal Resize (`R:` / `C:` / `Resize`)
Manually set the terminal dimensions (rows and columns) for the selected command's PTY. Enter the desired values and click **Resize** to send a `SIGWINCH` signal to the running process. The values must be within 1–200 rows and 1–500 columns.

### Buffer Select (`Current` dropdown)
Switches which terminal buffer is displayed in the selected panel:

| Option | Description |
|--------|-------------|
| **Current** | Shows the active buffer (main or alternate, depending on what the application is using) |
| **Main Buffer** | Forces display of the main screen buffer |
| **Alt Buffer** | Forces display of the alternate screen buffer (used by full-screen apps like htop, vim) |

When viewing the alternate buffer, an **ALT SCREEN** badge appears in the top bar.

## Right Group

### Theme Select (`Auto` / `Dark` / `Light` / `Grey`)
Selects the color theme for the entire UI. **Auto** follows the operating system's `prefers-color-scheme` setting.

### Sound Notifications (`🔔`)
Toggles audible notifications. When enabled, a bell sound plays when a command exits or produces a bell character (`\a`). The active state is indicated by a yellow highlight on the button.

### Logs
Toggles the log viewer, which shows vrw's internal event log. See [Log Viewer](./log-viewer.md) for details.

### Status
Toggles the bottom status bar, which shows cursor position, terminal dimensions, connection status, and update mode settings.

### Token Field
Enter a bearer token for API authentication. Click **Set** to save the token to `localStorage`. This token is sent with every API request in the `Authorization` header. Required when vrw is started with `--auth` or `--remote`.

### Docs
Opens the built-in documentation view within the web UI.

### Keyboard Shortcuts (`?`)
Opens the keyboard shortcuts reference overlay. See [Keyboard Shortcuts](./shortcuts.md) for the full list.

### Theme Toggle (`☾` / `☀`)
Quickly toggles between light and dark themes. Overrides the theme selector dropdown and saves the preference.
