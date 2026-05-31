# vrl Documentation

A virtual terminal runner with UDS IPC control plane.

## Overview

## Where to Start

| You want to... | Read this first |
|----------------|-----------------|
| Learn vrl from scratch | [Getting Started](tutorials/getting-started.md) |
| Run your first command in 5 minutes | [Getting Started, Lesson 1](tutorials/getting-started.md#lesson-1-your-first-command) |
| Look up a config option | [Configuration](configuration.md) |
| Understand the architecture | [Architecture](explanation/architecture.md) |
| Troubleshoot a problem | [FAQ](faq.md) |

## Document Index

### Tutorials
| Document | Description |
|----------|-------------|
| [Getting Started](tutorials/getting-started.md) | Lessons: install, first command, config, display, CLI commands |

### How-To Guides
| Document | Description |
|----------|-------------|
| [Running Commands](how-to-guides/running-commands.md) | Spawn commands via CLI and UDS IPC |
| [Daemon Mode](how-to-guides/daemon-mode.md) | Run vrl as a background service |
| [Interactive Display](how-to-guides/interactive-display.md) | Local TUI with tabs, search, split-pane, mouse support |
| [Configuration Profiles](how-to-guides/configuration-profiles.md) | Named presets for dev, staging, production |
| [Snapshots and Diffs](how-to-guides/snapshots-diffs.md) | Capture and compare terminal state |
| [Environment Variables](how-to-guides/environment-variables.md) | Three-layer environment variable control |
| [Event Hooks](how-to-guides/hooks.md) | Run shell commands on spawn, exit, error, kill events |

### Reference
| Document | Description |
|----------|-------------|
| [Configuration](configuration.md) | All config fields, CLI flags, types, defaults, and precedence |
| [CLI Reference](reference/cli.md) | All flags, subcommands, and key notation |
| [Keybindings](reference/keybindings.md) | Default and customizable keyboard shortcuts |
| [Usage](usage.md) | Practical guide to using vrl |

### Explanation
| Document | Description |
|----------|-------------|
| [Architecture](explanation/architecture.md) | System design, module breakdown, data flow, concurrency model |
| [Comparison](explanation/comparison.md) | Feature matrix: vrl vs tmux, screen, mprocs |
| [Lifecycle Policy](explanation/lifecycle-policy.md) | Start, retain, and exit behavior |

### Other
| Document | Description |
|----------|-------------|
| [FAQ](faq.md) | Frequently asked questions |
