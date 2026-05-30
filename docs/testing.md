# Testing Procedures

This document describes how to run vrunner's test suites, what each suite covers, and how to verify correctness after making changes.

## Quick Reference

```bash
# Run all tests (release profile for benchmark accuracy)
cargo test --release

# Run a specific test suite
cargo test --release renderer          # VTTY renderer tests
cargo test --release diff               # Buffer diff benchmarks
cargo test --release diff_json          # Diff JSON serialization benchmarks
cargo test --release to_html           # HTML rendering correctness
cargo test --release to_png             # PNG rendering tests
cargo test --release comprehensive      # Comprehensive integration tests
cargo test --release regression         # Regression tests

# Run only unit tests (fast, no I/O)
cargo test --lib

# Run with output
cargo test --release -- --nocapture

# Check formatting
cargo fmt -- --check

# Lint
cargo clippy --release
```

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
cargo test --release renderer
```

#### Cell Tests (`src/vtty/cell.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_cell_default` | Default cell has space character, default colors, no decorations |
| `test_cell_new` | `Cell::new('X')` sets character, uses default colors |
| `test_cell_clear` | `Cell::clear()` resets to default state |
| `test_cell_is_empty` | `Cell::default().is_empty()` returns true; modified cell returns false |

#### Comprehensive Tests (`tests/comprehensive_test.rs`)

Integration-level tests that verify multi-component interactions:

| Test | What it verifies |
|------|-----------------|
| `renderer_to_html_basic` | Basic HTML rendering produces valid output with expected structure |
| `renderer_to_html_empty_buffer` | Empty buffer renders without errors |

#### Regression Tests (`tests/regression_test.rs`)

Tests that prevent re-introduction of known bugs. These are added when a bug is discovered and fixed.

### 2. Performance Benchmarks

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

### 3. Manual Testing Procedures

#### Web Dashboard Smoke Test

After starting vrunner with a command, verify the following in the web dashboard:

1. **Initial load**: Open the admin page. The terminal should appear within 200ms (measured with browser DevTools Network tab — look for the `/api/snapshot` response time plus rendering time).
2. **Text alignment**: Terminal output should have proper column alignment. Text in adjacent columns should be vertically aligned, not shifted.
3. **Sidebar detail row**: Each command in the sidebar should show a second line with runtime, CPU%, MEM, and PID. These values should update every ~2 seconds.
4. **Refresh throttle**: Click the `↻` widget's `+` button several times. The value should increase in 100ms steps. Terminal updates should become noticeably less frequent at higher values. Click `-` to return to `off`.
5. **Kill all**: When no commands are running, the filter input and Kill All button should be hidden from the sidebar.
6. **Screenshot**: Click the screenshot button (📷). The downloaded file should be named `vrunner_YYYYMMDD_HHMMSS_rowsxcols_command.png`.

#### CLI Screenshot Test

```bash
# Start vrunner with a command
vrunner run htop

# In another terminal:
vrunner screenshot
# Expected: prints absolute path like /home/user/vrunner_20260530_092208_80x24_htop.png

vrunner screenshot --output /tmp/custom.png htop
# Expected: /tmp/custom.png (no stdout path printed when --output is given)
```

#### VTTY Rendering Tests

These are verified visually by running commands with known output patterns:

```bash
# Color test — should show 256-color palette with correct alignment
vrunner run -- bash -c "for c in {0..255}; do echo -en \"\e[38;5;${c}m█\e[0m\"; done; echo"

# Box drawing — should render continuous lines, no gaps
vrunner run -- bash -c "echo '┌────┬────┐'; echo '│ ab │ cd │'; echo '├────┼────┤'; echo '│ ef │ gh │'; echo '└────┴────┘'"

# Wide characters — CJK characters should occupy exactly 2 columns
vrunner run -- bash -c "echo '你好世界 Hello World'"

# Scrolling — rapid output should display smoothly without visible lag
vrunner run -- bash -c "for i in $(seq 1 1000); do echo \"Line $i: $(head -c 60 /dev/urandom | base64)\"; done"
```

## Common Issues

### Test fails with "No running commands" or "connection refused"

The integration tests that require a running vrunner instance should be run after starting vrunner. Most unit tests do not require this.

### Tests timeout

If tests hang, check for leftover vrunner processes:

```bash
pkill -f vrunner
cargo test --release
```

### Benchmark numbers vary wildly

Benchmarks are sensitive to CPU frequency scaling, thermal throttling, and system load. Run benchmarks multiple times and compare medians. The `--release` profile is essential for meaningful measurements.
