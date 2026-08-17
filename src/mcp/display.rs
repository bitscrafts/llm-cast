//! Display tools (spec-03 R6/R8/R9): the framebuffer xterm relaunch
//! (`set_font_size`, `mirror_session`) and the cycle-loop restore. Every
//! subprocess goes through the [`Runner`] seam, so a child can never inherit
//! the operator's `HERDR_*` env (N5) or corrupt the MCP stdio stream (N4).
//! `pgrep` with no match is absence, never an error.

use super::errors::McpServerError;
use super::runner::herdr_env_keys;
use super::McpServer;

/// Title marker of the display xterm; `pgrep -f` matches its `-T` title.
pub(crate) const XTERM_TITLE: &str = "herdr-tv";
/// Default display font size when mirroring a session (matches the live stack).
const DEFAULT_FONT_PTS: &str = "13";
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
        let argv_owned = self.xterm_argv(&pts.to_string(), &attach);
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
        let attach = self.mux.attach_shell(session)?;
        let argv_owned = self.xterm_argv(DEFAULT_FONT_PTS, &attach);
        let argv: Vec<&str> = argv_owned.iter().map(String::as_str).collect();
        let env = vec![("DISPLAY".to_string(), self.config.x_display.clone())];
        let keys = herdr_env_keys();
        let remove_env: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.runner.spawn_detached(&argv, &env, &remove_env)?;
        Ok(format!("display xterm now mirroring session '{session}'"))
    }

    /// The display xterm argv: the live `xterm … -T herdr-tv -e /bin/sh -c
    /// <attach>` shape with the given `-fs` and the configured geometry.
    fn xterm_argv(&self, font_size: &str, attach_shell: &str) -> Vec<String> {
        vec![
            "xterm".to_string(),
            "-class".to_string(),
            "XTerm".to_string(),
            "-fa".to_string(),
            "DejaVu Sans Mono".to_string(),
            "-fs".to_string(),
            font_size.to_string(),
            "-geometry".to_string(),
            self.config.xterm_geometry.clone(),
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

    /// `pgrep -af <pattern>`: pids plus the first `-fs` value found in the
    /// cmdlines (the display xterm's current font size). Absence → `None`.
    pub(crate) fn pgrep_full(&self, pattern: &str) -> Option<(Vec<String>, Option<i32>)> {
        let outcome = self.runner.run(&["pgrep", "-af", pattern], &[], &[]).ok()?;
        if outcome.status != 0 {
            return None;
        }
        let mut pids: Vec<String> = Vec::new();
        let mut font_size: Option<i32> = None;
        for line in outcome.stdout.lines() {
            let mut parts = line.splitn(2, ' ');
            if let Some(pid) = parts.next().map(str::trim).filter(|p| !p.is_empty()) {
                pids.push(pid.to_string());
            }
            if let Some(cmdline) = parts.next() {
                let tokens: Vec<&str> = cmdline.split_whitespace().collect();
                for (i, token) in tokens.iter().enumerate() {
                    if *token == "-fs" && font_size.is_none() {
                        if let Some(value) = tokens.get(i + 1).and_then(|v| v.parse().ok()) {
                            font_size = Some(value);
                        }
                    }
                }
            }
        }
        if pids.is_empty() {
            None
        } else {
            Some((pids, font_size))
        }
    }
}

fn write_cycle_pid(path: &str, pid: u32) -> Result<(), McpServerError> {
    std::fs::write(path, format!("{pid}\n"))
        .map_err(|e| McpServerError::Internal(format!("cannot write cycle pid file {path}: {e}")))
}
