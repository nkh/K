# vrunner Documentation

This documentation follows the [Diataxis framework](https://diataxis.fr/) — a systematic approach to technical documentation that separates content by its purpose and the reader's learning stage.

## The Four Quadrants

### [Tutorials](tutorials/) — Learning-Oriented

Guided, lesson-based instruction for **new users**. Follow them in order, hands-on keyboard.

Start here if you have never used vrunner before.

### [How-To Guides](how-to-guides/) — Goal-Oriented

Practical recipes for **achieving specific tasks**. Each guide solves one problem end-to-end.

Use these when you know what you want to accomplish but need the steps.

### [Reference](reference/) — Information-Oriented

Authoritative, technical specifications. CLI flags, config keys, API endpoints, WebSocket protocol, keybindings.

Use these when you need to look up an exact value, parameter, or behavior.

### [Explanation](explanation/) — Understanding-Oriented

Conceptual and architectural discussions. Why things work the way they do, design decisions, comparisons.

Use these when you want to understand the reasoning behind a feature or design.

## Where to Start

| You want to... | Read this first |
|----------------|-----------------|
| Learn vrunner from scratch | [Tutorials: Getting Started](tutorials/getting-started.md) |
| Run your first command in 5 minutes | [Tutorials: Getting Started, Lesson 1](tutorials/getting-started.md#lesson-1-your-first-command) |
| Set up a dev server dashboard | [How-To: Development Server](how-to-guides/dev-server.md) |
| Look up a config option | [Reference: Configuration](reference/configuration.md) |
| Find an API endpoint | [Reference: API](reference/api.md) |
| Understand the architecture | [Explanation: Architecture](explanation/architecture.md) |
| Compare vrunner with tmux | [Explanation: Comparison](explanation/comparison.md) |
| Troubleshoot a problem | [FAQ](faq.md) |

## Full Document Index

### Tutorials
| Document | Description |
|----------|-------------|
| [Getting Started](tutorials/getting-started.md) | Lessons 1-15: install, first command, web UI, API, config, display, TLS |

### How-To Guides
| Document | Description |
|----------|-------------|
| [Running Commands](how-to-guides/running-commands.md) | Spawn commands via CLI, web UI, and API |
| [Using the Web Dashboard](how-to-guides/web-dashboard.md) | Navigate and use the admin interface |
| [Using the REST API](how-to-guides/api-usage.md) | Programmatic control with curl and scripts |
| [Configuration Profiles](how-to-guides/configuration-profiles.md) | Named presets for dev, staging, production |
| [Remote Access with TLS](how-to-guides/remote-tls.md) | Secure remote connections with self-signed or custom certs |
| [Daemon Mode](how-to-guides/daemon-mode.md) | Run vrunner as a background service |
| [Certificate Management](how-to-guides/certificates.md) | Per-command access control with named certificates |
| [CI/CD Pipeline](how-to-guides/ci-pipeline.md) | Integrate vrunner into build pipelines |
| [Development Server](how-to-guides/dev-server.md) | Monitor frontend, backend, and DB from one dashboard |
| [Multi-Service Monitoring](how-to-guides/multi-service.md) | Run and monitor multiple production services |
| [Pair Programming](how-to-guides/pair-programming.md) | Share terminal sessions between developers |
| [Interactive Display](how-to-guides/interactive-display.md) | Local TUI with tabs, search, split-pane, mouse support |
| [Snapshots and Diffs](how-to-guides/snapshots-diffs.md) | Capture and compare terminal state for testing |
| [Environment Variables](how-to-guides/environment-variables.md) | Three-layer environment variable control |
| [Event Hooks](how-to-guides/hooks.md) | Run shell commands on spawn, exit, error, kill events |

### Reference
| Document | Description |
|----------|-------------|
| [Configuration](reference/configuration.md) | All config fields, CLI flags, types, defaults, and precedence |
| [API](reference/api.md) | Complete REST API endpoint reference with request/response schemas |
| [WebSocket Protocol](reference/websocket.md) | WebSocket message formats for VTTY and log streaming |
| [CLI Reference](reference/cli.md) | All flags, subcommands, and key notation |
| [Keybindings](reference/keybindings.md) | Default and customizable keyboard shortcuts |
| [Man Page](../../man/vrunner.1) | Unix manpage (`man vrunner`) |

### Explanation
| Document | Description |
|----------|-------------|
| [Architecture](explanation/architecture.md) | System design, module breakdown, data flow, concurrency model |
| [Comparison](explanation/comparison.md) | Feature matrix: vrunner vs tmux, screen, mprocs, gotty, wetty |
| [Incremental Diff Protocol](explanation/incremental-diff.md) | How the bandwidth-optimized VTTY streaming works |
| [Security Model](explanation/security-model.md) | Authentication, TLS, certificate-based access control |
| [Lifecycle Policy](explanation/lifecycle-policy.md) | How vrunner decides when to start, retain, and exit |

### Other
| Document | Description |
|----------|-------------|
| [FAQ](faq.md) | 50+ frequently asked questions |
| [User Manual](../../MANUAL.md) | Comprehensive all-in-one reference (pre-Diataxis) |
| [Example Configs](../examples/) | Complete YAML, TOML config files for common scenarios |
