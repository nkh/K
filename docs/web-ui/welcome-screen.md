# Welcome Screen

When no vrw instance is reachable (the server is not running or has exited), the web UI displays a connection error panel in the main content area. This is the only state in which the welcome panel appears.

![Welcome panel](screenshots/19-welcome-panel.png)

## Elements

### vrw Logo
The vrw favicon image is displayed at the top of the panel.

### Status Message
Displays **"vrw is not running"** in a prominent error style, indicating that the web UI cannot connect to a vrw instance.

### Instance URL
Shows the URL that the web UI attempted to connect to (e.g., `http://127.0.0.1:9090`). This helps diagnose connection issues — verify that vrw is running and listening on the expected address and port.

### Instruction
Displays **"Start vrw and refresh this page to connect."** as guidance for the user.

## Transition

The welcome panel automatically disappears when a vrw instance becomes reachable (when the server responds to the `/api/commands` endpoint). This happens automatically via the periodic command refresh cycle — no page reload is needed. The panel reappears if the server becomes unreachable (e.g., vrw is stopped or the connection is lost).

## Note

The welcome panel is NOT shown when the server is running but no commands have been spawned yet. In that state, the terminal area shows an empty panel with the message "No command selected — spawn or select a command to view its output". Use the **Spawn** tab in the sidebar to create your first command.
