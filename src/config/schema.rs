use serde::{Deserialize, Serialize};

/// Top-level configuration for vrunner.
///
/// All fields have sensible defaults, so a config file is entirely optional.
/// When no config file is present, vrunner runs with localhost-only HTTP on
/// port 9090, no authentication, and no TLS.
///
/// Config files are searched in this order (later files override earlier):
/// 1. ~/.config/vrunner/config.yaml (or .toml)
/// 2. ./vrunner.yaml (or .toml) in the current directory
/// 3. Path specified with --config CLI flag
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    /// HTTP server bind address and port.
    #[serde(default)]
    pub server: ServerConfig,
    /// Authentication settings (bearer token).
    #[serde(default)]
    pub security: SecurityConfig,
    /// TLS/HTTPS certificate settings.
    #[serde(default)]
    pub tls: TlsConfig,
    /// Per-command client certificate pool.
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
    /// Default exit configuration applied to all commands unless overridden per-command.
    #[serde(default)]
    pub default_exit: DefaultExitConfig,
    /// Environment variables applied to all spawned commands by default.
    /// Can be overridden per-command via the API or CLI.
    /// Ignored when --no-env is passed on the command line.
    #[serde(default)]
    pub environment: EnvironmentConfig,
    /// Web admin panel and VTTY streaming configuration.
    /// Controls how the web UI discovers buffer changes (push vs poll),
    /// the dirty-check interval on the server, and the default polling
    /// interval for clients that use poll mode.
    #[serde(default)]
    pub web: WebConfig,
    /// Named configuration presets.
    /// Each named config is a partial Config that can be referenced by name
    /// via --profile NAME (CLI) or "profile": "NAME" (API).
    /// When a profile is selected, only the fields present in the named config
    /// override the base config. CLI flags always take final precedence.
    #[serde(default)]
    pub profiles: ProfilesConfig,
}

/// HTTP server bind address and port.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Bind address. Default "127.0.0.1" (localhost only).
    /// Set to "0.0.0.0" to allow remote connections.
    pub bind: String,
    /// TCP port to listen on.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1".to_string(),
            port: 9090,
        }
    }
}

/// Authentication and authorization settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// When false (default), no authentication is required.
    /// When true, a bearer token must be provided in the Authorization header.
    /// This should be enabled when server.bind is set to 0.0.0.0.
    pub require_auth: bool,
    /// Path to a file containing the bearer token. If the file does not exist
    /// when auth is required, a random 256-bit token is generated and saved.
    /// Default: ~/.config/vrunner/token
    pub token_file: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_auth: false,
            token_file: dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("vrunner")
                .join("token")
                .to_string_lossy()
                .to_string(),
        }
    }
}

/// TLS/HTTPS settings.
/// When enabled without explicit cert/key paths, vrunner auto-generates
/// self-signed certificates stored in ~/.config/vrunner/.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Enable TLS (HTTPS). Default: false.
    /// When enabled, vrunner generates self-signed certificates on first run
    /// (or uses existing ones). The certificate and key are stored in
    /// ~/.config/vrunner/.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the PEM-encoded certificate file.
    /// If not set, defaults to ~/.config/vrunner/cert.pem.
    pub cert_file: Option<String>,
    /// Path to the PEM-encoded private key file.
    /// If not set, defaults to ~/.config/vrunner/key.pem.
    pub key_file: Option<String>,
}

/// Configuration for the certificate pool.
///
/// Each entry defines a named certificate that can be bound to running commands.
/// When a command is bound to a certificate, only clients presenting that
/// certificate (or its derived bearer token) can interact with the command.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CertificatesConfig {
    /// Directory where auto-generated certificates are stored.
    /// Default: ~/.config/vrunner/certs/
    #[serde(default)]
    pub directory: Option<String>,
    /// Named certificate definitions.
    /// Each entry has a name, cert_file, and key_file.
    /// Missing files are auto-generated on first use.
    #[serde(default)]
    pub entries: Vec<CertificateEntryConfig>,
}

/// A single named certificate in the pool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CertificateEntryConfig {
    /// Logical name for this certificate (e.g., "webapp-frontend").
    pub name: String,
    /// Path to the PEM-encoded certificate file.
    /// Can be absolute or relative to certificates.directory.
    #[serde(default)]
    pub cert_file: String,
    /// Path to the PEM-encoded private key file.
    /// Can be absolute or relative to certificates.directory.
    #[serde(default)]
    pub key_file: String,
}

/// Virtual terminal configuration.
/// Controls the dimensions, TERM value, and capabilities of the pseudo-terminal
/// allocated for each spawned command.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VttyConfig {
    /// Number of rows in the virtual terminal.
    pub rows: u16,
    /// Number of columns in the virtual terminal.
    pub cols: u16,
    /// The TERM value reported to child processes.
    pub term: String,
    /// Maximum number of scrollback lines retained.
    pub scrollback: usize,
    /// Enable 24-bit truecolor support.
    pub truecolor: bool,
    /// Enable mouse event forwarding.
    pub mouse: bool,
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
        }
    }
}

/// Local terminal display settings.
/// When enabled, vrunner renders VTTY output directly in the
/// terminal it was launched from (similar to mprocs).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    /// Show VTTY output on the local terminal.
    /// When the CLI command exits, the display is removed unless
    /// display_all is also enabled.
    pub enabled: bool,
    /// Refresh interval in milliseconds when display is enabled.
    pub refresh_ms: u64,
    /// When enabled, the display stays active after the initial CLI
    /// command exits — it switches to the next available command.
    /// When disabled (default), the display is dismissed and a status
    /// message is printed, but the server keeps running.
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

/// Command logging configuration.
/// Records API command events (spawn, kill, resize, etc.) to a log file.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CommandLogConfig {
    /// Enable logging of API commands.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the command log file. If set, logs are written to this file
    /// in addition to the terminal.
    #[serde(default)]
    pub file: Option<String>,
}

/// Daemon (background process) settings.
/// When enabled, vrunner forks into the background after binding.
/// Only available on Unix systems.
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
            stdout_file: "/tmp/vrunner.out".to_string(),
            stderr_file: "/tmp/vrunner.err".to_string(),
        }
    }
}

/// Configuration for interactive terminal display.
/// Controls keyboard input, scrolling, and command switching in the CLI.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InteractiveConfig {
    /// Show a tab bar listing all commands at the top of the display.
    /// When disabled, the active command name is shown in the status bar only.
    #[serde(default)]
    pub tabs: bool,
    /// Configurable keybindings for the terminal display.
    /// Maps action names to key sequences (raw bytes).
    /// When a key sequence matches, the corresponding action is executed
    /// instead of forwarding the keystroke to the active command.
    ///
    /// Key sequence format: raw escape notation.
    ///   Ctrl+Left  = "\x1b[1;5D"
    ///   Ctrl+Right = "\x1b[1;5C"
    ///   Ctrl+L     = "\x0c"
    ///   F12        = "\x1b[24~"
    ///
    /// Available actions:
    ///   "next_command"     — switch to the next running command (wraps around)
    ///   "prev_command"     — switch to the previous running command (wraps around)
    ///   "toggle_log"       — show/hide command log overlay
    ///   "spawn_command"    — open a prompt to spawn a new command
    ///   "quit"             — exit the display (same as Ctrl+\)
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

impl Default for InteractiveConfig {
    fn default() -> Self {
        Self {
            tabs: false,
            keybindings: KeybindingsConfig::default(),
        }
    }
}

/// Maps action names to key sequences for the interactive terminal display.
///
/// Example YAML:
/// ```yaml
/// interactive:
///   keybindings:
///     next_command: "\x1b[1;5C"   # Ctrl+Right
///     prev_command: "\x1b[1;5D"   # Ctrl+Left
///     toggle_log: "\x0c"           # Ctrl+L
///     spawn_command: "\x1b[24~"    # F12
///     quit: "\x1b"                 # Esc
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeybindingsConfig {
    /// Switch to the next running command. Default: Ctrl+Right (`\x1b[1;5C`)
    #[serde(default = "default_key_next_command")]
    pub next_command: Option<String>,
    /// Switch to the previous running command. Default: Ctrl+Left (`\x1b[1;5D`)
    #[serde(default = "default_key_prev_command")]
    pub prev_command: Option<String>,
    /// Toggle the command log overlay. Default: Ctrl+L (`\x0c`)
    #[serde(default = "default_key_toggle_log")]
    pub toggle_log: Option<String>,
    /// Open a prompt to spawn a new command. Default: F12 (`\x1b[24~`)
    #[serde(default = "default_key_spawn_command")]
    pub spawn_command: Option<String>,
    /// Quit the display loop. Default: none (use Ctrl+\ = `\x1c`)
    #[serde(default)]
    pub quit: Option<String>,
}

fn default_key_next_command() -> Option<String> { Some("\x1b[1;5C".into()) }
fn default_key_prev_command() -> Option<String> { Some("\x1b[1;5D".into()) }
fn default_key_toggle_log() -> Option<String> { Some("\x0c".into()) }
fn default_key_spawn_command() -> Option<String> { Some("\x1b[24~".into()) }

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next_command: default_key_next_command(),
            prev_command: default_key_prev_command(),
            toggle_log: default_key_toggle_log(),
            spawn_command: default_key_spawn_command(),
            quit: None,
        }
    }
}

/// Exit configuration for a command.
/// Controls what happens when a command exits, including cleanup commands and timeouts.
/// This can be set per-command via the spawn API or as defaults in the config.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExitConfig {
    /// Command to run when the child exits cleanly (exit code 0).
    /// The string is split on whitespace into a binary and arguments.
    /// Example: "notify-send Build OK"
    /// Set to null to disable.
    #[serde(default)]
    pub on_exit: Option<String>,
    /// Command to run when the child exits with a non-zero code.
    /// Example: "notify-send Build FAILED"
    /// Set to null to disable.
    #[serde(default)]
    pub on_error: Option<String>,
    /// Maximum seconds to wait for the child to exit after SIGTERM
    /// before sending SIGKILL. Default: 10 seconds.
    /// Applies when kill is called or when the server shuts down.
    #[serde(default = "default_exit_timeout")]
    pub timeout_secs: u64,
}

fn default_exit_timeout() -> u64 {
    10
}

impl Default for ExitConfig {
    fn default() -> Self {
        Self {
            on_exit: None,
            on_error: None,
            timeout_secs: default_exit_timeout(),
        }
    }
}

/// Default exit configuration (used when none is specified per-command).
/// The inner ExitConfig values serve as global defaults for all spawned commands.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DefaultExitConfig {
    /// Exit behavior applied to every command unless overridden.
    #[serde(default)]
    pub exit: ExitConfig,
}

/// A pre-configured output handle.
/// Handles can be attached to spawned commands to direct their output
/// to a file, VTTY, or null sink by name.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HandleConfig {
    /// Name of the handle (used as the identifier in the API).
    pub name: String,
    /// Sink type: "file", "vtty", or "null".
    pub sink: String,
    /// Path for file sinks. Supports {id} and {name} placeholders.
    pub path: Option<String>,
}

/// Environment variable configuration for spawned commands.
///
/// Variables defined here are applied to every spawned command unless:
/// - The command is spawned with --no-env (CLI), which skips config env vars
/// - The variable is overridden by a per-command env var (API or CLI)
///
/// Per-command environment variables (from API or CLI --env flags) are always
/// merged on top of config environment variables, allowing overrides.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnvironmentConfig {
    /// Key-value pairs of environment variables to set in child processes.
    /// Example: { "RUST_LOG": "debug", "DATABASE_URL": "postgres://..." }
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

/// Named configuration profiles.
///
/// Each profile is a partial configuration that can be selected by name.
/// When selected, only the fields present in the profile override the base config.
/// This allows defining reusable configurations for different environments
/// (e.g., "production", "development", "testing").
///
/// Example:
/// ```yaml
/// profiles:
///   production:
///     server:
///       bind: "0.0.0.0"
///     security:
///       require_auth: true
///     environment:
///       variables:
///         RUST_LOG: "warn"
///   development:
///     vtty:
///       rows: 40
///       cols: 120
///     environment:
///       variables:
///         RUST_LOG: "debug"
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProfilesConfig {
    /// Named configuration presets. The key is the profile name.
    #[serde(default)]
    pub entries: std::collections::HashMap<String, PartialConfig>,
}

/// Web admin panel and VTTY streaming configuration.
///
/// Controls how the web UI discovers that a terminal buffer has changed.
/// Two update modes are supported:
///
/// - **push** (default): The server detects buffer changes via a periodic
///   dirty-check loop and sends lightweight "dirty" signals over the
///   existing WebSocket connection.  The client then fetches fresh HTML
///   at its own pace (debounced).  This is the most efficient mode
///   because no polling is required — the server only sends when
///   something actually changed.
///
/// - **poll**: The web client periodically calls the
///   `GET /api/commands/:id/vtty/changed` endpoint to ask "has the
///   buffer changed since last time?".  If yes, the client fetches
///   the full HTML.  This mode is useful when WebSocket connections
///   are unreliable (e.g. reverse proxies that buffer frames) or
///   when the client wants full control over refresh timing.
///
/// The dirty-check interval (`dirty_check_ms`) only affects server-side
/// behaviour in push mode — it controls how often the server compares
/// the current buffer against the last-sent snapshot.
///
/// Example YAML:
/// ```yaml
/// web:
///   update_mode: push       # or "poll"
///   dirty_check_ms: 200     # server check interval (push mode)
///   default_poll_ms: 500    # client poll interval (poll mode)
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebConfig {
    /// How the web UI discovers buffer changes: "push" or "poll".
    /// Default: "push".
    pub update_mode: String,
    /// Server-side dirty-check interval in milliseconds.
    /// Only relevant in push mode. The server compares the VTTY buffer
    /// against the last-sent snapshot at this interval and sends a
    /// "vtty_dirty" WebSocket message when changes are detected.
    /// Default: 200 ms.
    pub dirty_check_ms: u64,
    /// Default client-side polling interval in milliseconds.
    /// Only relevant in poll mode. The web UI will poll
    /// `GET /api/commands/:id/vtty/changed` at this interval.
    /// The user can override this via the web UI controls.
    /// Default: 500 ms.
    pub default_poll_ms: u64,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            update_mode: "push".to_string(),
            dirty_check_ms: 200,
            default_poll_ms: 500,
        }
    }
}

/// A partial configuration used in profiles.
///
/// All fields are optional. When a profile is applied, only the fields
/// that are `Some(..)` override the corresponding fields in the base Config.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PartialConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<SecurityConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsConfig>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<WebConfig>,
}
