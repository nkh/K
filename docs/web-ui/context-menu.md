# Context Menu

Right-clicking on a command in the sidebar opens a context menu with additional actions for that command.

![Context menu](screenshots/21-context-menu.png)

## Menu Items

### Kill
Terminates the selected command. Equivalent to clicking the red kill button (`✕`) in the command list.

### Freeze / Thaw
Toggles the command between paused (SIGSTOP) and running (SIGCONT) states. When frozen, the command's CPU usage drops to zero and its terminal output stops updating.

### Restart
Restarts the command with the same configuration (command, arguments, working directory, environment).

### Copy Name
Copies the command name to the clipboard.

### Copy Command Line
Copies the full command line (name + arguments) to the clipboard.

### Copy Terminal
Copies the terminal output to the clipboard.

### Export Terminal
Downloads the full terminal buffer as a text file.

### Purge (exited commands only)
Permanently removes an exited command from the manager. This discards the VTTY buffer and all associated state. Only available for commands that have already exited.

### Pin / Unpin
Pins or unpins the command. Pinned commands always appear at the top of the command list, regardless of filter or sort order.

## Keyboard Navigation

The context menu supports keyboard navigation:
- **↑** / **↓** to move between items
- **Enter** to activate the selected item
- **Escape** to close the menu
- Tab focus is trapped within the menu while it is open
