# Testing Procedures

This document describes how to run the project's test suites, what each suite covers, and how to verify correctness after making changes.

The repository contains two binaries compiled via Cargo feature flags:

| Binary | Feature flag | Description |
|--------|-------------|-------------|
| `vrc`   | `vrc`       | UDS-based IPC interface |
| `vrw` | `vrw` | HTTP server interface |

Both binaries share the `vrc_core` library crate. Test coverage is split across three test files, with some tests gated behind `#[cfg(feature = "vrw")]` when they depend on vrw-specific `Config` fields (server, security, TLS).

## Quick Reference

```bash
# Run all tests — default feature (vrc only)
cargo test --release

# Run all tests — vrw binary path (HTTP server / Config fields)
cargo test --release --features vrw

# Run all tests — both binary paths
cargo test --release --features "vrc,vrw"

# Run a specific test suite
cargo test --release --features vrw renderer          # VTTY renderer tests
cargo test --release --features vrw diff               # Buffer diff benchmarks
cargo test --release --features vrw diff_json          # Diff JSON serialization benchmarks
cargo test --release --features vrw to_html           # HTML rendering correctness
cargo test --release --features vrw to_png            # PNG rendering tests
cargo test --release --features "vrc,vrw" comprehensive # Comprehensive tests
cargo test --release --features vrw integration       # Integration tests
cargo test --release --features vrw regression        # Regression tests

# Run only library unit tests (fast, no I/O)
cargo test --lib

# Run library unit tests with vrw feature
cargo test --lib --features vrw

# Run with output
cargo test --release --features "vrc,vrw" -- --nocapture

# Check formatting
cargo fmt -- --check

# Lint (with vrw feature to cover all code paths)
cargo clippy --release --features "vrc,vrw"
```

## Feature Flags and Binaries

The `vrc` feature is the default. It compiles the `vrc` binary and the `vrc_core` library without HTTP server dependencies. The `vrw` feature additionally pulls in axum, reqwest, rustls, and other server dependencies, and compiles the `vrw` binary.

```
[features]
default = ["vrc"]
vrc = []
vrw = [ "dep:axum", "dep:axum-server", "dep:tower", ... ]
```

### Which feature flag to use?

- **Working on VTTY / emulator / renderer**: `cargo test --release` (vrc is default, sufficient)
- **Working on config schema, server, or HTTP API**: `cargo test --release --features vrw`
- **CI / pre-merge checks**: `cargo test --release --features "vrc,vrw"`

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
cargo test --release --features "vrc,vrw" renderer
```

#### Cell Tests (`src/vtty/cell.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_cell_default` | Default cell has space character, default colors, no decorations |
| `test_cell_new` | `Cell::new('X')` sets character, uses default colors |
| `test_cell_clear` | `Cell::clear()` resets to default state |
| `test_cell_is_empty` | `Cell::default().is_empty()` returns true; modified cell returns false |

### 2. Comprehensive Tests (`tests/comprehensive_test.rs`)

Multi-module tests covering the `vrc_core` library. These tests are organized by module and each is independent with no external state.

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
- **vrw only** (`#[cfg(feature = "vrw")]`): `config_default_values`, `config_deserialize_minimal_json`, `config_deserialize_full_json`, `config_serialize_roundtrip`, `config_merge_local_overrides_global`, `config_apply_profile_*`, `config_partial_config_all_none`, `validation_port_zero_is_error`, `validation_bind_empty_is_error`, `validation_multiple_issues`

```bash
# Run all comprehensive tests (both features recommended for full coverage)
cargo test --release --features "vrc,vrw" comprehensive
```

### 3. Integration Tests (`tests/integration_test.rs`)

Tests that exercise multi-component interactions and the `CommandManager`.

| Test | Feature requirement | What it verifies |
|------|-------------------|-------------------|
| `test_key_encoding` | Both | `encode_keys` correctly translates named keys (`<C-c>`, `<Enter>`, `<Up>`, etc.) |
| `test_spawn_and_list` | vrw | Spawning a command and listing it via `CommandManager` |
| `test_vtty_contents` | vrw | VTTY plain-text output after command execution |
| `test_send_keys` | vrw | Sending keystrokes to a running command's stdin |

Tests that use `CommandManager::new(Config { server, security, tls, vtty, ... })` require the `vrw` feature because those `Config` fields are only compiled when the feature is enabled.

```bash
# Integration tests require vrw feature for Config fields
cargo test --release --features vrw integration
```

### 4. Regression Tests (`tests/regression_test.rs`)

Tests that prevent re-introduction of known bugs. Added when a bug is discovered and fixed.

| Section | Feature requirement | What it verifies |
|---------|-------------------|-------------------|
| 1. Command lifecycle | vrw | spawn, list, kill, purge, find_by_pid, kill_by_pid |
| 2. IPC simulation | vrw | HTTP client timeout behavior |
| 3. Exit behavior | Mixed | ExitConfig defaults and serialization (both); process removal on exit (vrw) |
| 4. Multi-command management | vrw | Spawn after kill, concurrent spawns, kill-one-preserves-others |
| 5. VTTY operations | vrw | Snapshot, HTML, plain text, resize, diff, has_changed |
| 6. Key encoding | Mixed | send_keys delivery (vrw); encode_keys correctness (both) |
| 7. Config and CLI overrides | Mixed | Full Config validation/roundtrip (vrw); ExitConfig serialization (both) |
| 8. Instance registry | vrw | InstanceRegistry creation |
| 9. Broadcast / shutdown signals | Both | tokio broadcast channel propagation, watch channel semantics |
| 10. Edge cases | vrw | Command-not-found errors, env vars, custom VTTY size, logger |

**Tests that work with both features** (no `#[cfg(feature = "vrw")]` gate):

- `regression_exit_config_default_no_retain`, `regression_exit_config_retain_true`, `regression_exit_config_snapshot_on_exit`
- `regression_encode_all_special_keys`, `regression_encode_plain_text_unchanged`
- `regression_exit_config_serialize`
- `regression_shutdown_signal_propagates`, `regression_watch_channel_stores_value`, `regression_watch_channel_already_changed`

```bash
# Run all regression tests (vrw feature recommended for full coverage)
cargo test --release --features vrw regression
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

#### Web Dashboard Smoke Test (vrw only)

After starting vrw with a command, verify the following in the web dashboard:

1. **Initial load**: Open the admin page. The terminal should appear within 200ms (measured with browser DevTools Network tab — look for the `/api/snapshot` response time plus rendering time).
2. **Text alignment**: Terminal output should have proper column alignment. Text in adjacent columns should be vertically aligned, not shifted.
3. **Sidebar detail row**: Each command in the sidebar should show a second line with runtime, CPU%, MEM, and PID. These values should update every ~2 seconds.
4. **Refresh throttle**: Click the `↻` widget's `+` button several times. The value should increase in 100ms steps. Terminal updates should become noticeably less frequent at higher values. Click `-` to return to `off`.
5. **Kill all**: When no commands are running, the filter input and Kill All button should be hidden from the sidebar.
6. **Screenshot**: Click the screenshot button (📷). The downloaded file should be named `vrw_YYYYMMDD_HHMMSS_rowsxcols_command.png`.

#### CLI Screenshot Test

```bash
# vrc binary (UDS IPC)
vrc -- htop
vrc screenshot
# Expected: prints absolute path like /home/user/vrc_YYYYMMDD_HHMMSS_80x24_htop.png

# vrw binary (HTTP server)
vrw -- htop
vrw screenshot
# Expected: prints absolute path like /home/user/vrw_YYYYMMDD_HHMMSS_80x24_htop.png

# Custom output path (--output suppresses stdout path)
vrc screenshot --output /tmp/custom.png htop
```

#### VTTY Rendering Tests

These are verified visually by running commands with known output patterns:

```bash
# Color test — should show 256-color palette with correct alignment
vrc -- -- bash -c "for c in {0..255}; do echo -en \"\e[38;5;${c}m█\e[0m\"; done; echo"

# Box drawing — should render continuous lines, no gaps
vrc -- -- bash -c "echo '┌────┬────┐'; echo '│ ab │ cd │'; echo '├────┼────┤'; echo '│ ef │ gh │'; echo '└────┴────┘'"

# Wide characters — CJK characters should occupy exactly 2 columns
vrc -- -- bash -c "echo '你好世界 Hello World'"

# Scrolling — rapid output should display smoothly without visible lag
vrc -- -- bash -c "for i in $(seq 1 1000); do echo \"Line $i: $(head -c 60 /dev/urandom | base64)\"; done"
```

### 7. Web UI JavaScript Tests (`static/admin/test/`)

The web admin interface has a custom zero-dependency JavaScript test framework with mock DOM. Tests are located in `static/admin/test/` and are HTML files that run in a browser. Each test file loads the application modules and exercises UI components such as the sidebar, search, notifications, logs, onboarding, and spawn dialogs. To run: open the test HTML files in a browser or use a headless browser. Tests use a simple assertion framework with `assert()` and `assertEqual()` helpers.

### 8. VTTY Integration Tests

Integration tests that exercise the full VTTY pipeline: spawning a real PTY, writing data through the PTY, reading it back through the VTTY emulator, and verifying the terminal state. These tests validate the VT100 parser, scrollback buffer, cursor movement, ANSI color rendering, and terminal resize behavior under realistic conditions. They are part of the standard `cargo test` suite.

### 9. Cookbook Test Scripts

End-to-end scenario tests that verify complete user workflows. These scripts typically start a vrc/vrw instance, spawn commands, interact with them via the CLI or API, and assert on the outcomes. Cookbook tests are run manually and documented in the project's cookbook. They serve as acceptance tests that validate the system behaves correctly from a user's perspective.

## Common Issues

### Test fails with "No running commands" or "connection refused"

The integration tests that require a running vrw instance should be run after starting vrw. Most unit tests do not require this.

### Tests timeout

If tests hang, check for leftover processes:

```bash
pkill -f vrw
pkill -f vrc
cargo test --release --features "vrc,vrw"
```

### Benchmark numbers vary wildly

Benchmarks are sensitive to CPU frequency scaling, thermal throttling, and system load. Run benchmarks multiple times and compare medians. The `--release` profile is essential for meaningful measurements.

### "Config has no field `server`" compile error

Some tests and code paths require the `vrw` feature. If you see errors about missing fields on `Config`, ensure you're building with the correct feature:

```bash
cargo test --features vrw
```
