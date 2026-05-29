# Incremental Diff Streaming

This document explains how vrunner achieves bandwidth-efficient terminal streaming
by transmitting only the cells that have changed since the last update, rather than
re-sending the entire terminal buffer on every frame. It covers the problem that
motivates this optimization, the server-side diff computation, the three-phase
wire protocol, the data structures involved, the client-side rendering strategy, and
measured performance characteristics. Read this if you want to understand how vrunner
keeps terminal streaming responsive over slow or metered connections, or if you are
implementing a custom client.

---

## Problem Statement

A typical terminal emulator renders a grid of 80 columns × 24 rows = 1,920 cells.
Each cell contains a character, a foreground color, a background color, and style
flags (bold, italic, underline). If we serialize every cell as HTML on every
update, the payload grows quickly.

Consider a `top` command that updates every second. Without diffing, each update
requires sending the full 1,920-cell grid. Even with gzip compression, this is
wasteful because only a fraction of the cells change between updates—typically
the CPU percentages, the process table, and the load averages, while the header
and most of the layout remain static.

| Scenario | Grid Size | Changed Cells | Full HTML | Diff HTML | Reduction |
|---|---:|---:|---:|---:|---:|
| `top` (1 Hz) | 80×24 | ~400 | ~45 KB | ~8 KB | 82% |
| `vim` (typing) | 80×24 | ~5 | ~45 KB | ~0.2 KB | 99% |
| `tail -f` (slow) | 120×40 | ~120 | ~120 KB | ~3 KB | 97% |
| Build output (fast) | 120×40 | ~2000 | ~120 KB | ~80 KB | 33% |

The incremental diff approach reduces bandwidth usage by 80–99% for typical
workloads, and the reduction is most dramatic for the interactive editing and
monitoring scenarios that benefit most from responsiveness.

---

## Server-Side Diff Computation

The diff engine lives in `vtty/renderer.rs`. It operates on a simple principle:
maintain a snapshot of the last transmitted buffer and compare it cell-by-cell
with the current buffer on each tick.

### Polling Interval

The diff engine polls the VTTY emulator every **200 milliseconds** (5 Hz). This
interval balances two competing concerns:

- **Responsiveness** — A lower interval (e.g., 50ms) would reduce perceived
  latency for fast-scrolling output but increase CPU usage and the number of
  WebSocket messages.
- **Efficiency** — A higher interval (e.g., 500ms) would batch more changes
  into a single diff but make interactive input feel sluggish.

200ms was chosen as the sweet spot for most terminal workloads. It is configurable
via the `VRUNNER_DIFF_INTERVAL` environment variable.

### Cell-Level Comparison

For each cell in the grid, the engine computes whether the cell has changed by
comparing four properties:

```
function cells_differ(prev: &Cell, curr: &Cell) -> bool {
    prev.ch    != curr.ch
    || prev.fg  != curr.fg
    || prev.bg  != curr.bg
    || prev.flags != curr.flags
}
```

Only cells where any property has changed are included in the diff payload. The
comparison is O(rows × cols) per tick, which is trivial for typical grid sizes
(≤ 200×60 = 12,000 cells).

### Changed-Only Serialization

The diff payload is a JSON object that lists only the changed cells along with
their row and column coordinates. The cursor position is always included because
it affects rendering even when no cells change (e.g., a blinking cursor).

---

## Three-Phase Protocol

The streaming protocol between server and client has three distinct phases:

### Phase 1: Initial Snapshot (`vtty_full`)

When a client first connects (or reconnects after losing the diff stream), the
server sends the complete terminal state as a `vtty_full` message:

```json
{
  "type": "vtty_full",
  "data": {
    "cols": 80,
    "rows": 24,
    "cursor": { "col": 42, "row": 11, "visible": true },
    "cells": [
      { "r": 0, "c": 0, "ch": "u", "fg": 7, "bg": 0, "flags": 0 },
      { "r": 0, "c": 1, "ch": "s", "fg": 7, "bg": 0, "flags": 0 },
      ...
    ]
  }
}
```

The `vtty_full` message is typically large (tens of kilobytes) but is sent exactly
once per connection. It is also sent over gzip-compressed WebSocket.

### Phase 2: Incremental Diffs (`vtty_diff`)

After the initial snapshot, the server sends only the changes on each 200ms tick:

```json
{
  "type": "vtty_diff",
  "data": {
    "cursor": { "col": 43, "row": 11, "visible": true },
    "cells": [
      { "r": 11, "c": 42, "ch": "x", "fg": 10, "bg": 0, "flags": 0 },
      { "r": 23, "c": 0, "ch": ">", "fg": 7, "bg": 0, "flags": 0 }
    ]
  }
}
```

If no cells have changed and the cursor has not moved, the server sends nothing
(the tick is a no-op). This means the WebSocket is truly silent during idle
periods.

### Phase 3: Resynchronization

If the client detects a mismatch (e.g., it receives a diff that references a
row it doesn't have, or it has been disconnected for a long time), it can request
a fresh snapshot:

```
Client → Server:  { "type": "request_full" }
Server → Client:  { "type": "vtty_full", "data": { ... } }
```

The server also proactively resynchronizes when the terminal size changes
(because the entire grid may be reorganized) or when the diff engine detects that
its internal snapshot is out of sync with the current buffer.

```
┌──────────────────────────────────────────────────────────┐
│                    Protocol Timeline                       │
│                                                           │
│  Client connects                                          │
│       │                                                   │
│       ▼                                                   │
│  ┌──────────────┐                                        │
│  │ Phase 1:     │  Server sends vtty_full (complete grid)│
│  │ Initial Load │                                        │
│  └──────┬───────┘                                        │
│         │                                                 │
│         ▼                                                 │
│  ┌──────────────┐                                        │
│  │ Phase 2:     │  Server sends vtty_diff (changes only) │
│  │ Incremental  │  ◄──── every 200ms while changes exist │
│  │ Streaming    │                                        │
│  └──────┬───────┘                                        │
│         │                                                 │
│         │   desync / resize / reconnect                   │
│         ▼                                                 │
│  ┌──────────────┐                                        │
│  │ Phase 3:     │  Server sends fresh vtty_full           │
│  │ Resync       │                                        │
│  └──────┬───────┘                                        │
│         │                                                 │
│         ▼                                                 │
│  └──► Phase 2 (resume incremental diffs)                 │
└──────────────────────────────────────────────────────────┘
```

---

## Data Structures

### CellDiff

Represents a single changed cell:

```rust
#[derive(Serialize, Clone)]
pub struct CellDiff {
    pub r:     usize,    // Row index (0-based)
    pub c:     usize,    // Column index (0-based)
    pub ch:    char,     // Character
    pub fg:    u8,       // Foreground color index (256-color)
    pub bg:    u8,       // Background color index (256-color)
    pub flags: u8,       // CellFlags bitmask
}
```

### BufferDiff

The payload of a `vtty_diff` message:

```rust
#[derive(Serialize, Clone)]
pub struct BufferDiff {
    pub cursor: CursorDiff,
    pub cells:  Vec<CellDiff>,
}

#[derive(Serialize, Clone)]
pub struct CursorDiff {
    pub col:     usize,
    pub row:     usize,
    pub visible: bool,
}
```

### StoredSnapshot

The server's reference for computing diffs:

```rust
pub struct StoredSnapshot {
    pub cols:  usize,
    pub rows:  usize,
    pub cells: Vec<StoredCell>,  // flat array: cells[row * cols + col]
}

#[derive(Clone)]
pub struct StoredCell {
    pub ch:    char,
    pub fg:    u8,
    pub bg:    u8,
    pub flags: u8,
}
```

When the terminal is resized, the `StoredSnapshot` is discarded and a new full
snapshot is sent to the client (triggering Phase 3).

---

## Client Implementation Strategy

vrunner ships with a default admin interface that uses `xterm.js` for rendering.
However, the protocol is designed to support two rendering strategies:

### Strategy A: Direct DOM Updates (Recommended)

The client maintains a parallel grid of DOM elements (or a virtual DOM). On
receiving a `vtty_full`, it rebuilds the entire grid. On receiving a `vtty_diff`,
it updates only the DOM elements at the specified `(r, c)` coordinates.

This approach is used by the shipped admin interface and by `xterm.js`-based
clients. Advantages:

- Minimal DOM manipulation per frame.
- No string parsing or HTML injection required.
- Works correctly with scrolling, selection, and accessibility.

### Strategy B: HTTP Fallback

For environments where WebSocket is unavailable (e.g., behind restrictive proxies),
the client can poll the REST endpoint:

```
GET /api/commands/{id}/snapshot?seq=<last_seq>
```

If `seq` matches the server's current sequence number, the response is `204 No
Content`. Otherwise, the response is a `vtty_full` payload. This is
significantly less efficient than WebSocket streaming but provides a graceful
degradation path.

```
┌───────────────┐     WebSocket      ┌───────────────┐
│  Client       │◄═══════════════════►│  Server       │
│  (Strategy A) │  vtty_full/vtty_diff│  (diff engine)│
└───────────────┘                     └───────────────┘

┌───────────────┐     HTTP Poll      ┌───────────────┐
│  Client       │◄──────────────────►│  Server       │
│  (Strategy B) │  GET /snapshot     │  (diff engine)│
└───────────────┘                     └───────────────┘
```

---

## Performance Characteristics

### Bandwidth

| Metric | Value |
|---|---|
| Full snapshot (80×24) | ~40–60 KB (uncompressed), ~8–12 KB (gzip) |
| Typical diff (interactive) | ~0.1–1 KB (uncompressed) |
| Typical diff (fast scroll) | ~5–20 KB (uncompressed) |
| Idle period traffic | 0 bytes (no messages sent) |

### CPU

| Metric | Value |
|---|---|
| Diff computation per tick | ~0.05 ms (80×24 grid) |
| Diff computation per tick | ~0.2 ms (200×60 grid) |
| Memory overhead (snapshot) | ~120 KB per command (200×60) |

### Latency

| Metric | Value |
|---|---|
| Time-to-first-byte (connection) | < 5 ms (localhost) |
| Time-to-first-byte (remote) | + RTT |
| Diff delivery latency | ≤ 200ms + RTT |
| Resync latency | ≤ 400ms + RTT (2 ticks max) |

### Scaling

The diff engine is per-command and stateless (each command has its own snapshot).
Scaling to N commands requires O(N) memory and O(N) CPU at 5 Hz, which is
negligible for N < 1000 on modern hardware.

---

*This document is part of the [Diátaxis](https://diataxis.fr/) documentation framework
for vrunner. See the [explanation index](./) for related topics.*
