# Context Menu

Right-clicking on a command in the sidebar opens a context menu with additional actions for that command.

![Context menu](screenshots/21-context-menu.png)

## Menu Items

### View Terminal
Selects the command and switches to its terminal panel. Equivalent to clicking the command in the sidebar.

### Copy URL
Copies the vrunner instance URL for the command to the clipboard. Useful when working with multi-instance setups.

### Kill
Terminates the selected command. Equivalent to clicking the red kill button (`✕`) in the command list.

### Freeze / Thaw
Toggles the command between paused (SIGSTOP) and running (SIGCONT) states. When frozen, the command's CPU usage drops to zero and its terminal output stops updating.

### Restart
Restarts the command with the same configuration (command, arguments, working directory, environment).

### Purge (exited commands only)
Permanently removes an exited command from the manager. This discards the VTTY buffer and all associated state. Only available for commands that have already exited.

## Keyboard Navigation

The context menu supports keyboard navigation:
- **↑** / **↓** to move between items
- **Enter** to activate the selected item
- **Escape** to close the menu
- Tab focus is trapped within the menu while it is open
