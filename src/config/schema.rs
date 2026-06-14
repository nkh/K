use serde::{Deserialize, Serialize};

// ── display ──

/// Local terminal display settings.
/// When enabled, vrc renders VTTY output directly in the
/// terminal it was launched from (similar to mprocs).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
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

/// Configuration for interactive terminal display.
/// Controls keyboard input, scrolling, and command switching in the CLI.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct InteractiveConfig {
    /// Show a tab bar listing all commands at the top of the display.
    /// When disabled, the active command name is shown in the status bar only.
    #[serde(default)]
    pub tabs: bool,
    /// Configurable keybindings for the terminal display.
    /// Maps action names to human-readable key names.
    /// When a key sequence matches, the corresponding action is executed
    /// instead of forwarding the keystroke to the active command.
    ///
    /// Key name format: human-readable names.
    ///   Ctrl+Left  = "ctrl+left"
    ///   Ctrl+Right = "ctrl+right"
    ///   Ctrl+L     = "ctrl+l"
    ///   F12        = "f12"
    ///   Ctrl+H     = "ctrl+h"
    ///
    /// Raw escape sequences (e.g., "\x1b[1;5C") are also accepted
    /// for backward compatibility.
    ///
    /// Available actions:
    ///   "next_command"     — switch to the next running command (wraps around)
    ///   "prev_command"     — switch to the previous running command (wraps around)
    ///   "toggle_log"       — show/hide command log overlay
    ///   "spawn_command"    — open a prompt to spawn a new command
    ///   "show_help"        — show keybinding help overlay
    ///   "kill_command"     — kill (SIGTERM) the active command
    ///   "toggle_pause"     — pause/resume (SIGSTOP/SIGCONT) the active command
    ///   "quit"             — exit the display (same as Ctrl+\)
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

/// Maps action names to key sequences for the interactive terminal display.
/// ```yaml
/// interactive:
///   keybindings:
///     next_command: "ctrl+right"
///     prev_command: "ctrl+left"
///     toggle_log: "ctrl+l"
///     spawn_command: "f12"
///     show_help: "ctrl+h"
///     kill_command: "ctrl+k"
///     toggle_pause: "ctrl+z"
///     quit: "esc"
/// ```
///
/// Raw escape sequences (e.g., `"\x1b[1;5C"`) are still accepted for
/// backward compatibility.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeybindingsConfig {
    /// Switch to the next running command. Default: Ctrl+Right (`ctrl+right`)
    #[serde(default = "default_key_next_command")]
    pub next_command: Option<String>,
    /// Switch to the previous running command. Default: Ctrl+Left (`ctrl+left`)
    #[serde(default = "default_key_prev_command")]
    pub prev_command: Option<String>,
    /// Toggle the command log overlay. Default: Ctrl+L (`ctrl+l`)
    #[serde(default = "default_key_toggle_log")]
    pub toggle_log: Option<String>,
    /// Open a prompt to spawn a new command. Default: F12 (`f12`)
    #[serde(default = "default_key_spawn_command")]
    pub spawn_command: Option<String>,
    /// Show the help overlay. Default: Ctrl+H (`ctrl+h`)
    #[serde(default = "default_key_show_help")]
    pub show_help: Option<String>,
    /// Kill the active command. Default: none
    #[serde(default)]
    pub kill_command: Option<String>,
    /// Pause / resume (freeze/thaw) the active command. Default: none
    #[serde(default)]
    pub toggle_pause: Option<String>,
    /// Quit the display loop. Default: none (use Ctrl+\ = `ctrl+\\`)
    #[serde(default)]
    pub quit: Option<String>,
}

fn default_key_next_command() -> Option<String> {
    Some("ctrl+right".into())
}
fn default_key_prev_command() -> Option<String> {
    Some("ctrl+left".into())
}
fn default_key_toggle_log() -> Option<String> {
    Some("ctrl+l".into())
}
fn default_key_spawn_command() -> Option<String> {
    Some("f12".into())
}
fn default_key_show_help() -> Option<String> {
    Some("ctrl+h".into())
}

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

/// Event hooks configuration.
/// Allows registering shell commands that run on lifecycle events.
///
/// Example:
/// ```yaml
/// hooks:
///   on_spawn: "notify-send 'Started' {name}"
///   on_exit: "echo done"
///   on_kill: "echo killed {name} (pid={pid})"
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HooksConfig {
    /// Command to run when ANY child process is spawned.
    /// Placeholders: {name}, {id}, {pid}
    #[serde(default)]
    pub on_spawn: Option<String>,
    /// Command to run when ANY child process exits cleanly (exit code 0).
    /// Placeholders: {name}, {id}, {pid}, {exit_code}
    #[serde(default)]
    pub on_exit: Option<String>,
    /// Command to run when ANY child process exits with error (non-zero).
    /// Placeholders: {name}, {id}, {pid}, {exit_code}
    #[serde(default)]
    pub on_error: Option<String>,
    /// Command to run when ANY child process is killed.
    /// Placeholders: {name}, {id}, {pid}
    #[serde(default)]
    pub on_kill: Option<String>,
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
    /// When true, the command's VTTY buffer is retained in memory after
    /// the child process exits.  The command appears in the display tab
    /// bar and web UI with an "exited" status, allowing inspection of
    /// the final output.  The buffer can be manually purged via the API.
    /// Default: false (commands are removed from the manager on exit).
    #[serde(default)]
    pub retain_on_exit: bool,
    /// When set to a file path, the VTTY buffer is saved to that file
    /// as plain text when the child process exits.  The snapshot is taken
    /// after the process exits but before the command is removed from the
    /// manager.  This is a per-command option (set via CLI or API).
    ///
    /// The output includes scrollback content followed by the visible
    /// screen rows.  Each line is trimmed of trailing whitespace.
    ///
    /// Example: --snapshot-on-exit /tmp/htop-output.txt
    #[serde(default)]
    pub snapshot_on_exit: Option<String>,
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
            retain_on_exit: false,
            snapshot_on_exit: None,
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

/// ANSI SGR escape sequence for a log field. Empty string = no color.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ColorField {
    #[serde(default)]
    pub ansi: String,
}

/// Terminal log appearance: format string, per-field colors and padding.
///
/// Format placeholders: %timestamp% %pid% %id% %cmd% %event% %details%
/// Default format: "%timestamp% %pid% %cmd% %event% %details%"

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TerminalLogConfig {
    /// Printf-like format string controlling which fields appear in the
    /// terminal log line and in what order.  Available placeholders:
    ///
    ///   %timestamp%  — wall-clock time (HH:MM:SS.cc)
    ///   %pid%        — child process ID
    ///   %id%         — internal command UUID (first 8 chars)
    ///   %cmd%        — command name (binary path basename)
    ///   %event%      — rvw event type (spawn, resize, kill, …)
    ///   %details%    — remaining key=value pairs
    ///
    /// Default: "%timestamp% %pid% %cmd% %event% %details%"
    #[serde(default = "default_terminal_format")]
    pub format: String,
    /// Per-field ANSI color configuration.  Each field maps to a ColorField
    /// containing the ANSI SGR escape sequence.
    #[serde(default)]
    pub colors: TerminalLogColors,
    /// Per-field padding widths.  Fields are padded (right-aligned) to this
    /// width and truncated if longer.
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

fn default_clr_timestamp() -> ColorField { ColorField { ansi: "\x1b[90m".to_string() } }
fn default_clr_pid() -> ColorField { ColorField { ansi: "\x1b[1;37m".to_string() } }
fn default_clr_id() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_cmd() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_event() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_arg() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_cert() -> ColorField { ColorField { ansi: "\x1b[34m".to_string() } }
fn default_clr_env() -> ColorField { ColorField { ansi: "\x1b[32m".to_string() } }
fn default_clr_size() -> ColorField { ColorField { ansi: "\x1b[1;33m".to_string() } }
fn default_clr_dir() -> ColorField { ColorField { ansi: "\x1b[34m".to_string() } }
fn default_clr_detail() -> ColorField { ColorField { ansi: "\x1b[90m".to_string() } }


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

impl Default for TerminalLogColors {
    fn default() -> Self {
        Self {
            timestamp: default_clr_timestamp(),
            pid: default_clr_pid(),
            id: default_clr_id(),
            cmd: default_clr_cmd(),
            event: default_clr_event(),
            arg: default_clr_arg(),
            cert: default_clr_cert(),
            env: default_clr_env(),
            size: default_clr_size(),
            dir: default_clr_dir(),
            detail: default_clr_detail(),
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
    /// Path to a file where raw PTY output from child processes is logged.
    /// Each line contains one `read()` call's worth of data from the PTY
    /// master fd, formatted with elapsed time and escaped bytes:
    ///
    ///   <elapsed_ms> <escaped_bytes>
    ///
    /// Printable ASCII is written as-is; non-printable bytes use \xHH
    /// notation.  This produces a human-readable yet machine-parseable
    /// log that can be replayed step-by-step with the `ansi-replay` tool.
    ///
    /// Set via CLI: `--log-pty-raw <FILE>`
    /// Set via config:
    ///   command_log:
    ///     pty_raw_log: "/tmp/pty-output.log"
    #[serde(default)]
    pub pty_raw_log: Option<String>,
    /// Terminal log appearance and format configuration.
    /// Controls which fields are shown, their colors, padding, and layout.
    ///
    /// Set via config:
    ///   command_log:
    ///     terminal:
    ///       format: "%timestamp% %pid% %cmd% %event% %details%"
    ///       colors:
    ///         timestamp: { ansi: "\x1b[90m" }
    ///         pid: { ansi: "\x1b[1;37m" }
    ///       pad:
    ///         pid: 6
    ///         cmd: 16
    ///         event: 17
    #[serde(default)]
    pub terminal: TerminalLogConfig,
}

// Default derived: all fields have natural defaults


// ── templates ──

/// A pre-defined command template.
///
/// Templates appear in the web UI's Templates sidebar tab and allow
/// users to spawn frequently-used commands with a single click.
/// Optional arguments and environment variables are pre-filled but
/// can be overridden at spawn time.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemplateConfig {
    /// Display name shown in the Templates panel.
    ///
    /// Example: `"Dev server"`
    pub name: String,

    /// The command executable to run.
    ///
    /// Example: `"npm"`
    /// Example: `"/usr/bin/htop"`
    pub cmd: String,

    /// Space-separated arguments passed to the command.
    ///
    /// Example: `"run dev"`
    /// Optional — omit or leave empty for no arguments.
    #[serde(default)]
    pub args: Option<String>,

    /// Environment variables to set when spawning this template.
    ///
    /// Each entry is a `KEY=VALUE` string.  These override the global
    /// `[environment]` defaults but can be overridden per-spawn via the
    /// API `env` field.
    ///
    /// Optional — omit or leave empty for no extra environment.
    #[serde(default)]
    pub env: Option<Vec<String>>,

    /// Working directory for the spawned command.
    ///
    /// Optional — defaults to vrc's own working directory.
    #[serde(default)]
    pub workdir: Option<String>,

    /// Certificate name to bind (from the `[certificates]` section).
    ///
    /// Optional — defaults to no certificate binding.
    #[serde(default)]
    pub certificate: Option<String>,

    /// VTTY rows for the terminal.
    ///
    /// Optional — defaults to the global `[vtty].rows` setting.
    #[serde(default)]
    pub rows: Option<u16>,

    /// VTTY columns for the terminal.
    ///
    /// Optional — defaults to the global `[vtty].cols` setting.
    #[serde(default)]
    pub cols: Option<u16>,
}

/// The templates section of the configuration.
///
/// Contains an array of `[[templates]]` entries.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TemplatesConfig(pub Vec<TemplateConfig>);

impl TemplatesConfig {
    /// Iterate over the template entries.
    pub fn iter(&self) -> impl Iterator<Item = &TemplateConfig> {
        self.0.iter()
    }

    /// Number of templates.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no templates.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}


// ── environments (workspace) ──

/// A single command to spawn within an environment panel.
///
/// Each panel in an environment can have zero or more commands.
/// Commands are spawned sequentially in the order listed.
/// The first command's VTTY is displayed in the panel.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentCommand {
    /// The command executable to run.
    ///
    /// Example: `"npm"`, `"/usr/bin/htop"`, `"cargo"`
    pub cmd: String,

    /// Space-separated arguments passed to the command.
    ///
    /// Example: `"run dev"`, `"--sort-key PID"`
    #[serde(default)]
    pub args: Option<String>,

    /// Working directory for the spawned command.
    ///
    /// Optional — defaults to the server's working directory.
    #[serde(default)]
    pub workdir: Option<String>,

    /// Certificate name to bind (from the server's certificates).
    ///
    /// Optional — defaults to no certificate binding.
    #[serde(default)]
    pub certificate: Option<String>,

    /// VTTY rows for the terminal.
    ///
    /// Optional — defaults to the global `[vtty].rows` setting.
    #[serde(default)]
    pub rows: Option<u16>,

    /// VTTY columns for the terminal.
    ///
    /// Optional — defaults to the global `[vtty].cols` setting.
    #[serde(default)]
    pub cols: Option<u16>,

    /// Whether to retain the terminal buffer after the command exits.
    #[serde(default)]
    pub retain_on_exit: Option<bool>,
}

/// A single panel within an environment.
///
/// Each panel optionally connects to a server and spawns commands.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentPanel {
    /// Panel title/label displayed in the panel header.
    ///
    /// Optional — defaults to the command name or "Panel N".
    #[serde(default)]
    pub title: Option<String>,

    /// Server URL for this panel's commands.
    ///
    /// Optional — if omitted, uses the environment's default server
    /// or the primary (local) instance. If set to a remote URL,
    /// commands are spawned on that remote instance.
    ///
    /// Example: `"http://localhost:9090"`, `"https://prod.example.com:9090"`
    #[serde(default)]
    pub server: Option<String>,

    /// Auth token for the server (if different from the global token).
    #[serde(default)]
    pub token: Option<String>,

    /// Label for the server connection (displayed in the sidebar).
    #[serde(default)]
    pub server_label: Option<String>,

    /// Commands to spawn in this panel.
    ///
    /// Optional — if empty, the panel is created without a running command.
    /// The user can manually connect a command later.
    #[serde(default)]
    pub commands: Vec<EnvironmentCommand>,
}

/// An environment configuration.
///
/// An environment defines a complete workspace setup: one or more panels,
/// each optionally connected to a server and running commands.
/// Environments allow users to quickly switch between different work
/// contexts (e.g., "Development", "Production Monitoring", "CI Pipeline").
///
/// Example TOML:
/// ```toml
/// [[environments]]
/// name = "Dev Workspace"
/// description = "Local development with frontend, backend, and database monitors"
/// layout = "horizontal"
/// auto_start = true
///
/// [[environments.panels]]
/// title = "Frontend"
/// commands = [{ cmd = "npm", args = "run dev", workdir = "/home/user/frontend" }]
///
/// [[environments.panels]]
/// title = "Backend"
/// commands = [{ cmd = "cargo", args = "run", workdir = "/home/user/api" }]
///
/// [[environments.panels]]
/// title = "Database"
/// server = "http://db-server:9090"
/// server_label = "DB Server"
/// commands = [{ cmd = "psql", args = "-U admin mydb" }]
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceEnvironment {
    /// Unique name for this environment.
    ///
    /// Used for display in the web UI and for CLI selection.
    pub name: String,

    /// Optional description of what this environment is for.
    #[serde(default)]
    pub description: Option<String>,

    /// Panel layout direction: "horizontal" (side-by-side) or "vertical" (stacked).
    ///
    /// Optional — defaults to "horizontal".
    #[serde(default)]
    pub layout: Option<String>,

    /// Whether to automatically start this environment when the server loads.
    ///
    /// If true, the server will pre-spawn all commands in all panels
    /// when the environment is activated.
    #[serde(default)]
    pub auto_start: Option<bool>,

    /// Default server URL for panels that don't specify their own.
    ///
    /// Optional — defaults to the primary (local) instance.
    #[serde(default)]
    pub default_server: Option<String>,

    /// Default auth token for panels that don't specify their own.
    #[serde(default)]
    pub default_token: Option<String>,

    /// The panels that make up this environment.
    #[serde(default)]
    pub panels: Vec<EnvironmentPanel>,
}

/// The environments section of the configuration.
///
/// Contains an array of `[[environments]]` entries.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnvironmentsConfig(pub Vec<WorkspaceEnvironment>);

impl EnvironmentsConfig {
    /// Iterate over the environment entries.
    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceEnvironment> {
        self.0.iter()
    }

    /// Number of environments.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no environments.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

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

/// Cross-Origin Resource Sharing (CORS) configuration.
///
/// Controls which origins are allowed to make cross-origin requests to the
/// vrw API and admin interface.
///
/// # Example (YAML)
///
/// ```yaml
/// security:
///   cors:
///     policy: "https://myapp.example.com,https://admin.example.com"
/// ```
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorsConfig {
    /// CORS policy. Determines which origins are allowed for cross-origin requests.
    ///
    /// - `"any"` — allow all origins (default, backward compatible).
    /// - `"none"` — block all cross-origin requests by not setting any
    ///   `Access-Control-Allow-Origin` header.
    /// - A comma-separated list of allowed origins for fine-grained control.
    ///   Example: `"https://myapp.example.com,https://admin.example.com"`
    #[serde(default = "default_cors_policy")]
    pub policy: String,
}

#[cfg(feature = "vrw")]
fn default_cors_policy() -> String {
    "any".to_string()
}

#[cfg(feature = "vrw")]
impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            policy: default_cors_policy(),
        }
    }
}

/// Authentication and authorization settings.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// When false (default), no authentication is required.
    /// When true, a bearer token must be provided in the Authorization header.
    /// This should be enabled when server.bind is set to 0.0.0.0.
    pub require_auth: bool,
    /// Path to a file containing the bearer token. If the file does not exist
    /// when auth is required, a random 256-bit token is generated and saved.
    /// Default: ~/.config/vrw/token
    pub token_file: String,
    /// CORS (Cross-Origin Resource Sharing) configuration.
    /// Controls which origins may make cross-origin requests.
    /// Default: allow all origins.
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

/// TLS/HTTPS settings.
/// When enabled without explicit cert/key paths, vrw auto-generates
/// self-signed certificates stored in ~/.config/vrw/.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Enable TLS (HTTPS). Default: false.
    /// When enabled, vrw generates self-signed certificates on first run
    /// (or uses existing ones). The certificate and key are stored in
    /// ~/.config/vrw/.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the PEM-encoded certificate file.
    /// If not set, defaults to ~/.config/vrw/cert.pem.
    pub cert_file: Option<String>,
    /// Path to the PEM-encoded private key file.
    /// If not set, defaults to ~/.config/vrw/key.pem.
    pub key_file: Option<String>,
}

/// Configuration for the certificate pool.
///
/// Each entry defines a named certificate that can be bound to running commands.
/// When a command is bound to a certificate, only clients presenting that
/// certificate (or its derived bearer token) can interact with the command.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CertificatesConfig {
    /// Directory where auto-generated certificates are stored.
    /// Default: ~/.config/vrw/certs/
    #[serde(default)]
    pub directory: Option<String>,
    /// Named certificate definitions.
    /// Each entry has a name, cert_file, and key_file.
    /// Missing files are auto-generated on first use.
    #[serde(default)]
    pub entries: Vec<CertificateEntryConfig>,
}

/// A single named certificate in the pool.
#[cfg(feature = "vrw")]
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

/// HTTP server bind address and port.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Bind address. Default "127.0.0.1" (localhost only).
    /// Set to "0.0.0.0" to allow remote connections.
    pub bind: String,
    /// TCP port to listen on.
    pub port: u16,
    /// Human-readable name for this server instance.
    /// Displayed in `vrw list`, `vrw cat`, and the web UI panel titlebar.
    /// Falls back to "host:port" when not set.
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

/// Web admin panel and VTTY streaming. update_mode: "push" or "poll".
/// Breaking change: `web.rate_limit.max_updates_per_sec` → `web.max_updates_per_sec`.
#[cfg(feature = "vrw")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebConfig {
    /// How the web UI discovers buffer changes: "push" or "poll".
    /// Default: "push".
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
            panel_colors: Vec::new(), // empty = use built-in palette
            max_updates_per_sec: default_max_updates_per_sec(),
        }
    }
}

#[cfg(feature = "vrw")]
fn default_max_updates_per_sec() -> u32 {
    30
}

// ── daemon ──

/// Daemon (background process) settings.
/// When enabled, vrc forks into the background after binding.
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
            stdout_file: "/tmp/vrc.out".to_string(),
            stderr_file: "/tmp/vrc.err".to_string(),
        }
    }
}

// ── vtty ──

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

// ── environment (env vars) ──

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

// ── profiles ──

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

/// Top-level configuration.
///
/// All fields have sensible defaults, so a config file is entirely optional.
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
    /// Default exit configuration applied to all commands unless overridden per-command.
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

    // ── display tests ──

    #[test]
    fn test_display_config_deserialize_partial() {
        let json = r#"{"enabled": true}"#;
        let config: DisplayConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        // refresh_ms and display_all fall back to Rust Default impl
        assert_eq!(config.refresh_ms, 100);
        assert!(!config.display_all);
    }

    #[test]
    fn test_keybindings_config_deserialize_partial() {
        let json = r#"{"next_command": "ctrl+left", "prev_command": "ctrl+right"}"#;
        let config: KeybindingsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.next_command.as_deref(), Some("ctrl+left"));
        assert_eq!(config.prev_command.as_deref(), Some("ctrl+right"));
        // Other fields use serde defaults
        assert_eq!(config.toggle_log.as_deref(), Some("ctrl+l"));
        assert!(config.kill_command.is_none());
    }

    // ── server tests (vrw only) ──

    #[cfg(feature = "vrw")]
    #[test]
    fn test_server_config_name_skipped_when_none() {
        let cfg = ServerConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(!json.contains("name"));
    }

    #[cfg(feature = "vrw")]
    #[test]
    fn test_server_config_name_included_when_some() {
        let cfg = ServerConfig {
            bind: "127.0.0.1".to_string(),
            port: 9090,
            name: Some("test".to_string()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("name"));
        assert!(json.contains("test"));
    }

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