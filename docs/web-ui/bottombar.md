# Bottom Bar

The bottom bar displays status information about the selected command and the connection to the vrw server. It is hidden by default and can be toggled via the **Status** button in the top bar.

![Bottom bar](screenshots/10-bottombar.png)

## Elements

### Command Label
Shows the name, arguments, and PID of the selected command. The name is displayed in the primary color, arguments in secondary, and PID in muted text. Long labels are truncated with ellipsis.

### Cursor Position (`Cursor: --`)
Shows the current cursor row and column in the terminal buffer (e.g., `Cursor: 12, 40`). Updated in real time as the cursor moves.

### Terminal Dimensions (`--`)
Shows the current terminal dimensions in rows × columns format (e.g., `80x24`). This reflects the actual PTY size, which may differ from the displayed size if the terminal has been resized.

### SCROLLBACK Indicator
Appears when the terminal view is scrolled up from the bottom of the buffer. It is displayed in yellow to alert you that new output may not be visible.

### Update Mode Select
Controls how the web UI detects VTTY buffer changes:

| Mode | Description |
|------|-------------|
| **Push (WS)** | Server pushes dirty signals via WebSocket. Most efficient, lowest latency. |
| **Poll** | Client periodically requests the current terminal state via HTTP. Configurable interval. |

### Poll Interval
When in **Poll** mode, this field sets the polling interval in milliseconds (50–5000ms, default 500ms). Changes take effect immediately.

### WebSocket Quality Indicator
Shows the round-trip time (latency) of the WebSocket connection and reconnect count. Hover over it for a tooltip with detailed connection statistics.

### Connection Status (`Connected`)
Shows the current connection state to the vrw server. Displays **Connected** (green), **Reconnecting...** (yellow), or **Disconnected** (red).
