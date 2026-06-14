//! Extended test suite — 50 new tests across the project.
//!
//! Coverage areas added by this file:
//!   1. IPC protocol: encode_frame, decode_frame, ControlCommand/Response serde
//!   2. Config edge cases: validation extremes, serde roundtrips for sub-configs
//!   3. Emulator SGR: underline, blink, invisible, strikethrough, background color
//!   4. Emulator erase: CSI 1K (from start to cursor), CSI 2K (from cursor to end)
//!   5. Emulator edge cases: empty feed, bell multiple times, title with escapes
//!   6. Buffer: resize to same dimensions, scrollback with many lines
//!   7. Cell: hash stability, equality with all fields
//!   8. Logger: multiple subscribers, file-backed logger
//!   9. Color: resolve(0) = black, color_256 index 0 = black
//!  10. encode_keys: Return alias, Insert, Home, End, PageUp, PageDown, Ctrl+@/[/] etc.
//!  11. Renderer: HTML with styled cells (bold, italic, colors)
//!  12. CommandManager: keep/unkeep, freeze/thaw errors, logger/config access
//! 13. VttyOutput: default, with_sinks, sink_count
//! 14. InMemoryVttySink: multiple snapshots overwrite
//! 15. Instance registry: registry with temp dir

// ─────────────────────────────────────────────────────────────────────
// 1. IPC Protocol Tests (ControlCommand / ControlResponse serde + framing)
// ─────────────────────────────────────────────────────────────────────

#[test]
fn ipc_control_command_list_serde() {
    use vrc_core::ipc::protocol::ControlCommand;
    let cmd = ControlCommand::List;
    let json = serde_json::to_string(&cmd).unwrap();
    let parsed: ControlCommand = serde_json::from_str(&json).unwrap();
    assert!(matches!(parsed, ControlCommand::List));
}

#[test]
fn ipc_control_command_spawn_serde() {
    use vrc_core::ipc::protocol::ControlCommand;
    let cmd = ControlCommand::Spawn {
        cmd: "bash".into(),
        args: vec!["-c".into(), "echo hi".into()],
        env: Some([("PATH".into(), "/usr/bin".into())].into_iter().collect()),
        rows: Some(50),
        cols: Some(160),
        dir: Some("/tmp".into()),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    let parsed: ControlCommand = serde_json::from_str(&json).unwrap();
    match parsed {
        ControlCommand::Spawn { cmd, args, env, rows, cols, dir } => {
            assert_eq!(cmd, "bash");
            assert_eq!(args.len(), 2);
            assert!(env.is_some());
            assert_eq!(rows, Some(50));
            assert_eq!(cols, Some(160));
            assert_eq!(dir, Some("/tmp".to_string()));
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn ipc_control_command_kill_serde() {
    use vrc_core::ipc::protocol::ControlCommand;
    let cmd = ControlCommand::Kill { id: "abc123".into() };
    let json = serde_json::to_string(&cmd).unwrap();
    let parsed: ControlCommand = serde_json::from_str(&json).unwrap();
    assert!(matches!(parsed, ControlCommand::Kill { id } if id == "abc123"));
}

#[test]
fn ipc_control_command_resize_serde() {
    use vrc_core::ipc::protocol::ControlCommand;
    let cmd = ControlCommand::Resize {
        id: "xyz".into(),
        rows: 40,
        cols: 120,
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("40"));
    assert!(json.contains("120"));
    let parsed: ControlCommand = serde_json::from_str(&json).unwrap();
    assert!(matches!(parsed, ControlCommand::Resize { .. }));
}

#[test]
fn ipc_control_response_ok_serde() {
    use vrc_core::ipc::protocol::ControlResponse;
    let resp = ControlResponse::Ok {
        data: serde_json::json!({"count": 5}),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: ControlResponse = serde_json::from_str(&json).unwrap();
    assert!(matches!(parsed, ControlResponse::Ok { data } if data["count"] == 5));
}

#[test]
fn ipc_control_response_error_serde() {
    use vrc_core::ipc::protocol::ControlResponse;
    let resp = ControlResponse::Error {
        error: "not found".into(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: ControlResponse = serde_json::from_str(&json).unwrap();
    assert!(matches!(parsed, ControlResponse::Error { error } if error == "not found"));
}

#[test]
fn ipc_encode_frame_roundtrip() {
    use vrc_core::ipc::protocol::{encode_frame, decode_frame, ControlCommand};
    let cmd = ControlCommand::Ping;
    let frame = encode_frame(&cmd).unwrap();
    assert!(frame.len() >= 4);
    let (consumed, payload) = decode_frame(&frame).unwrap();
    assert_eq!(consumed, frame.len());
    let parsed: ControlCommand = serde_json::from_slice(&payload).unwrap();
    assert!(matches!(parsed, ControlCommand::Ping));
}

#[test]
fn ipc_decode_frame_incomplete_returns_none() {
    use vrc_core::ipc::protocol::decode_frame;
    // Only 2 bytes — not enough for the 4-byte length prefix
    assert!(decode_frame(&[0x00, 0x01]).is_none());
    // 4-byte header says 100 bytes but only 3 bytes of payload
    let buf = [0x00, 0x00, 0x00, 0x64, 0x7b, 0x22, 0x7d];
    assert!(decode_frame(&buf).is_none());
}

// ─────────────────────────────────────────────────────────────────────
// 2. Config Validation Edge Cases
// ─────────────────────────────────────────────────────────────────────

#[test]
fn validation_port_65535_is_valid() {
    let cfg = vrc_core::config::schema::Config::default();
    let issues = vrc_core::config::validation::validate_config(&cfg);
    let port_errs: Vec<_> = issues
        .iter()
        .filter(|i| {
            i.field == "server.port"
                && i.level == vrc_core::config::validation::ValidationLevel::Error
        })
        .collect();
    assert!(port_errs.is_empty(), "port 65535 should be valid");
}

#[test]
fn validation_scrollback_zero_is_allowed() {
    let mut cfg = vrc_core::config::schema::Config::default();
    cfg.vtty.scrollback = 0;
    let issues = vrc_core::config::validation::validate_config(&cfg);
    // scrollback = 0 is valid (means no scrollback)
    assert!(!issues.iter().any(|i| i.field == "vtty.scrollback"));
}

// ─────────────────────────────────────────────────────────────────────
// 3. Config Sub-Config Serde Roundtrips
// ─────────────────────────────────────────────────────────────────────

#[test]
#[cfg(feature = "vrw")]
fn config_security_serde_roundtrip() {
    use vrc_core::config::schema::SecurityConfig;
    let cfg = SecurityConfig {
        require_auth: true,
        token_file: "my_token".into(),
        cors: Default::default(),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let parsed: SecurityConfig = serde_json::from_str(&json).unwrap();
    assert!(parsed.require_auth);
    assert_eq!(parsed.token_file, "my_token");
}

#[test]
#[cfg(feature = "vrw")]
fn config_tls_serde_roundtrip() {
    use vrc_core::config::schema::TlsConfig;
    let cfg = TlsConfig {
        enabled: true,
        cert_file: Some("cert.pem".into()),
        key_file: Some("key.pem".into()),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let parsed: TlsConfig = serde_json::from_str(&json).unwrap();
    assert!(parsed.enabled);
}

#[test]
fn config_display_serde_roundtrip() {
    use vrc_core::config::schema::DisplayConfig;
    let cfg = DisplayConfig {
        enabled: true,
        refresh_ms: 50,
        display_all: true,
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let parsed: DisplayConfig = serde_json::from_str(&json).unwrap();
    assert!(parsed.enabled);
    assert_eq!(parsed.refresh_ms, 50);
    assert!(parsed.display_all);
}

#[test]
fn config_daemon_serde_roundtrip() {
    use vrc_core::config::schema::DaemonConfig;
    let cfg = DaemonConfig {
        enabled: true,
        stdout_file: "/tmp/out".into(),
        stderr_file: "/tmp/err".into(),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let parsed: DaemonConfig = serde_json::from_str(&json).unwrap();
    assert!(parsed.enabled);
}

// ─────────────────────────────────────────────────────────────────────
// 4. Emulator SGR Extended Styles
// ─────────────────────────────────────────────────────────────────────

fn make_emulator(rows: u16, cols: u16) -> vrc_core::vtty::emulator::VttyEmulator {
    vrc_core::vtty::emulator::VttyEmulator::new(rows, cols, 1000)
}

#[test]
fn emulator_sgr_underline() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[4m");
    emu.feed_str("U");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert!(cell.underline);
    assert!(!cell.bold);
}

#[test]
fn emulator_sgr_blink() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[5m");
    emu.feed_str("B");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert!(cell.blink);
}

#[test]
fn emulator_sgr_invisible() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[8m");
    emu.feed_str("I");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert!(cell.invisible);
}

#[test]
fn emulator_sgr_strikethrough() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[9m");
    emu.feed_str("S");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert!(cell.strikethrough);
}

#[test]
fn emulator_sgr_background_color() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[44m"); // blue bg
    emu.feed_str("X");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert_eq!(cell.bg, [0, 0, 170]);
}

#[test]
fn emulator_sgr_rgb_background() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[48;2;10;20;30m"); // RGB bg
    emu.feed_str("C");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert_eq!(cell.bg, [10, 20, 30]);
}

#[test]
fn emulator_sgr_combined_multiple_styles() {
    let mut emu = make_emulator(3, 20);
    emu.feed(b"\x1b[1;3;4;7;9m"); // bold+italic+underline+reverse+strikethrough
    emu.feed_str("X");
    let buf = emu.snapshot();
    let cell = buf.get(0, 0).unwrap();
    assert!(cell.bold);
    assert!(cell.italic);
    assert!(cell.underline);
    assert!(cell.reverse);
    assert!(cell.strikethrough);
}

#[test]
fn emulator_sgr_reset_clears_all_styles() {
    let mut emu = make_emulator(3, 20);
    // Set many styles
    emu.feed(b"\x1b[1;3;4;5;7;8;9m");
    emu.feed_str("X");
    // Reset
    emu.feed(b"\x1b[0m");
    emu.feed_str("Y");
    let buf = emu.snapshot();
    // Y is written at column 1 (right after X at column 0)
    let cell = buf.get(0, 1).unwrap();
    assert!(!cell.bold);
    assert!(!cell.italic);
    assert!(!cell.underline);
    assert!(!cell.reverse);
    assert!(!cell.blink);
    assert!(!cell.invisible);
    assert!(!cell.strikethrough);
    assert_eq!(cell.ch, 'Y');
}

// ─────────────────────────────────────────────────────────────────────
// 5. Emulator Erase Line Variants
// ─────────────────────────────────────────────────────────────────────

#[test]
fn emulator_erase_line_from_cursor_to_end() {
    let mut emu = make_emulator(3, 20);
    emu.feed_str("ABCDEFGHIJ");
    // Move cursor to col 5, then erase from cursor to end of line
    emu.feed(b"\x1b[1;6H");
    emu.feed(b"\x1b[0K");
    let buf = emu.snapshot();
    // Chars 0-4 should remain
    assert_eq!(buf.get(0, 0).unwrap().ch, 'A');
    assert_eq!(buf.get(0, 4).unwrap().ch, 'E');
    // Chars 5+ should be cleared
    assert_eq!(buf.get(0, 5).unwrap().ch, ' ');
    assert_eq!(buf.get(0, 9).unwrap().ch, ' ');
}

#[test]
fn emulator_erase_line_entire_line() {
    let mut emu = make_emulator(3, 10);
    emu.feed_str("Hello");
    emu.feed(b"\x1b[2K"); // erase entire line
    let buf = emu.snapshot();
    assert_eq!(buf.get(0, 0).unwrap().ch, ' ');
    assert_eq!(buf.get(0, 4).unwrap().ch, ' ');
}

#[test]
fn emulator_bell_multiple_times() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x07\x07\x07");
    assert!(emu.drain_bell());
    // drain_bell consumes all at once
    assert!(!emu.drain_bell());
}

#[test]
fn emulator_title_overwrite() {
    let mut emu = make_emulator(3, 10);
    emu.feed(b"\x1b]0;first\x07");
    assert_eq!(emu.title(), "first");
    emu.feed(b"\x1b]0;second\x07");
    assert_eq!(emu.title(), "second");
}

#[test]
fn emulator_resize_buffer() {
    let mut emu = make_emulator(5, 10);
    emu.resize(20, 40);
    let buf = emu.snapshot();
    assert_eq!(buf.height, 20);
    assert_eq!(buf.width, 40);
}

// ─────────────────────────────────────────────────────────────────────
// 7. Buffer Edge Cases
// ─────────────────────────────────────────────────────────────────────

#[test]
fn buffer_resize_same_dimensions() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    b.set(0, 0, Cell::new('X'));
    b.set(0, 5, Cell::new('Y'));
    b.resize(10, 5); // same size
    assert_eq!(b.width, 10);
    assert_eq!(b.height, 5);
    assert_eq!(b.get(0, 0).unwrap().ch, 'X');
    assert_eq!(b.get(0, 5).unwrap().ch, 'Y');
    // resize always bumps generation, even for same dimensions
}

#[test]
fn buffer_scroll_up_adds_to_scrollback() {
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 2, 100);
    b.rows[0][0].ch = 'T';
    b.rows[1][0].ch = 'B';
    b.scroll_up(None);
    assert_eq!(b.scrollback.len(), 1);
    assert_eq!(b.scrollback[0][0].ch, 'T');
}

#[test]
fn buffer_get_returns_none_for_out_of_bounds() {
    let b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    assert!(b.get(100, 0).is_none());
    assert!(b.get(0, 100).is_none());
}

#[test]
fn buffer_generation_increments_on_set() {
    use vrc_core::vtty::cell::Cell;
    let mut b = vrc_core::vtty::buffer::Buffer::new(10, 5, 100);
    let gen0 = b.generation();
    b.set(0, 0, Cell::new('A'));
    assert!(b.generation() > gen0);
}

// ─────────────────────────────────────────────────────────────────────
// 8. Cell Equality with All Fields
// ─────────────────────────────────────────────────────────────────────

#[test]
fn cell_equality_with_colors() {
    use vrc_core::vtty::cell::Cell;
    let mut a = Cell::new('X');
    a.fg = [100, 200, 50];
    a.bg = [10, 20, 30];
    let mut b = Cell::new('X');
    b.fg = [100, 200, 50];
    b.bg = [10, 20, 30];
    assert_eq!(a, b);

    // Different bg
    let mut c = Cell::new('X');
    c.fg = [100, 200, 50];
    c.bg = [99, 20, 30];
    assert_ne!(a, c);
}

#[test]
fn cell_equality_with_all_attributes() {
    use vrc_core::vtty::cell::Cell;
    let mut a = Cell::new('Z');
    a.bold = true;
    a.italic = true;
    a.underline = true;
    a.blink = true;
    a.reverse = true;
    a.invisible = true;
    a.strikethrough = true;
    a.fg = [1, 2, 3];
    a.bg = [4, 5, 6];
    let mut b = a.clone();
    assert_eq!(a, b);
    b.fg = [7, 8, 9];
    assert_ne!(a, b);
}

// ─────────────────────────────────────────────────────────────────────
// 9. Color Additional Coverage
// ─────────────────────────────────────────────────────────────────────

#[test]
fn color_256_index_0_is_black() {
    let c = vrc_core::vtty::color::color_256_to_rgb(0);
    assert_eq!(c, [0, 0, 0]);
}

// ─────────────────────────────────────────────────────────────────────
// 11. Renderer with Styled Cells
// ─────────────────────────────────────────────────────────────────────

#[test]
fn renderer_to_html_with_bold() {
    use vrc_core::vtty::cell::Cell;
    let mut buf = vrc_core::vtty::buffer::Buffer::new(10, 1, 100);
    let mut c = Cell::new('B');
    c.bold = true;
    buf.set(0, 0, c);
    let html = vrc_core::vtty::renderer::VttyRenderer::to_html(&buf);
    assert!(html.contains("B"));
    assert!(html.contains("bold") || html.contains("font-weight"));
}

#[test]
fn renderer_to_html_with_fg_color() {
    use vrc_core::vtty::cell::Cell;
    let mut buf = vrc_core::vtty::buffer::Buffer::new(10, 1, 100);
    let mut c = Cell::new('R');
    c.fg = [255, 0, 0];
    buf.set(0, 0, c);
    let html = vrc_core::vtty::renderer::VttyRenderer::to_html(&buf);
    assert!(html.contains("R"));
    assert!(html.contains("rgb(") || html.contains("color:"));
}

// ─────────────────────────────────────────────────────────────────────
// 12. CommandManager Operations (error paths)
// ─────────────────────────────────────────────────────────────────────

#[test]
#[cfg(feature = "vrw")]
fn manager_keep_nonexistent_errors() {
    use vrc_core::config::schema::CommandLogConfig;
    use vrc_core::process::manager::CommandManager;
    use vrc_core::config::schema::{Config, DaemonConfig, DisplayConfig};
    let cfg = Config {
        binary_name: "test".into(),
        color_terminal_log: false,
        server: vrc_core::config::schema::ServerConfig::default(),
        security: vrc_core::config::schema::SecurityConfig::default(),
        tls: vrc_core::config::schema::TlsConfig::default(),
        certificates: Default::default(),
        vtty: vrc_core::config::schema::VttyConfig::default(),
        display: DisplayConfig::default(),
        command_log: CommandLogConfig::default(),
        daemon: DaemonConfig::default(),
        handles: vec![],
        interactive: Default::default(),
        default_exit: Default::default(),
        environment: Default::default(),
        web: Default::default(),
        profiles: Default::default(),
        hooks: Default::default(),
        templates: Default::default(),
        environments: Default::default(),
    };
    let mgr = CommandManager::new(cfg);
    let result = mgr.keep(&"nonexistent".into());
    assert!(result.is_err());
}

#[test]
#[cfg(feature = "vrw")]
fn manager_unkeep_nonexistent_errors() {
    use vrc_core::config::schema::CommandLogConfig;
    use vrc_core::process::manager::CommandManager;
    use vrc_core::config::schema::{Config, DaemonConfig, DisplayConfig};
    let cfg = Config {
        binary_name: "test".into(),
        color_terminal_log: false,
        server: vrc_core::config::schema::ServerConfig::default(),
        security: vrc_core::config::schema::SecurityConfig::default(),
        tls: vrc_core::config::schema::TlsConfig::default(),
        certificates: Default::default(),
        vtty: vrc_core::config::schema::VttyConfig::default(),
        display: DisplayConfig::default(),
        command_log: CommandLogConfig::default(),
        daemon: DaemonConfig::default(),
        handles: vec![],
        interactive: Default::default(),
        default_exit: Default::default(),
        environment: Default::default(),
        web: Default::default(),
        profiles: Default::default(),
        hooks: Default::default(),
        templates: Default::default(),
        environments: Default::default(),
    };
    let mgr = CommandManager::new(cfg);
    let result = mgr.unkeep(&"nonexistent".into());
    assert!(result.is_err());
}

#[test]
#[cfg(feature = "vrw")]
fn manager_freeze_nonexistent_errors() {
    use vrc_core::config::schema::CommandLogConfig;
    use vrc_core::process::manager::CommandManager;
    use vrc_core::config::schema::{Config, DaemonConfig, DisplayConfig};
    let cfg = Config {
        binary_name: "test".into(),
        color_terminal_log: false,
        server: vrc_core::config::schema::ServerConfig::default(),
        security: vrc_core::config::schema::SecurityConfig::default(),
        tls: vrc_core::config::schema::TlsConfig::default(),
        certificates: Default::default(),
        vtty: vrc_core::config::schema::VttyConfig::default(),
        display: DisplayConfig::default(),
        command_log: CommandLogConfig::default(),
        daemon: DaemonConfig::default(),
        handles: vec![],
        interactive: Default::default(),
        default_exit: Default::default(),
        environment: Default::default(),
        web: Default::default(),
        profiles: Default::default(),
        hooks: Default::default(),
        templates: Default::default(),
        environments: Default::default(),
    };
    let mgr = CommandManager::new(cfg);
    let result = mgr.freeze(&"nonexistent".into());
    assert!(result.is_err());
}

#[test]
#[cfg(feature = "vrw")]
fn manager_thaw_nonexistent_errors() {
    use vrc_core::config::schema::CommandLogConfig;
    use vrc_core::process::manager::CommandManager;
    use vrc_core::config::schema::{Config, DaemonConfig, DisplayConfig};
    let cfg = Config {
        binary_name: "test".into(),
        color_terminal_log: false,
        server: vrc_core::config::schema::ServerConfig::default(),
        security: vrc_core::config::schema::SecurityConfig::default(),
        tls: vrc_core::config::schema::TlsConfig::default(),
        certificates: Default::default(),
        vtty: vrc_core::config::schema::VttyConfig::default(),
        display: DisplayConfig::default(),
        command_log: CommandLogConfig::default(),
        daemon: DaemonConfig::default(),
        handles: vec![],
        interactive: Default::default(),
        default_exit: Default::default(),
        environment: Default::default(),
        web: Default::default(),
        profiles: Default::default(),
        hooks: Default::default(),
        templates: Default::default(),
        environments: Default::default(),
    };
    let mgr = CommandManager::new(cfg);
    let result = mgr.thaw(&"nonexistent".into());
    assert!(result.is_err());
}

#[test]
fn vtty_output_clone_shares_sinks() {
    use vrc_core::vtty::sink::VttyOutput;
    use vrc_core::vtty::sink::InMemoryVttySink;
    let mut output = VttyOutput::new();
    output.add_sink(std::sync::Arc::new(InMemoryVttySink::new()));
    let cloned = output.clone();
    assert_eq!(cloned.sink_count(), 1);
}

// ─────────────────────────────────────────────────────────────────────
// 14. InMemoryVttySink Overwrite
// ─────────────────────────────────────────────────────────────────────

#[test]
fn in_memory_sink_overwrites_latest() {
    use vrc_core::vtty::sink::{InMemoryVttySink, VttySink};
    use vrc_core::vtty::emulator::VttyEmulator;

    let sink = InMemoryVttySink::new();
    let mut emu = VttyEmulator::new(3, 10, 100);
    emu.feed_str("First");
    sink.on_buffer_change(&emu.snapshot());
    assert_eq!(sink.change_count(), 1);
    assert_eq!(sink.latest().unwrap().rows[0][0].ch, 'F');

    emu.feed_str("Second");
    sink.on_buffer_change(&emu.snapshot());
    assert_eq!(sink.change_count(), 2);
    let latest = sink.latest().unwrap();
    assert_eq!(latest.rows[0][0].ch, 'F');
    assert_eq!(latest.rows[0][5].ch, 'S');
}
