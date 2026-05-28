# Panel Header

The panel header appears above each terminal view and contains per-panel controls for managing the selected command. When multiple panels are open (multi-instance mode), each panel has its own header.

![Panel header](screenshots/07-panel-header.png)

## Elements

### Drag Handle (`⠿`)
Visible only when multiple panels are open. Drag this handle to reorder panels horizontally. The panel being dragged shows a reduced opacity, and drop zones are indicated by colored border highlights on adjacent panels.

### Command Info
Displays the full command name and arguments of the currently selected command. The command name uses a monospace font and is truncated with an ellipsis if it exceeds the available width. The arguments appear below in a smaller, muted font.

### Resource Badge
Shows real-time CPU percentage and memory usage for the selected command (e.g., `CPU 2.3% | 14.5MB`). This data is polled periodically from the server.

### Instance URL
Shows the URL of the vrunner instance this panel is connected to. Truncated if too long.

### Panel Font Size (`A-` / `10px` / `A+`)
Adjusts the font size for this specific panel only, independent of the global font size setting. Each panel's font size is saved to `localStorage` with its own key.

### Pause/Resume Button (`⏸ Pause` / `▶ Run`)
Toggles between pausing (SIGSTOP) and resuming (SIGCONT) the selected command. When a command is paused, the button changes to a green "Run" style. This button is hidden when the selected command has exited.

### Restart Button (`↻`)
Restarts the selected command by re-spawning it with the same command, arguments, working directory, and environment variables.

### Send Keys Input
A text field for typing keystrokes to send to the running command. The input accepts text as-is — typed characters are sent directly to the PTY. Press Enter to send the keystrokes, or click the **Send** button. See [Send Keys](./send-keys.md) for details on typing special keys.

### Send Button
Sends the contents of the send keys input field to the selected command's PTY. Equivalent to pressing Enter in the input field.

### Help Button (`?`)
Opens the special keys reference modal, which explains how to type special keys (Return, Backspace, Escape, arrow keys, etc.) in the send keys input field. See [Special Keys Reference](./special-keys.md).

### Remove Panel Button (`✕`)
Visible only when multiple panels are open. Removes this panel from the display. The underlying vrunner instance is not affected.

### Copy Button
Copies any selected text in the terminal to the clipboard. Text must be selected using the **Select** mode first.

### Export Button (`⤋`)
Exports the entire terminal buffer as plain text. This includes the full scrollback buffer, not just the visible portion.

### Panel Theme Button (`◯` / `☾` / `☀`)
Cycles the terminal area's theme through: inherit from global → light → dark → inherit. This allows having a dark UI with a light terminal (or vice versa), which is useful for readability.
