//! Display tools (spec-03 R6/R8/R9): the framebuffer xterm relaunch
//! (`set_font_size`, `mirror_session`) and the cycle-loop restore. Every
//! subprocess goes through the [`Runner`] seam, so a child can never inherit
//! the operator's `HERDR_*` env (N5) or corrupt the MCP stdio stream (N4).
//! `pgrep` with no match is absence, never an error.
//!
//! Font + geometry are resolution-adaptive by default: `mirror_session`
//! computes them from `TV_RESOLUTION`/`TV_TERMINAL`/`TV_MARGIN` via
//! [`sizing`](super::sizing), so the terminal always fits the real frame. A
//! non-empty `XTERM_GEOMETRY` config opts back into the old hardcoded path.

use std::path::Path;
use std::sync::MutexGuard;

use super::config::DisplaySettings;
use super::errors::McpServerError;
use super::runner::herdr_env_keys;
use super::sizing::{fit, geometry_at, Resolution, TerminalSize};
use super::McpServer;

/// Title marker of the display xterm; `pgrep -f` matches its `-T` title.
pub(crate) const XTERM_TITLE: &str = "herdr-tv";
/// Font size of the legacy verbatim path (a non-empty `XTERM_GEOMETRY`
/// override preserves the pre-adaptive behavior bit-for-bit).
const LEGACY_FONT_PTS: &str = "13";
/// xterm resource overrides for the framebuffer display.
const XTERM_XRM: &str = "XTerm*scrollBar:false XTerm*menuBar:false \
     XTerm*internalBorder:0 XTerm*background:black XTerm*foreground:white";
/// `pgrep -f` pattern identifying the detached cycle loop.
const CYCLE_PATTERN: &str = "herdr tab focus";

impl McpServer {
    /// R6 — validate `pts`, kill the display xterm, relaunch it with
    /// `-fs <pts>` re-attached to the mux session, `HERDR_*` removed and
    /// `DISPLAY=<X>` set.
    pub fn set_font_size_impl(&self, pts: i32) -> Result<String, McpServerError> {
        if !(6..=32).contains(&pts) {
            return Err(McpServerError::InvalidArgument(format!(
                "pts must be in 6..=32, got {pts}"
            )));
        }
        self.kill_display_xterm();
        let attach = self.mux.attach_shell(&self.config.mux_session)?;
        let (frame_str, term_str, margin, geometry) = self.display_settings();
        let geometry = if geometry.is_empty() {
            let frame = Resolution::parse(&frame_str)
                .map_err(|e| McpServerError::InvalidArgument(format!("TV_RESOLUTION: {e}")))?;
            let term = TerminalSize::parse(&term_str)
                .map_err(|e| McpServerError::InvalidArgument(format!("TV_TERMINAL: {e}")))?;
            geometry_at(&frame, &term, margin, f64::from(pts))
                .map_err(|e| McpServerError::InvalidArgument(format!("TV_MARGIN: {e}")))?
        } else {
            geometry
        };
        let argv_owned = self.xterm_argv(&pts.to_string(), &geometry, &attach);
        let argv: Vec<&str> = argv_owned.iter().map(String::as_str).collect();
        let env = vec![("DISPLAY".to_string(), self.config.x_display.clone())];
        let keys = herdr_env_keys();
        let remove_env: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.runner.spawn_detached(&argv, &env, &remove_env)?;
        Ok(format!("display xterm relaunched with font size {pts}"))
    }

    /// R8 — kill the current cycle loop (pid file + `pgrep`), spawn a fresh
    /// detached loop unless disabled and record its pid, then focus the first
    /// cycle window.
    pub fn restore_impl(&self, restart_cycle: bool) -> Result<String, McpServerError> {
        self.kill_cycle_loop();
        if restart_cycle {
            let argv_owned = self.cycle_loop_argv();
            let argv: Vec<&str> = argv_owned.iter().map(String::as_str).collect();
            let keys = herdr_env_keys();
            let remove_env: Vec<&str> = keys.iter().map(String::as_str).collect();
            let pid = self.runner.spawn_detached(&argv, &[], &remove_env)?;
            write_cycle_pid(&self.config.cycle_pid_file, pid)?;
        }
        let first = self.first_cycle_window();
        self.mux.focus(&first)?;
        Ok(format!("cycle restored; focusing {first}"))
    }

    /// R9 — kill the current display xterm, optionally focus a window, then
    /// spawn a new xterm running the driver's single-arg `attach_shell`
    /// (herdr: `exec herdr --session <name>`; tmux: `exec tmux attach -t
    /// <name> -r` — the readonly `-r` is baked into the part-1 tmux driver).
    pub fn mirror_session_impl(
        &self,
        session: &str,
        window: Option<&str>,
    ) -> Result<String, McpServerError> {
        if session.trim().is_empty() {
            return Err(McpServerError::InvalidArgument(
                "session must not be empty".to_string(),
            ));
        }
        self.kill_display_xterm();
        if let Some(target) = window {
            self.mux.focus(target)?;
        }
        self.relaunch_display(session)
    }

    /// Kill the display xterm and respawn it attached to `session`, sized from
    /// the effective display settings with the session's pane size auto-detected
    /// when the backend can report it. Records `session` as the last-mirrored
    /// session so `set_config`/`load_profile` can re-attach to it.
    pub(crate) fn relaunch_display(&self, session: &str) -> Result<String, McpServerError> {
        let attach = self.mux.attach_shell(session)?;
        let term = self.effective_terminal(session)?;
        let (frame_str, _term_str, margin, geometry) = self.display_settings();
        let (font, geometry) = if !geometry.is_empty() {
            (LEGACY_FONT_PTS.to_string(), geometry)
        } else {
            let frame = Resolution::parse(&frame_str)
                .map_err(|e| McpServerError::InvalidArgument(format!("TV_RESOLUTION: {e}")))?;
            let spec = fit(&frame, &term, margin)
                .map_err(|e| McpServerError::InvalidArgument(format!("TV_MARGIN: {e}")))?;
            (spec.font_pts, spec.geometry)
        };
        let argv_owned = self.xterm_argv(&font, &geometry, &attach);
        let argv: Vec<&str> = argv_owned.iter().map(String::as_str).collect();
        let env = vec![("DISPLAY".to_string(), self.config.x_display.clone())];
        let keys = herdr_env_keys();
        let remove_env: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.runner.spawn_detached(&argv, &env, &remove_env)?;
        *self.last_session_guard() = Some(session.to_string());
        Ok(format!(
            "display xterm now mirroring session '{session}' at {}x{}",
            term.cols, term.rows
        ))
    }

    /// The display terminal for a mirror: the auto-detected session size
    /// (session panes + herdr chrome) when the backend can report it, otherwise
    /// the effective `TV_TERMINAL`. The detected size is best-effort — a
    /// failure degrades to the configured value, never an error.
    pub(crate) fn effective_terminal(&self, session: &str) -> Result<TerminalSize, McpServerError> {
        let term_str = self.display_settings().1;
        let config_term = TerminalSize::parse(&term_str)
            .map_err(|e| McpServerError::InvalidArgument(format!("TV_TERMINAL: {e}")))?;
        match self.mux.session_size(session) {
            Ok(Some((cols, rows))) if cols > 0 && rows > 0 => Ok(TerminalSize { cols, rows }),
            _ => Ok(config_term),
        }
    }

    /// The effective display settings: each runtime-override value wins, else
    /// the env-derived config base. `(resolution, terminal, margin, geometry)`.
    pub(crate) fn display_settings(&self) -> (String, String, f64, String) {
        let overlay = self.display_guard();
        (
            overlay
                .resolution
                .clone()
                .unwrap_or_else(|| self.config.tv_resolution.clone()),
            overlay
                .terminal
                .clone()
                .unwrap_or_else(|| self.config.tv_terminal.clone()),
            overlay.margin.unwrap_or(self.config.tv_margin),
            overlay
                .geometry
                .clone()
                .unwrap_or_else(|| self.config.xterm_geometry.clone()),
        )
    }

    /// The runtime display overlay (poison-recovering: a panic while held
    /// yields the value rather than crashing the tool).
    pub(crate) fn display_guard(&self) -> MutexGuard<'_, DisplaySettings> {
        self.display
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The last-mirrored session, so a settings change can re-attach the display
    /// xterm to the same session it was showing.
    pub(crate) fn last_session_guard(&self) -> MutexGuard<'_, Option<String>> {
        self.last_session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The session a display relaunch should attach to: the last-mirrored one
    /// when known, else the configured mux session.
    pub(crate) fn relaunch_session(&self) -> String {
        self.last_session_guard()
            .clone()
            .unwrap_or_else(|| self.config.mux_session.clone())
    }

    /// Change the display settings (resolution/terminal/margin/geometry) at
    /// runtime — each `None` leaves that field untouched. Values are validated
    /// before the overlay is touched, so a bad value leaves the current
    /// settings intact. When a display xterm is running it is relaunched at the
    /// new settings attached to the last-mirrored session.
    pub fn set_config_impl(
        &self,
        resolution: Option<String>,
        terminal: Option<String>,
        margin: Option<f64>,
        geometry: Option<String>,
    ) -> Result<String, McpServerError> {
        if let Some(r) = &resolution {
            Resolution::parse(r)
                .map_err(|e| McpServerError::InvalidArgument(format!("resolution: {e}")))?;
        }
        if let Some(t) = &terminal {
            TerminalSize::parse(t)
                .map_err(|e| McpServerError::InvalidArgument(format!("terminal: {e}")))?;
        }
        if let Some(m) = margin {
            if !(m > 0.0 && m < 0.5) {
                return Err(McpServerError::InvalidArgument(format!(
                    "margin must be in (0, 0.5), got {m}"
                )));
            }
        }
        {
            let mut overlay = self.display_guard();
            if let Some(r) = resolution {
                overlay.resolution = Some(r);
            }
            if let Some(t) = terminal {
                overlay.terminal = Some(t);
            }
            if let Some(m) = margin {
                overlay.margin = Some(m);
            }
            if let Some(g) = geometry {
                overlay.geometry = Some(g);
            }
        }
        let (r, t, m, g) = self.display_settings();
        if self.pgrep_pids(XTERM_TITLE).is_some() {
            let session = self.relaunch_session();
            self.kill_display_xterm();
            self.relaunch_display(&session)?;
        }
        Ok(format!(
            "display settings updated: resolution={r} terminal={t} margin={m} geometry='{g}'"
        ))
    }

    /// Save the current effective display settings as a named profile under
    /// `PROFILES_DIR/<name>.json`.
    pub fn save_profile_impl(&self, name: &str) -> Result<String, McpServerError> {
        let name = validate_profile_name(name)?;
        let (resolution, terminal, margin, geometry) = self.display_settings();
        let profile = Profile {
            resolution,
            terminal,
            margin,
            geometry,
        };
        let json = serde_json::to_string_pretty(&profile)
            .map_err(|e| McpServerError::Internal(format!("serialize profile: {e}")))?;
        let dir = Path::new(&self.config.profiles_dir);
        std::fs::create_dir_all(dir).map_err(|e| {
            McpServerError::Internal(format!("cannot create {}: {e}", dir.display()))
        })?;
        let path = dir.join(format!("{name}.json"));
        std::fs::write(&path, json).map_err(|e| {
            McpServerError::Internal(format!("cannot write {}: {e}", path.display()))
        })?;
        Ok(format!("profile '{name}' saved to {}", path.display()))
    }

    /// Load a named profile and apply it as the display settings, relaunching a
    /// running display xterm at the restored values.
    pub fn load_profile_impl(&self, name: &str) -> Result<String, McpServerError> {
        let name = validate_profile_name(name)?;
        let path = Path::new(&self.config.profiles_dir).join(format!("{name}.json"));
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            McpServerError::InvalidArgument(format!("cannot read profile '{name}': {e}"))
        })?;
        let profile: Profile = serde_json::from_str(&raw).map_err(|e| {
            McpServerError::InvalidArgument(format!("profile '{name}' is not valid JSON: {e}"))
        })?;
        {
            let mut overlay = self.display_guard();
            overlay.resolution = Some(profile.resolution);
            overlay.terminal = Some(profile.terminal);
            overlay.margin = Some(profile.margin);
            overlay.geometry = Some(profile.geometry);
        }
        let (r, t, m, g) = self.display_settings();
        if self.pgrep_pids(XTERM_TITLE).is_some() {
            let session = self.relaunch_session();
            self.kill_display_xterm();
            self.relaunch_display(&session)?;
        }
        Ok(format!(
            "profile '{name}' applied: resolution={r} terminal={t} margin={m} geometry='{g}'"
        ))
    }

    /// The display xterm argv: the live `xterm … -T herdr-tv -e /bin/sh -c
    /// <attach>` shape with the given `-fs` and geometry.
    fn xterm_argv(&self, font_size: &str, geometry: &str, attach_shell: &str) -> Vec<String> {
        vec![
            "xterm".to_string(),
            "-class".to_string(),
            "XTerm".to_string(),
            "-fa".to_string(),
            "DejaVu Sans Mono".to_string(),
            "-fs".to_string(),
            font_size.to_string(),
            "-geometry".to_string(),
            geometry.to_string(),
            "-xrm".to_string(),
            XTERM_XRM.to_string(),
            "-T".to_string(),
            XTERM_TITLE.to_string(),
            "-e".to_string(),
            "/bin/sh".to_string(),
            "-c".to_string(),
            attach_shell.to_string(),
        ]
    }

    /// Kill the display xterm: `pgrep -f` its `-T herdr-tv` title and kill
    /// every match. No match is absence; kill failures are ignored (the
    /// process may already be gone).
    fn kill_display_xterm(&self) {
        if let Some(pids) = self.pgrep_pids(XTERM_TITLE) {
            for pid in pids {
                let _ = self.runner.run(&["kill", &pid], &[], &[]);
            }
        }
    }

    /// Kill the cycle loop: the pid file first, then a `pgrep` fallback for
    /// any process running the loop command.
    fn kill_cycle_loop(&self) {
        if let Ok(content) = std::fs::read_to_string(&self.config.cycle_pid_file) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                if pid > 0 {
                    let _ = self.runner.run(&["kill", &pid.to_string()], &[], &[]);
                }
            }
        }
        if let Some(pids) = self.pgrep_pids(CYCLE_PATTERN) {
            for pid in pids {
                let _ = self.runner.run(&["kill", &pid], &[], &[]);
            }
        }
    }

    /// The fresh cycle loop: a detached `bash -c 'while true; do …; done'`
    /// that focuses each cycle window for `MUX_FOCUS_SECS`, the socket baked
    /// into every herdr invocation.
    fn cycle_loop_argv(&self) -> Vec<String> {
        let labels: Vec<&str> = self
            .config
            .mux_cycle_labels
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let steps: Vec<String> = labels
            .iter()
            .enumerate()
            .map(|(i, _label)| {
                // herdr tab ids are `workspace:tN`; tmux windows are 0-based
                // indices (matching `first_cycle_window`), so the focus command
                // is valid for whichever driver is configured.
                let window = if self.config.mux == "tmux" {
                    i.to_string()
                } else {
                    format!("{}:t{}", self.config.mux_workspace, i + 1)
                };
                format!(
                    "{}; sleep {}",
                    self.cycle_focus_command(&window),
                    self.config.mux_focus_secs
                )
            })
            .collect();
        let body = steps.join("; ");
        vec![
            "bash".to_string(),
            "-c".to_string(),
            format!("while true; do {body}; done"),
        ]
    }

    /// The focus command one cycle step runs (driver-specific).
    fn cycle_focus_command(&self, window: &str) -> String {
        if self.config.mux == "tmux" {
            format!(
                "tmux select-window -t {}:{}",
                self.config.mux_session, window
            )
        } else {
            format!(
                "HERDR_SOCKET_PATH={} herdr tab focus {}",
                self.config.mux_socket, window
            )
        }
    }

    /// The first cycle window id (`w1:t1` for herdr, `0` for tmux).
    fn first_cycle_window(&self) -> String {
        if self.config.mux == "tmux" {
            "0".to_string()
        } else {
            format!("{}:t1", self.config.mux_workspace)
        }
    }

    /// `pgrep -f <pattern>` pids, or `None` when the runner failed or no
    /// process matched — absence, never an error.
    pub(crate) fn pgrep_pids(&self, pattern: &str) -> Option<Vec<String>> {
        let outcome = self.runner.run(&["pgrep", "-f", pattern], &[], &[]).ok()?;
        if outcome.status != 0 {
            return None;
        }
        let pids: Vec<String> = outcome
            .stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        if pids.is_empty() {
            None
        } else {
            Some(pids)
        }
    }

    /// `pgrep -af <pattern>`: pids plus the full cmdlines. The caller pulls
    /// whatever value it needs (the display xterm's `-fs`, Xvfb's `-screen`
    /// frame) via the helpers below. Absence → `None`.
    pub(crate) fn pgrep_full(&self, pattern: &str) -> Option<(Vec<String>, Vec<String>)> {
        let outcome = self.runner.run(&["pgrep", "-af", pattern], &[], &[]).ok()?;
        if outcome.status != 0 {
            return None;
        }
        let mut pids: Vec<String> = Vec::new();
        let mut cmdlines: Vec<String> = Vec::new();
        for line in outcome.stdout.lines() {
            let mut parts = line.splitn(2, ' ');
            if let Some(pid) = parts.next().map(str::trim).filter(|p| !p.is_empty()) {
                pids.push(pid.to_string());
            }
            if let Some(cmdline) = parts.next() {
                cmdlines.push(cmdline.to_string());
            }
        }
        if pids.is_empty() {
            None
        } else {
            Some((pids, cmdlines))
        }
    }
}

/// The display xterm's current `-fs` from `pgrep -af` cmdlines, as `f64`
/// (xterm's `faceSize` is a float resource, so the fit can carry a fractional
/// font). Absent `-fs` → `None`.
pub(crate) fn xterm_font_size(cmdlines: &[String]) -> Option<f64> {
    cmdline_arg_value(cmdlines, "-fs").and_then(|value| value.parse().ok())
}

/// Xvfb's frame from `pgrep -af` cmdlines: the `-screen <n> <WxHx<depth>>`
/// token, returned as `WxH`. Absent → `None`.
pub(crate) fn xvfb_resolution(cmdlines: &[String]) -> Option<String> {
    for tokens in cmdlines
        .iter()
        .map(|line| line.split_whitespace().collect::<Vec<&str>>())
    {
        for (i, token) in tokens.iter().enumerate() {
            if *token == "-screen" {
                if let Some(spec) = tokens.get(i + 2) {
                    let dims: Vec<&str> = spec.split('x').collect();
                    if dims.len() == 3 && !dims[0].is_empty() && !dims[1].is_empty() {
                        return Some(format!("{}x{}", dims[0], dims[1]));
                    }
                }
            }
        }
    }
    None
}

/// The value of the first `-<option> <value>` pair across the cmdlines.
fn cmdline_arg_value(cmdlines: &[String], option: &str) -> Option<String> {
    for tokens in cmdlines
        .iter()
        .map(|line| line.split_whitespace().collect::<Vec<&str>>())
    {
        for (i, token) in tokens.iter().enumerate() {
            if *token == option {
                if let Some(value) = tokens.get(i + 1) {
                    return Some((*value).to_string());
                }
            }
        }
    }
    None
}

fn write_cycle_pid(path: &str, pid: u32) -> Result<(), McpServerError> {
    std::fs::write(path, format!("{pid}\n"))
        .map_err(|e| McpServerError::Internal(format!("cannot write cycle pid file {path}: {e}")))
}

/// A saved named display config: the resolved effective settings at save time,
/// so `load_profile` restores exactly what was saved regardless of the env
/// base. Mirrors [`DisplaySettings`] minus the `None` fallbacks.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Profile {
    pub(crate) resolution: String,
    pub(crate) terminal: String,
    pub(crate) margin: f64,
    pub(crate) geometry: String,
}

/// A profile name must be a single path-safe segment: non-empty, no `/`, no
/// `..`, no whitespace. This is the only guard between a tool argument and a
/// filesystem path, so it errs strict.
fn validate_profile_name(name: &str) -> Result<String, McpServerError> {
    let name = name.trim();
    if name.is_empty()
        || name.contains('/')
        || name.contains("..")
        || name.chars().any(char::is_whitespace)
    {
        return Err(McpServerError::InvalidArgument(format!(
            "profile name must be a single non-empty path-safe segment, got '{name}'"
        )));
    }
    Ok(name.to_string())
}
