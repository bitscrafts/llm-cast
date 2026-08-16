//! Server configuration (spec-03 N6): every runtime value comes from the
//! environment with documented defaults matching the live tv-demo stack. No
//! hardcoded absolute paths — the default socket is built from `$HOME`.

use std::env;

/// All environment-derived server configuration with defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// `XTERM_GEOMETRY` — the display xterm geometry (default `116x32+0+0`).
    pub xterm_geometry: String,
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
            xterm_geometry: env::var("XTERM_GEOMETRY").unwrap_or_else(|_| "116x32+0+0".to_string()),
        }
    }
}
