use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

// ── display ──

/// Local terminal display settings (mprocs-style VTTY rendering).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// Show VTTY output on the local terminal. Removed when the CLI exits
    /// unless `display_all` is also enabled.
    pub enabled: bool,
    /// Refresh interval in milliseconds when display is enabled.
    pub refresh_ms: u64,
    /// Keep the display active after the initial CLI command exits,
    /// switching to the next available command. When disabled (default),
    /// the display is dismissed and a status message is printed.
    pub display_all: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            refresh_ms: 100,
            display_all: false,
        }
    }
}

/// Configuration for interactive terminal display (tab bar, keyboard).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct InteractiveConfig {
    /// Show a tab bar listing all commands at the top of the display.
    #[serde(default)]
    pub tabs: bool,
    /// Configurable keybindings for the terminal display.
    /// Maps action names to key sequences. When a key matches,
    /// the corresponding action is executed instead of forwarding
    /// the keystroke to the active command.
    ///
    /// Key format: human-readable names (`ctrl+left`, `f12`, `esc`).
    /// Raw escape sequences (`"\x1b[1;5C"`) also accepted for compatibility.
    ///
    /// Actions: `next_command`, `prev_command`, `toggle_log`,
    /// `spawn_command`, `show_help`, `kill_command`, `toggle_pause`, `quit`.
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

/// Maps action names to key sequences for the interactive terminal display.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeybindingsConfig {
    /// Switch to next command. Default: `ctrl+right`
    #[serde(default = "default_key_next_command")]
    pub next_command: Option<String>,
    /// Switch to previous command. Default: `ctrl+left`
    #[serde(default = "default_key_prev_command")]
    pub prev_command: Option<String>,
    /// Toggle command log overlay. Default: `ctrl+l`
    #[serde(default = "default_key_toggle_log")]
    pub toggle_log: Option<String>,
    /// Open a prompt to spawn a new command. Default: `f12`
    #[serde(default = "default_key_spawn_command")]
    pub spawn_command: Option<String>,
    /// Show the help overlay. Default: `ctrl+h`
    #[serde(default = "default_key_show_help")]
    pub show_help: Option<String>,
    /// Kill (SIGTERM) the active command. Default: none
    #[serde(default)]
    pub kill_command: Option<String>,
    /// Pause/resume (SIGSTOP/SIGCONT) the active command. Default: none
    #[serde(default)]
    pub toggle_pause: Option<String>,
    /// Quit the display loop. Default: none (use Ctrl+\)
    #[serde(default)]
    pub quit: Option<String>,
}

fn default_key_next_command() -> Option<String> { Some("ctrl+right".into()) }
fn default_key_prev_command() -> Option<String> { Some("ctrl+left".into()) }
fn default_key_toggle_log() -> Option<String> { Some("ctrl+l".into()) }
fn default_key_spawn_command() -> Option<String> { Some("f12".into()) }
fn default_key_show_help() -> Option<String> { Some("ctrl+h".into()) }

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next_command: default_key_next_command(),
            prev_command: default_key_prev_command(),
            toggle_log: default_key_toggle_log(),
            spawn_command: default_key_spawn_command(),
            show_help: default_key_show_help(),
            kill_command: None,
            toggle_pause: None,
            quit: None,
        }
    }
}

// ── hooks ──

/// Event hooks: shell commands triggered on process lifecycle events.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HooksConfig {
    /// Command to run when ANY child is spawned. Placeholders: {name}, {id}, {pid}
    #[serde(default)]
    pub on_spawn: Option<String>,
    /// Command to run when ANY child exits cleanly (code 0). Placeholders: {name}, {id}, {pid}, {exit_code}
    #[serde(default)]
    pub on_exit: Option<String>,
    /// Command to run when ANY child exits with non-zero code. Placeholders: {name}, {id}, {pid}, {exit_code}
    #[serde(default)]
    pub on_error: Option<String>,
    /// Command to run when ANY child is killed. Placeholders: {name}, {id}, {pid}
    #[serde(default)]
    pub on_kill: Option<String>,
}

/// Exit configuration for a command (cleanup commands, timeouts).
/// Set per-command via spawn API or as defaults in the config.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExitConfig {
    /// Command to run on clean exit (code 0). Split on whitespace into binary + args.
    #[serde(default)]
    pub on_exit: Option<String>,
    /// Command to run on error exit (non-zero code). Split on whitespace into binary + args.
    #[serde(default)]
    pub on_error: Option<String>,
    /// Max seconds to wait after SIGTERM before SIGKILL. Default: 10.
    #[serde(default = "default_exit_timeout")]
    pub timeout_secs: u64,
    /// Retain VTTY buffer after exit so the command stays inspectable in the
    /// display and web UI. Default: false (removed from manager on exit).
    #[serde(default)]
    pub retain_on_exit: bool,
    /// Save VTTY buffer to this file path on exit (per-command option).
    /// Includes scrollback + visible rows, each line trimmed.
    #[serde(default)]
    pub snapshot_on_exit: Option<String>,
}

fn default_exit_timeout() -> u64 { 10 }

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            on_exit: None,
            on_error: None,
            timeout_secs: default_exit_timeout(),
            retain_on_exit: false,
            snapshot_on_exit: None,
        }
    }
}

/// Default exit configuration (used when none is specified per-command).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DefaultExitConfig {
    /// Exit behavior applied to every command unless overridden.
    #[serde(default)]
    pub exit: ExitConfig,
}

/// ANSI SGR escape sequence for a log field. Empty string = no color.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ColorField {
    #[serde(default)]
    pub ansi: String,
}

/// Terminal log appearance: format string, per-field colors and padding.
/// Placeholders: %timestamp% %pid% %id% %cmd% %event% %details%
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TerminalLogConfig {
    /// Printf-like format string controlling log line layout.
    /// Placeholders: %timestamp% (HH:MM:SS.cc) %pid% %id% (UUID first 8) %cmd% %event% %details%
    /// Default: "%timestamp% %pid% %cmd% %event% %details%"
    #[serde(default = "default_terminal_format")]
    pub format: String,
    /// Per-field ANSI color configuration.
    #[serde(default)]
    pub colors: TerminalLogColors,
    /// Per-field padding widths (right-aligned, truncated if longer).
    #[serde(default)]
    pub pad: TerminalLogPad,
}

fn default_terminal_format() -> String {
    "%timestamp% %pid% %cmd% %event% %details%".to_string()
}

impl Default for TerminalLogConfig {
    fn default() -> Self {
        Self {
            format: default_terminal_format(),
            colors: TerminalLogColors::default(),
            pad: TerminalLogPad::default(),
        }
    }
}

/// Per-field ANSI colors. Each value is a `ColorField` with an `ansi` escape string.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TerminalLogColors {
    #[serde(default = "default_clr_timestamp")]
    pub timestamp: ColorField,
    #[serde(default = "default_clr_pid")]
    pub pid: ColorField,
    #[serde(default = "default_clr_id")]
    pub id: ColorField,
    #[serde(default = "default_clr_cmd")]
    pub cmd: ColorField,
    #[serde(default = "default_clr_event")]
    pub event: ColorField,
    #[serde(default = "default_clr_arg")]
    pub arg: ColorField,
    #[serde(default = "default_clr_cert")]
    pub cert: ColorField,
    #[serde(default = "default_clr_env")]
    pub env: ColorField,
    #[serde(default = "default_clr_size")]
    pub size: ColorField,
    #[serde(default = "default_clr_dir")]
    pub dir: ColorField,
    #[serde(default = "default_clr_detail")]
    pub detail: ColorField,
}

fn default_clr(field: &str) -> ColorField {
    let ansi = match field {
        "timestamp" | "detail" => "\x1b[90m",
        "pid" => "\x1b[1;37m",
        "id" | "cmd" | "event" | "arg" | "env" => "\x1b[32m",
        "cert" | "dir" => "\x1b[34m",
        "size" => "\x1b[1;33m",
        _ => "\x1b[0m",
    };
    ColorField { ansi: ansi.to_string() }
}

macro_rules! clr_default { ($f:ident, $k:expr) => { fn $f() -> ColorField { default_clr($k) } } }
clr_default!(default_clr_timestamp, "timestamp");
clr_default!(default_clr_pid, "pid");
clr_default!(default_clr_id, "id");
clr_default!(default_clr_cmd, "cmd");
clr_default!(default_clr_event, "event");
clr_default!(default_clr_arg, "arg");
clr_default!(default_clr_cert, "cert");
clr_default!(default_clr_env, "env");
clr_default!(default_clr_size, "size");
clr_default!(default_clr_dir, "dir");
clr_default!(default_clr_detail, "detail");

impl Default for TerminalLogColors {
    fn default() -> Self {
        Self {
            timestamp: default_clr("timestamp"),
            pid: default_clr("pid"),
            id: default_clr("id"),
            cmd: default_clr("cmd"),
            event: default_clr("event"),
            arg: default_clr("arg"),
            cert: default_clr("cert"),
            env: default_clr("env"),
            size: default_clr("size"),
            dir: default_clr("dir"),
            detail: default_clr("detail"),
        }
    }
}

/// Per-field padding widths (right-aligned, truncated if longer).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TerminalLogPad {
    #[serde(default = "default_pad_pid")]
    pub pid: usize,
    #[serde(default = "default_pad_cmd")]
    pub cmd: usize,
    #[serde(default = "default_pad_event")]
    pub event: usize,
}

fn default_pad_pid() -> usize { 6 }
fn default_pad_cmd() -> usize { 16 }
fn default_pad_event() -> usize { 17 }

impl Default for TerminalLogPad {
    fn default() -> Self {
        Self {
            pid: default_pad_pid(),
            cmd: default_pad_cmd(),
            event: default_pad_event(),
        }
    }
}

/// Command logging configuration.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CommandLogConfig {
    /// Enable logging of API commands.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the command log file.
    #[serde(default)]
    pub file: Option<String>,
    /// File path for raw PTY output log. Each line is one `read()` call
    /// with elapsed time and escaped bytes (non-printable → \xHH).
    #[serde(default)]
    pub pty_raw_log: Option<String>,
    /// Terminal log appearance and format configuration.
    #[serde(default)]
    pub terminal: TerminalLogConfig,
}

// ── templates ──

/// A pre-defined command template (appears in the web UI Templates sidebar).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemplateConfig {
    /// Display name shown in the Templates panel.
    pub name: String,
    /// Command executable to run.
    pub cmd: String,
    /// Space-separated arguments. Omit for none.
    #[serde(default)]
    pub args: Option<String>,
    /// Environment variables (KEY=VALUE strings). Override global `[environment]`
    /// defaults but can be overridden per-spawn via the API `env` field.
    #[serde(default)]
    pub env: Option<Vec<String>>,
    /// Working directory. Defaults to vrc's own working directory.
    #[serde(default)]
    pub workdir: Option<String>,
    /// Certificate name to bind (from the `[certificates]` section).
    #[serde(default)]
    pub certificate: Option<String>,
    /// VTTY rows. Defaults to the global `[vtty].rows` setting.
    #[serde(default)]
    pub rows: Option<u16>,
    /// VTTY columns. Defaults to the global `[vtty].cols` setting.
    #[serde(default)]
    pub cols: Option<u16>,
}

/// The templates section of the configuration (`[[templates]]` entries).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TemplatesConfig(pub Vec<TemplateConfig>);

impl Deref for TemplatesConfig {
    type Target = Vec<TemplateConfig>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for TemplatesConfig {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

// ── environments (workspace) ──

/// A command to spawn within an environment panel. Commands are spawned
/// sequentially; the first command's VTTY is displayed in the panel.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentCommand {
    /// Command executable to run.
    pub cmd: String,
    /// Space-separated arguments. Omit for none.
    #[serde(default)]
    pub args: Option<String>,
    /// Working directory. Defaults to the server's working directory.
    #[serde(default)]
    pub workdir: Option<String>,
    /// Certificate name to bind (from the server's certificates).
    #[serde(default)]
    pub certificate: Option<String>,
    /// VTTY rows. Defaults to the global `[vtty].rows` setting.
    #[serde(default)]
    pub rows: Option<u16>,
    /// VTTY columns. Defaults to the global `[vtty].cols` setting.
    #[serde(default)]
    pub cols: Option<u16>,
    /// Retain terminal buffer after the command exits.
    #[serde(default)]
    pub retain_on_exit: Option<bool>,
}

/// A panel within an environment, optionally connected to a server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentPanel {
    /// Panel title displayed in the header. Defaults to command name or "Panel N".
    #[serde(default)]
    pub title: Option<String>,
    /// Server URL. Defaults to the environment's `default_server` or the primary instance.
    #[serde(default)]
    pub server: Option<String>,
    /// Auth token for the server (if different from the global token).
    #[serde(default)]
    pub token: Option<String>,
    /// Label for the server connection (displayed in sidebar).
    #[serde(default)]
    pub server_label: Option<String>,
    /// Commands to spawn in this panel. Empty = no running command.
    #[serde(default)]
    pub commands: Vec<EnvironmentCommand>,
}

/// A workspace environment: panels with servers and commands for quick context switching.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceEnvironment {
    /// Unique name for display and CLI selection.
    pub name: String,
    /// Description of what this environment is for.
    #[serde(default)]
    pub description: Option<String>,
    /// Panel layout: "horizontal" (side-by-side) or "vertical" (stacked).
    #[serde(default)]
    pub layout: Option<String>,
    /// Auto-spawn all commands when the server loads this environment.
    #[serde(default)]
    pub auto_start: Option<bool>,
    /// Default server URL for panels without their own.
    #[serde(default)]
    pub default_server: Option<String>,
    /// Default auth token for panels without their own.
    #[serde(default)]
    pub default_token: Option<String>,
    /// The panels that make up this environment.
    #[serde(default)]
    pub panels: Vec<EnvironmentPanel>,
}

/// The environments section of the configuration (`[[environments]]` entries).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnvironmentsConfig(pub Vec<WorkspaceEnvironment>);

impl Deref for EnvironmentsConfig {
    type Target = Vec<WorkspaceEnvironment>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for EnvironmentsConfig {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl EnvironmentsConfig {
    /// Find an environment by name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&WorkspaceEnvironment> {
        self.0.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// Get all auto-start environments.
    pub fn auto_start(&self) -> Vec<&WorkspaceEnvironment> {
        self.0.iter().filter(|e| e.auto_start == Some(true)).collect()
    }
}

// ── vrw-only types ──

/// CORS configuration. Controls which origins may make cross-origin requests.
///
/// Policy values:
/// - `"any"` — allow all origins (default)
/// - `"none"` — block all cross-origin requests
/// - Comma-separated list of allowed origins
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorsConfig {
    #[serde(default = "default_cors_policy")]
    pub policy: String,
}

#[cfg(feature = "vrw")]
fn default_cors_policy() -> String { "any".to_string() }

#[cfg(feature = "vrw")]
impl Default for CorsConfig {
    fn default() -> Self { Self { policy: default_cors_policy() } }
}

/// Authentication and authorization settings.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// When true, a bearer token must be provided in the Authorization header.
    /// Enable when server.bind is set to 0.0.0.0.
    pub require_auth: bool,
    /// Path to a file containing the bearer token. If missing when auth is
    /// required, a random 256-bit token is generated and saved.
    /// Default: ~/.config/vrw/token
    pub token_file: String,
    /// CORS configuration. Default: allow all origins.
    #[serde(default)]
    pub cors: CorsConfig,
}

#[cfg(feature = "vrw")]
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_auth: false,
            token_file: dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("vrw")
                .join("token")
                .to_string_lossy()
                .to_string(),
            cors: CorsConfig::default(),
        }
    }
}

/// TLS/HTTPS settings. Auto-generates self-signed certs when enabled without
/// explicit cert/key paths (stored in ~/.config/vrw/).
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Enable TLS (HTTPS). Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Path to PEM certificate file. Default: ~/.config/vrw/cert.pem.
    pub cert_file: Option<String>,
    /// Path to PEM private key file. Default: ~/.config/vrw/key.pem.
    pub key_file: Option<String>,
}

/// Certificate pool configuration. Each entry defines a named certificate
/// that can be bound to running commands for client authentication.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CertificatesConfig {
    /// Directory for auto-generated certificates. Default: ~/.config/vrw/certs/
    #[serde(default)]
    pub directory: Option<String>,
    /// Named certificate definitions. Missing files are auto-generated on first use.
    #[serde(default)]
    pub entries: Vec<CertificateEntryConfig>,
}

/// A single named certificate in the pool.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CertificateEntryConfig {
    /// Logical name for this certificate (e.g., "webapp-frontend").
    pub name: String,
    /// Path to PEM certificate file (absolute or relative to `directory`).
    #[serde(default)]
    pub cert_file: String,
    /// Path to PEM private key file (absolute or relative to `directory`).
    #[serde(default)]
    pub key_file: String,
}

/// HTTP server bind address and port.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Bind address. Default "127.0.0.1". Set to "0.0.0.0" for remote access.
    pub bind: String,
    /// TCP port to listen on.
    pub port: u16,
    /// Human-readable name shown in `vrw list` and the web UI panel titlebar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[cfg(feature = "vrw")]
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            port: 9090,
            name: None,
        }
    }
}

/// Web admin panel and VTTY streaming. `update_mode`: "push" or "poll".
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebConfig {
    /// How the web UI discovers buffer changes: "push" or "poll". Default: "push".
    pub update_mode: String,
    /// Server-side dirty-check interval in ms (push mode). Default: 200.
    pub dirty_check_ms: u64,
    /// Client-side polling interval in ms (poll mode). Default: 500.
    pub default_poll_ms: u64,
    /// Per-server panel header colors. Empty = built-in dark palette.
    #[serde(default)]
    pub panel_colors: Vec<PanelColorEntry>,
    /// Max VTTY updates/sec/command. 0 = disabled.
    #[serde(default = "default_max_updates_per_sec")]
    pub max_updates_per_sec: u32,
}

/// A (background, text) color pair for panel headers.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PanelColorEntry {
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub text: String,
}

#[cfg(feature = "vrw")]
impl Default for WebConfig {
    fn default() -> Self {
        Self {
            update_mode: "push".to_string(),
            dirty_check_ms: 200,
            default_poll_ms: 500,
            panel_colors: Vec::new(),
            max_updates_per_sec: default_max_updates_per_sec(),
        }
    }
}

#[cfg(feature = "vrw")]
fn default_max_updates_per_sec() -> u32 { 30 }

// ── daemon ──

/// Daemon (background process) settings. Unix only.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DaemonConfig {
    /// Run as a background daemon (Unix only).
    pub enabled: bool,
    /// File to redirect stdout to when daemonized.
    pub stdout_file: String,
    /// File to redirect stderr to when daemonized.
    pub stderr_file: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            stdout_file: "/tmp/vrc.out".to_string(),
            stderr_file: "/tmp/vrc.err".to_string(),
        }
    }
}

// ── vtty ──

/// Virtual terminal configuration (dimensions, TERM, capabilities).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VttyConfig {
    /// Number of rows in the virtual terminal.
    pub rows: u16,
    /// Number of columns in the virtual terminal.
    pub cols: u16,
    /// The TERM value reported to child processes.
    pub term: String,
    /// Maximum scrollback lines retained.
    pub scrollback: usize,
    /// Enable 24-bit truecolor support.
    pub truecolor: bool,
    /// Enable mouse event forwarding.
    pub mouse: bool,
    /// Default font size for PNG screenshots (vrw only).
    #[cfg(feature = "vrw")]
    pub screenshot_font_size: f32,
    /// Default font file path for PNG screenshots (vrw only).
    #[cfg(feature = "vrw")]
    pub screenshot_font_name: Option<String>,
}

impl Default for VttyConfig {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            term: "xterm-256color".to_string(),
            scrollback: 5000,
            truecolor: true,
            mouse: false,
            #[cfg(feature = "vrw")]
            screenshot_font_size: 14.0,
            #[cfg(feature = "vrw")]
            screenshot_font_name: None,
        }
    }
}

// ── handles ──

/// A pre-configured output handle for directing command output.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HandleConfig {
    /// Handle name (used as identifier in the API).
    pub name: String,
    /// Sink type: "file", "vtty", or "null".
    pub sink: String,
    /// Path for file sinks. Supports {id} and {name} placeholders.
    pub path: Option<String>,
}

// ── environment (env vars) ──

/// Environment variables applied to every spawned command unless overridden
/// per-command (API/CLI --env) or skipped (--no-env).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnvironmentConfig {
    /// Key-value pairs of environment variables. Example: { "RUST_LOG": "debug" }
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

// ── profiles ──

/// Named configuration presets. Each profile is a partial configuration
/// that overrides only the fields present in the profile.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProfilesConfig {
    /// Named configuration presets (key = profile name).
    #[serde(default)]
    pub entries: std::collections::HashMap<String, PartialConfig>,
}

/// Top-level configuration. All fields have sensible defaults.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    /// Binary name ("vrw" or "vrc") — set at runtime from CLI, not from config file.
    #[serde(default, skip)]
    pub binary_name: String,
    /// Whether to use ANSI color codes in terminal log output.
    /// Set at runtime from the `--color-terminal-log` / `-F` CLI flag.
    #[serde(default, skip)]
    pub color_terminal_log: bool,

    /// HTTP server bind address and port (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub server: ServerConfig,
    /// Authentication settings (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub security: SecurityConfig,
    /// TLS/HTTPS certificate settings (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub tls: TlsConfig,
    /// Per-command client certificate pool (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub certificates: CertificatesConfig,
    /// Virtual terminal (VTTY) dimensions and behavior.
    #[serde(default)]
    pub vtty: VttyConfig,
    /// Local terminal display of VTTY output.
    #[serde(default)]
    pub display: DisplayConfig,
    /// Logging of API command events.
    #[serde(default)]
    pub command_log: CommandLogConfig,
    /// Daemon (background) mode settings.
    #[serde(default)]
    pub daemon: DaemonConfig,
    /// Pre-configured output handles for spawned commands.
    #[serde(default)]
    pub handles: Vec<HandleConfig>,
    /// Interactive terminal display settings (tab bar, keyboard).
    #[serde(default)]
    pub interactive: InteractiveConfig,
    /// Default exit configuration applied to all commands unless overridden.
    #[serde(default)]
    pub default_exit: DefaultExitConfig,
    /// Global event hooks — shell commands triggered on lifecycle events.
    #[serde(default)]
    pub hooks: HooksConfig,
    /// Environment variables applied to all spawned commands by default.
    #[serde(default)]
    pub environment: EnvironmentConfig,
    /// Web admin panel and VTTY streaming configuration (vrw only).
    #[cfg(feature = "vrw")]
    #[serde(default)]
    pub web: WebConfig,
    /// Pre-defined command templates.
    #[serde(default)]
    pub templates: TemplatesConfig,
    /// Named workspace environments (panels, servers, commands).
    #[serde(default)]
    pub environments: EnvironmentsConfig,
    /// Named configuration presets.
    #[serde(default)]
    pub profiles: ProfilesConfig,
}

/// A partial configuration used in profiles.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PartialConfig {
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificates: Option<CertificatesConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vtty: Option<VttyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<DisplayConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_log: Option<CommandLogConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handles: Option<Vec<HandleConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interactive: Option<InteractiveConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_exit: Option<DefaultExitConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<EnvironmentConfig>,
    #[cfg(feature = "vrw")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<WebConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<TemplatesConfig>,
}

// ── tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── environments tests ──

    fn make_env(name: &str) -> WorkspaceEnvironment {
        WorkspaceEnvironment {
            name: name.to_string(),
            description: Some(format!("{} env", name)),
            layout: Some("horizontal".to_string()),
            auto_start: Some(true),
            default_server: Some("http://localhost:9090".to_string()),
            default_token: Some("tok123".to_string()),
            panels: vec![
                EnvironmentPanel {
                    title: Some("Panel 1".to_string()),
                    server: Some("http://localhost:9090".to_string()),
                    token: Some("tok123".to_string()),
                    server_label: Some("local".to_string()),
                    commands: vec![EnvironmentCommand {
                        cmd: "bash".to_string(),
                        args: Some("-l".to_string()),
                        workdir: Some("/tmp".to_string()),
                        certificate: Some("cert1".to_string()),
                        rows: Some(30),
                        cols: Some(100),
                        retain_on_exit: Some(true),
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_find_by_name_case_insensitive() {
        let cfg = EnvironmentsConfig(vec![make_env("Dev"), make_env("Prod")]);
        assert!(cfg.find_by_name("dev").is_some());
        assert!(cfg.find_by_name("DEV").is_some());
        assert!(cfg.find_by_name("Prod").is_some());
        assert!(cfg.find_by_name("prod").is_some());
        assert!(cfg.find_by_name("staging").is_none());
    }

    #[test]
    fn test_auto_start_filter() {
        let mut cfg = EnvironmentsConfig(vec![make_env("Dev"), make_env("Prod")]);
        assert_eq!(cfg.auto_start().len(), 2);
        cfg.0[1].auto_start = Some(false);
        assert_eq!(cfg.auto_start().len(), 1);
        assert_eq!(cfg.auto_start()[0].name, "Dev");
    }
}
