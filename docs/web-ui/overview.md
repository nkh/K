# Overview

The vrw web UI is the primary interface for interacting with running commands. It displays real-time terminal output, provides controls for sending keystrokes, and manages the full command lifecycle from spawn to termination.

## Screenshot

![Full overview](screenshots/01-overview.png)

## Key Features

- **Real-time terminal display**: View command output as it happens, with support for ANSI escape codes and full-color rendering
- **Per-panel WebSocket connections**: Each panel maintains its own dedicated WebSocket to stream VTTY updates independently, allowing simultaneous monitoring of multiple commands on different servers without interference
- **Multi-instance panels**: View commands from multiple vrw instances side by side in resizable panels. Panels are decoupled from server connections — an empty panel can exist without any connected server, and the user can assign a command to any panel at any time
- **Interactive keyboard input**: Send keystrokes, special keys, and multi-line input to any running command
- **Command lifecycle management**: Spawn, pause, resume, restart, and kill commands from the browser. The Kill All button terminates running commands across all connected servers simultaneously
- **Environment presets**: Define named workspace environments in the server configuration file (`[[environments]]` in TOML) that specify a complete layout of panels, server connections, and commands. Activate environments from the Envs tab in the sidebar or auto-start them on server boot
- **Search**: Search across all command output or within a single terminal buffer
- **Theming**: Three built-in themes (dark, light, grey) with automatic OS preference detection. Per-panel theme overrides allow mixing themes (e.g., dark UI with a light terminal)
- **Persistent settings**: Font size, theme, panel layout, sidebar visibility, and auth tokens are saved to `localStorage`
- **WebSocket and HTTP transport**: Terminal updates can be pushed via WebSocket or polled via HTTP. The spawn instance selection is now independent of the focused panel's server, preventing the bug where commands were spawned on the wrong server
- **Per-panel toolbar state**: Max Fit and Max Font toggle states are tracked independently per panel, ensuring that toggling on one panel does not affect others
