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

### Server-Side Rendering Benchmarks (measured)

Server-side HTML rendering and diff computation benchmarks (`cargo test --lib
vtty::renderer::tests::benchmark -- --nocapture`) on a debug build:

| Metric | 80×24 | 120×40 | 200×50 |
|---|---:|---:|---:|
| `to_html()` per frame | ~1.4 ms | ~3.3 ms | ~7.1 ms |
| HTML payload per frame | ~208 KB | ~520 KB | ~1,083 KB |
| `Buffer::diff()` per frame | ~61 µs | ~151 µs | ~310 µs |
| Diff computation per tick | ~0.05 ms (80×24 grid) |
| Diff computation per tick | ~0.2 ms (200×60 grid) |

### Client-Side Optimization Levels

The web admin interface implements three levels of optimization:

**Level 1 — Native Scroll + Scroll Position Preservation:**
- The browser's native scroll is no longer blocked for the live buffer view.
- Wheel events are intercepted only at the top edge (to enter scrollback history)
  or when in scrollback view (to navigate history via server-side offset).
- Native scroll provides smooth inertia, momentum, and GPU-accelerated compositing
  — the browser handles repaint timing, which is far more efficient than per-tick
  HTTP round-trips.
- `scrollTop` is saved before `innerHTML` replacement and restored after, using
  height-delta adjustment. Auto-scroll to bottom only happens when the user was
  already viewing the bottom.

**Level 2 — Generation-Based Skip:**
- The server includes a `generation` counter (monotonic `u64`) in every
  `vtty_full` WebSocket message and `GET /vtty/html` HTTP response.
- The client tracks `state._lastGeneration[cmdId]` and skips the entire DOM
  update (no `innerHTML`, no cursor repositioning) if the generation matches.
- This eliminates redundant work when multiple dirty signals arrive between
  client fetch cycles, or when the 50ms debounce window coalesces signals.
- Metadata-only updates (cursor position, dimensions, mouse state) are still
  applied even when the generation is unchanged.

**Level 3 — Server-Side Cell Diff + Client-Side Incremental DOM Patching:**
- The diff watcher (`spawn_diff_watcher` in `manager.rs`) maintains a local
  snapshot of the previous buffer state. When the generation counter changes,
  it computes a `Buffer::diff()` between the previous and current buffer.
- If dimensions changed or more than 90% of cells differ, the watcher falls
  back to sending `vtty_full` (complete HTML resync). Otherwise, it sends a
  `vtty_diff` WebSocket message containing only the changed cells as a
  `Vec<CellDiff>` JSON array.
- The client maintains a 2D cell grid (`_cellGrids[cmdId]`) that maps
  `(row, col)` positions to the corresponding `<span>` DOM elements inside
  the `<pre>` container. This grid is rebuilt after each full HTML replacement.
- On receiving a `vtty_diff` message, `applyVttyDiff()` patches only the
  changed spans in-place using `setAttribute('style', ...)` and
  `textContent` updates — no `innerHTML` replacement, no DOM destruction.
- The client-side `_cellStyle()` function generates inline CSS that exactly
  matches the server's `VttyRenderer::to_html()` output, ensuring visual
  consistency between full HTML replacement and incremental diff patching.
- If the cell grid is unavailable, dimensions have changed, or a grid
  desync is detected, the client automatically falls back to a full HTML
  fetch via `scheduleVttyHttp()`. The client can also send a `request_full`
  WebSocket message to request an immediate resync from the server.
- An HTTP endpoint `GET /api/commands/:id/vtty/diff` provides diff data for
  poll-mode clients that do not use WebSocket streaming.

### Bandwidth

| Metric | Value |
|---|---|
| Full snapshot (80×24) | ~208 KB (uncompressed), per `to_html` benchmark |
| Full snapshot (120×40) | ~520 KB (uncompressed) |
| Full snapshot (200×50) | ~1,083 KB (uncompressed) |
| Diff 1% cells (80×24, typing) | ~3.5 KB/msg |
| Diff 5% cells (80×24, interactive) | ~16.8 KB/msg |
| Diff 25% cells (80×24, partial) | ~83.1 KB/msg |
| Diff 5% cells (120×40, interactive) | ~41.7 KB/msg |
| Diff 1% cells (200×50, typing) | ~17.5 KB/msg |
| Diff 5% cells (200×50, interactive) | ~86.8 KB/msg |
| Idle period traffic | 0 bytes (no messages sent) |

### CPU

| Metric | Value |
|---|---|
| Diff computation per tick (80×24) | ~66 µs |
| Diff computation per tick (120×40) | ~164 µs |
| Diff computation per tick (200×50) | ~327 µs |
| Diff JSON serialization 5% (80×24) | ~1.1 ms |
| Diff JSON serialization 5% (120×40) | ~2.7 ms |
| Diff JSON serialization 5% (200×50) | ~5.8 ms |
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
