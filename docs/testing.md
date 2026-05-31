# Testing Procedures

This document describes how to run the project's test suites, what each suite covers, and how to verify correctness after making changes.

The repository contains two binaries compiled via Cargo feature flags:

| Binary | Feature flag | Description |
|--------|-------------|-------------|
| `vrl`   | `vrl`       | UDS-based IPC interface |
| `vrunner` | `vrunner` | HTTP server interface |

Both binaries share the `vrl_core` library crate. Test coverage is split across three test files, with some tests gated behind `#[cfg(feature = "vrunner")]` when they depend on vrunner-specific `Config` fields (server, security, TLS).

## Quick Reference

```bash
# Run all tests — default feature (vrl only)
cargo test --release

# Run all tests — vrunner binary path (HTTP server / Config fields)
cargo test --release --features vrunner

# Run all tests — both binary paths
cargo test --release --features "vrl,vrunner"

# Run a specific test suite
cargo test --release --features vrunner renderer          # VTTY renderer tests
cargo test --release --features vrunner diff               # Buffer diff benchmarks
cargo test --release --features vrunner diff_json          # Diff JSON serialization benchmarks
cargo test --release --features vrunner to_html           # HTML rendering correctness
cargo test --release --features vrunner to_png            # PNG rendering tests
cargo test --release --features "vrl,vrunner" comprehensive # Comprehensive tests
cargo test --release --features vrunner integration       # Integration tests
cargo test --release --features vrunner regression        # Regression tests

# Run only library unit tests (fast, no I/O)
cargo test --lib

# Run library unit tests with vrunner feature
cargo test --lib --features vrunner

# Run with output
cargo test --release --features "vrl,vrunner" -- --nocapture

# Check formatting
cargo fmt -- --check

# Lint (with vrunner feature to cover all code paths)
cargo clippy --release --features "vrl,vrunner"
```

## Feature Flags and Binaries

The `vrl` feature is the default. It compiles the `vrl` binary and the `vrl_core` library without HTTP server dependencies. The `vrunner` feature additionally pulls in axum, reqwest, rustls, and other server dependencies, and compiles the `vrunner` binary.

```
[features]
default = ["vrl"]
vrl = []
vrunner = [ "dep:axum", "dep:axum-server", "dep:tower", ... ]
```

### Which feature flag to use?

- **Working on VTTY / emulator / renderer**: `cargo test --release` (vrl is default, sufficient)
- **Working on config schema, server, or HTTP API**: `cargo test --release --features vrunner`
- **CI / pre-merge checks**: `cargo test --release --features "vrl,vrunner"`

## Test Suites

### 1. Unit Tests (`src/*/tests` and inline `#[test]`)

Unit tests are embedded directly in source files using Rust's `#[test]` attribute. They run quickly (milliseconds) and require no external services.

#### VTTY Renderer Tests (`src/vtty/renderer.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_to_plain` | Plain text serialization preserves cell characters |
| `test_to_ansi` | ANSI output includes correct SGR color codes |
| `test_to_html` | HTML output uses hex color format (`#RRGGBB`), CSS class `c`, no inline-block |
| `test_html_escape_metacharacters` | All five XML metacharacters (`<>&'"`) are properly escaped |
| `test_lines_with_scrollback` | Scrollback lines are prepended correctly |
| `benchmark_to_html` | Measures HTML generation speed for small/medium/large buffers |
| `benchmark_buffer_diff` | Measures cell-level diff computation speed |
| `benchmark_diff_json_serialization` | Measures JSON serialization speed for diff data at various change rates |

**After modifying the renderer** (e.g., changing RLE logic, color format, empty cell handling), always run:

```bash
cargo test --release --features "vrl,vrunner" renderer
```

#### Cell Tests (`src/vtty/cell.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_cell_default` | Default cell has space character, default colors, no decorations |
| `test_cell_new` | `Cell::new('X')` sets character, uses default colors |
| `test_cell_clear` | `Cell::clear()` resets to default state |
| `test_cell_is_empty` | `Cell::default().is_empty()` returns true; modified cell returns false |

### 2. Comprehensive Tests (`tests/comprehensive_test.rs`)

Multi-module tests covering the `vrl_core` library. These tests are organized by module and each is independent with no external state.

| Module | Feature requirement | What it verifies |
|--------|-------------------|-------------------|
| VTTY buffer, cell, color, emulator | Both | Buffer operations, cell properties, color palette, full terminal emulator (CSI sequences, SGR, OSC, alternate screen, scroll regions, etc.) |
| Config schema, merge, validation | Mixed (see below) | Config deserialization, merge logic, profile overrides, validation rules |
| Process error types | Both | Error display, `Send + Sync`, `std::error::Error` source chain |
| Handle registry, null sink, file sink | Both | Handle lifecycle, sink write/flush |
| CommandLogger | Both | Memory buffer, broadcast subscriber, ring buffer |
| VTTY renderer (HTML) | Both | HTML output structure |
| Rate limiter | Both | Token bucket / rate limiting behavior |

**Config and validation tests** are split:

- **Both features**: `config_merge_handles_keeps_global_when_local_empty`, `config_environment_variables`, `validation_default_config_no_errors`, `validation_vtty_zero_rows_is_error`, `validation_vtty_zero_cols_is_error`, `validation_refresh_ms_too_low`
- **vrunner only** (`#[cfg(feature = "vrunner")]`): `config_default_values`, `config_deserialize_minimal_json`, `config_deserialize_full_json`, `config_serialize_roundtrip`, `config_merge_local_overrides_global`, `config_apply_profile_*`, `config_partial_config_all_none`, `validation_port_zero_is_error`, `validation_bind_empty_is_error`, `validation_multiple_issues`

```bash
# Run all comprehensive tests (both features recommended for full coverage)
cargo test --release --features "vrl,vrunner" comprehensive
```

### 3. Integration Tests (`tests/integration_test.rs`)

Tests that exercise multi-component interactions and the `CommandManager`.

| Test | Feature requirement | What it verifies |
|------|-------------------|-------------------|
| `test_key_encoding` | Both | `encode_keys` correctly translates named keys (`<C-c>`, `<Enter>`, `<Up>`, etc.) |
| `test_spawn_and_list` | vrunner | Spawning a command and listing it via `CommandManager` |
| `test_vtty_contents` | vrunner | VTTY plain-text output after command execution |
| `test_send_keys` | vrunner | Sending keystrokes to a running command's stdin |

Tests that use `CommandManager::new(Config { server, security, tls, vtty, ... })` require the `vrunner` feature because those `Config` fields are only compiled when the feature is enabled.

```bash
# Integration tests require vrunner feature for Config fields
cargo test --release --features vrunner integration
```

### 4. Regression Tests (`tests/regression_test.rs`)

Tests that prevent re-introduction of known bugs. Added when a bug is discovered and fixed.

| Section | Feature requirement | What it verifies |
|---------|-------------------|-------------------|
| 1. Command lifecycle | vrunner | spawn, list, kill, purge, find_by_pid, kill_by_pid |
| 2. IPC simulation | vrunner | HTTP client timeout behavior |
| 3. Exit behavior | Mixed | ExitConfig defaults and serialization (both); process removal on exit (vrunner) |
| 4. Multi-command management | vrunner | Spawn after kill, concurrent spawns, kill-one-preserves-others |
| 5. VTTY operations | vrunner | Snapshot, HTML, plain text, resize, diff, has_changed |
| 6. Key encoding | Mixed | send_keys delivery (vrunner); encode_keys correctness (both) |
| 7. Config and CLI overrides | Mixed | Full Config validation/roundtrip (vrunner); ExitConfig serialization (both) |
| 8. Instance registry | vrunner | InstanceRegistry creation |
| 9. Broadcast / shutdown signals | Both | tokio broadcast channel propagation, watch channel semantics |
| 10. Edge cases | vrunner | Command-not-found errors, env vars, custom VTTY size, logger |

**Tests that work with both features** (no `#[cfg(feature = "vrunner")]` gate):

- `regression_exit_config_default_no_retain`, `regression_exit_config_retain_true`, `regression_exit_config_snapshot_on_exit`
- `regression_encode_all_special_keys`, `regression_encode_plain_text_unchanged`
- `regression_exit_config_serialize`
- `regression_shutdown_signal_propagates`, `regression_watch_channel_stores_value`, `regression_watch_channel_already_changed`

```bash
# Run all regression tests (vrunner feature recommended for full coverage)
cargo test --release --features vrunner regression
```

### 5. Performance Benchmarks

Several tests in `renderer.rs` are benchmarks that print timing information. These are not assertions — they report performance metrics to stderr.

To run benchmarks:

```bash
cargo test --release -- --nocapture 2>&1 | grep -E '(avg|benchmark|to_html|diff)'
```

Expected output (approximate, varies by hardware):

```
  to_html(80x24 (small)) — 100 iterations, avg ~200 µs/frame, ~12 KB/frame
  to_html(120x40 (medium)) — 100 iterations, avg ~500 µs/frame, ~25 KB/frame
  to_html(200x50 (large)) — 100 iterations, avg ~1200 µs/frame, ~70 KB/frame
  diff(80x24 (small)) — 10000 iterations, avg ~500 ns/diff
  diff(120x40 (medium)) — 10000 iterations, avg ~1500 ns/diff
```

These benchmarks are useful for measuring the impact of RLE optimizations. A regression of >2x in HTML generation time suggests a problem with the run-length encoding.

### 6. Manual Testing Procedures

#### Web Dashboard Smoke Test (vrunner only)

After starting vrunner with a command, verify the following in the web dashboard:

1. **Initial load**: Open the admin page. The terminal should appear within 200ms (measured with browser DevTools Network tab — look for the `/api/snapshot` response time plus rendering time).
2. **Text alignment**: Terminal output should have proper column alignment. Text in adjacent columns should be vertically aligned, not shifted.
3. **Sidebar detail row**: Each command in the sidebar should show a second line with runtime, CPU%, MEM, and PID. These values should update every ~2 seconds.
4. **Refresh throttle**: Click the `↻` widget's `+` button several times. The value should increase in 100ms steps. Terminal updates should become noticeably less frequent at higher values. Click `-` to return to `off`.
5. **Kill all**: When no commands are running, the filter input and Kill All button should be hidden from the sidebar.
6. **Screenshot**: Click the screenshot button (📷). The downloaded file should be named `vrunner_YYYYMMDD_HHMMSS_rowsxcols_command.png`.

#### CLI Screenshot Test

```bash
# vrl binary (UDS IPC)
vrl run htop
vrl screenshot
# Expected: prints absolute path like /home/user/vrl_YYYYMMDD_HHMMSS_80x24_htop.png

# vrunner binary (HTTP server)
vrunner run htop
vrunner screenshot
# Expected: prints absolute path like /home/user/vrunner_YYYYMMDD_HHMMSS_80x24_htop.png

# Custom output path (--output suppresses stdout path)
vrl screenshot --output /tmp/custom.png htop
```

#### VTTY Rendering Tests

These are verified visually by running commands with known output patterns:

```bash
# Color test — should show 256-color palette with correct alignment
vrl run -- bash -c "for c in {0..255}; do echo -en \"\e[38;5;${c}m█\e[0m\"; done; echo"

# Box drawing — should render continuous lines, no gaps
vrl run -- bash -c "echo '┌────┬────┐'; echo '│ ab │ cd │'; echo '├────┼────┤'; echo '│ ef │ gh │'; echo '└────┴────┘'"

# Wide characters — CJK characters should occupy exactly 2 columns
vrl run -- bash -c "echo '你好世界 Hello World'"

# Scrolling — rapid output should display smoothly without visible lag
vrl run -- bash -c "for i in $(seq 1 1000); do echo \"Line $i: $(head -c 60 /dev/urandom | base64)\"; done"
```

## Common Issues

### Test fails with "No running commands" or "connection refused"

The integration tests that require a running vrunner instance should be run after starting vrunner. Most unit tests do not require this.

### Tests timeout

If tests hang, check for leftover processes:

```bash
pkill -f vrunner
pkill -f vrl
cargo test --release --features "vrl,vrunner"
```

### Benchmark numbers vary wildly

Benchmarks are sensitive to CPU frequency scaling, thermal throttling, and system load. Run benchmarks multiple times and compare medians. The `--release` profile is essential for meaningful measurements.

### "Config has no field `server`" compile error

Some tests and code paths require the `vrunner` feature. If you see errors about missing fields on `Config`, ensure you're building with the correct feature:

```bash
cargo test --features vrunner
```
