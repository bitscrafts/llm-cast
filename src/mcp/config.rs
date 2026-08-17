//! Server configuration (spec-03 N6): every runtime value comes from the
//! environment with documented defaults matching the live tv-demo stack. No
//! hardcoded absolute paths — the default socket is built from `$HOME`.

use std::env;

/// All environment-derived server configuration with defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `MUX` — "herdr" (default) or "tmux".
    pub mux: String,
    /// `MUX_SESSION` — the multiplexer session name (default `tv-demo`).
    pub mux_session: String,
    /// `MUX_SOCKET` — the herdr socket path (default
    /// `$HOME/.config/herdr/sessions/tv-demo/herdr.sock`).
    pub mux_socket: String,
    /// `MUX_WORKSPACE` — herdr workspace id (default `w1`).
    pub mux_workspace: String,
    /// `MUX_AGENT_LABEL` — the window label the agent tools use (default
    /// `agent`).
    pub mux_agent_label: String,
    /// `MUX_CYCLE_LABELS` — comma-separated tab labels shown by the cycle
    /// (default `1,watch`).
    pub mux_cycle_labels: String,
    /// `MUX_FOCUS_SECS` — seconds each tab is focused by the cycle (default 10).
    pub mux_focus_secs: u64,
    /// `CAST_DEVICE` — Chromecast host or IP (default `10.10.10.208`).
    pub cast_device: String,
    /// `HLS_DIR` — directory of the HLS segments served to the TV (default
    /// `/tmp/m2/xhls`).
    pub hls_dir: String,
    /// `CYCLE_PID_FILE` — pid file of the running cycle loop (default
    /// `/tmp/m2/tv_cycle.pid`).
    pub cycle_pid_file: String,
    /// `X_DISPLAY` — the framebuffer display (default `:99`).
    pub x_display: String,
    /// `XTERM_GEOMETRY` — the display xterm geometry (default `""` = computed
    /// from `TV_RESOLUTION`/`TV_TERMINAL`/`TV_MARGIN` via `sizing::fit`; any
    /// non-empty value is a legacy verbatim override preserving the old
    /// hardcoded geometry).
    pub xterm_geometry: String,
    /// `TV_RESOLUTION` — the display frame the pipeline renders at, `WxH`
    /// (default `1280x720`). Drives Xvfb, ffmpeg and the computed xterm spec.
    pub tv_resolution: String,
    /// `TV_TERMINAL` — the logical display terminal size, `CxR` (default
    /// `116x34`); the font is auto-fit so this many cols×rows clears the
    /// margins.
    pub tv_terminal: String,
    /// `TV_MARGIN` — symmetric inset fraction of each frame edge kept clear of
    /// the terminal (default `0.10`; must be in `(0, 0.5)`).
    pub tv_margin: f64,
    /// `PROFILES_DIR` — directory where `save_profile` writes and
    /// `load_profile` reads named display configs (default
    /// `$HOME/.config/chromecast-tv-mirror/profiles`).
    pub profiles_dir: String,
}

/// Runtime-overridable display settings. The env-derived [`Config`] is the
/// immutable base; `set_config`/`load_profile` mutate this overlay and the
/// display tools read `overlay.or(config)`. Each `None` means "use the config
/// base", so an unset overlay is bit-identical to the env config.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySettings {
    /// `TV_RESOLUTION` override (`WxH`), or `None` = config.
    pub resolution: Option<String>,
    /// `TV_TERMINAL` override (`CxR`), or `None` = config.
    pub terminal: Option<String>,
    /// `TV_MARGIN` override, or `None` = config.
    pub margin: Option<f64>,
    /// `XTERM_GEOMETRY` legacy-verbatim override, or `None` = config.
    pub geometry: Option<String>,
}

impl Default for DisplaySettings {
    /// The default overlay: everything falls back to the env config.
    fn default() -> Self {
        Self {
            resolution: None,
            terminal: None,
            margin: None,
            geometry: None,
        }
    }
}

impl Config {
    /// Resolve all configuration from the environment, applying the documented
    /// defaults for anything missing.
    pub fn from_env() -> Self {
        let home = env::var("HOME").unwrap_or_default();
        let default_socket = format!("{home}/.config/herdr/sessions/tv-demo/herdr.sock");
        Self {
            mux: env::var("MUX").unwrap_or_else(|_| "herdr".to_string()),
            mux_session: env::var("MUX_SESSION").unwrap_or_else(|_| "tv-demo".to_string()),
            mux_socket: env::var("MUX_SOCKET").unwrap_or(default_socket),
            mux_workspace: env::var("MUX_WORKSPACE").unwrap_or_else(|_| "w1".to_string()),
            mux_agent_label: env::var("MUX_AGENT_LABEL").unwrap_or_else(|_| "agent".to_string()),
            mux_cycle_labels: env::var("MUX_CYCLE_LABELS")
                .unwrap_or_else(|_| "1,watch".to_string()),
            mux_focus_secs: env::var("MUX_FOCUS_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            cast_device: env::var("CAST_DEVICE").unwrap_or_else(|_| "10.10.10.208".to_string()),
            hls_dir: env::var("HLS_DIR").unwrap_or_else(|_| "/tmp/m2/xhls".to_string()),
            cycle_pid_file: env::var("CYCLE_PID_FILE")
                .unwrap_or_else(|_| "/tmp/m2/tv_cycle.pid".to_string()),
            x_display: env::var("X_DISPLAY").unwrap_or_else(|_| ":99".to_string()),
            xterm_geometry: env::var("XTERM_GEOMETRY").unwrap_or_default(),
            tv_resolution: env::var("TV_RESOLUTION").unwrap_or_else(|_| "1280x720".to_string()),
            tv_terminal: env::var("TV_TERMINAL").unwrap_or_else(|_| "116x34".to_string()),
            tv_margin: env::var("TV_MARGIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.10),
            profiles_dir: env::var("PROFILES_DIR")
                .unwrap_or_else(|_| format!("{home}/.config/chromecast-tv-mirror/profiles")),
        }
    }
}
