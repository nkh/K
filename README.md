# vrl

A virtual terminal runner and process orchestrator with UDS IPC. Run commands in pseudo-terminals, monitor them through a local terminal display, and control them via Unix Domain Socket — all from a single statically-linked binary with under 5ms startup.

## Features

- **UDS IPC** — All inter-instance communication uses Unix Domain Sockets with length-prefixed JSON protocol
- **Interactive Display** — Full terminal UI with tab bar, search, copy/paste, split-pane, and scrollback navigation
- **Daemon Mode** — Background execution with double-fork, detachable from the terminal
- **Multi-Instance** — Run multiple vrl servers, discover and manage them from the CLI
- **Configuration** — YAML/TOML/JSON with 3-layer precedence, named profiles, environment variable control
- **Advanced VTTY** — Scrollback, search, mouse support, alternate screen, 256/truecolor
- **Process Control** — Freeze/thaw (SIGSTOP/SIGCONT), graceful shutdown with timeouts, exit handlers, per-command retain/snapshot on exit, initial keystroke injection

## Quick Start

```bash
# Build from source
cargo build --release

# Run a command
vrl -- htop

# Run with local terminal display
vrl --display -- htop

# Send initial keystrokes and retain buffer after exit
vrl --retain-on-exit --send-keys "ls<Enter>" -- bash

# Run as a background daemon
vrl --daemon -- npm run dev
```

## Installation

### From Source

```bash
git clone https://github.com/nkh/K.git
cd K
cargo build --release
# Binary at target/release/vrl
```

### System-Wide Install

```bash
cargo install --path .
```

### Man Pages

```bash
cp man/vrl.1 /usr/local/share/man/man1/
man vrl
```

## Documentation

Documentation is organized using the [Diataxis framework](https://diataxis.fr/) into four quadrants. Start at the **[documentation index](docs/index.md)** to find what you need.

| Quadrant | Description | Start here |
|----------|-------------|------------|
| [Tutorials](docs/tutorials/getting-started.md) | Hands-on lessons for new users | Lesson 1: Your First Command |
| [How-To Guides](docs/how-to-guides/) | Task-oriented recipes for specific goals | Pick the task you want |
| [Reference](docs/configuration.md) | Authoritative specs: config, CLI, protocol | Look up a value |
| [Explanation](docs/explanation/architecture.md) | Concepts, architecture, design decisions | Understand why |

**Other resources:**

| Document | Description |
|----------|-------------|
| [FAQ](docs/faq.md) | Frequently asked questions |
| [User Manual](MANUAL.md) | Comprehensive all-in-one reference |
| [docs/examples/](docs/examples/) | Complete example configuration files |
| [man/vrl.1](man/vrl.1) | Unix manpage |

## License

Dual-licensed under **GPL-3.0-or-later** or **Artistic-2.0** — see [LICENSE](LICENSE) for the full text.
