//! tmux driver (spec-03 R2): tmux-compatible backend for the same `Mux`
//! contract — `list-windows`/`new-window`/`select-window`/`send-keys`/
//! `kill-window`/`list-panes` with `-F` formatted output. Contract-tested
//! against a fake runner only; live parity is a phase-8 verification step.

use std::sync::Arc;

use crate::mcp::runner::Runner;

use super::{Mux, MuxError, PaneInfo, WindowInfo};

/// Driver for the tmux CLI, session-scoped by `-t <session>`.
pub struct TmuxMux {
    runner: Arc<dyn Runner>,
    session: String,
    agent_label: String,
}

impl TmuxMux {
    pub fn new(runner: Arc<dyn Runner>, session: &str, agent_label: &str) -> Self {
        Self {
            runner,
            session: session.to_string(),
            agent_label: agent_label.to_string(),
        }
    }

    /// Run `tmux <args>`; non-zero exits become [`MuxError`]s with the output.
    fn tmux(&self, args: &[&str]) -> Result<String, MuxError> {
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push("tmux");
        argv.extend_from_slice(args);
        let outcome = self
            .runner
            .run(&argv, &[], &[])
            .map_err(|e| MuxError::Missing {
                detail: format!("spawn tmux: {e}"),
            })?;
        if outcome.status != 0 {
            return Err(MuxError::Command {
                command: argv.join(" "),
                status: outcome.status,
                stdout: outcome.stdout,
                stderr: outcome.stderr,
            });
        }
        Ok(outcome.stdout)
    }

    /// `-F '#{window_index}|#{window_name}'` lines → [`WindowInfo`].
    fn parse_windows(&self, output: &str) -> Result<Vec<WindowInfo>, MuxError> {
        Ok(output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let mut parts = line.splitn(2, '|');
                let id = parts.next().unwrap_or_default();
                let label = parts.next().unwrap_or_default();
                WindowInfo {
                    id: id.to_string(),
                    label: label.to_string(),
                }
            })
            .collect())
    }

    /// `-F '#{window_index}|#{pane_id}'` lines → [`PaneInfo`].
    fn parse_panes(&self, output: &str) -> Result<Vec<PaneInfo>, MuxError> {
        Ok(output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let mut parts = line.splitn(2, '|');
                let window_id = parts.next().unwrap_or_default();
                let id = parts.next().unwrap_or_default();
                PaneInfo {
                    id: id.to_string(),
                    window_id: window_id.to_string(),
                }
            })
            .collect())
    }
}

impl Mux for TmuxMux {
    fn ensure_window(&self, label: &str) -> Result<WindowInfo, MuxError> {
        if let Some(found) = self.list_windows()?.into_iter().find(|w| w.label == label) {
            return Ok(found);
        }
        // new-window's output format is not parsed; re-list and match by name.
        let _ = self.tmux(&["new-window", "-t", &self.session, "-n", label])?;
        self.list_windows()?
            .into_iter()
            .find(|w| w.label == label)
            .ok_or_else(|| MuxError::Runtime {
                detail: format!("created window '{label}' but could not find it by name"),
            })
    }

    fn focus(&self, window: &str) -> Result<(), MuxError> {
        let target = format!("{}:{}", self.session, window);
        let _ = self.tmux(&["select-window", "-t", &target])?;
        Ok(())
    }

    fn send_text(&self, text: &str) -> Result<(), MuxError> {
        let window = self.ensure_window(&self.agent_label)?;
        self.focus(&window.id)?;
        // Same printf shape as the herdr driver: the text is shell-single-quoted
        // so tmux only has to send a fixed script + Enter (no key-name hazards).
        let target = format!("{}:{}", self.session, window.id);
        let script = format!("printf '%s\\n' {}", super::shell_single_quote(text));
        let _ = self.tmux(&["send-keys", "-t", &target, &script, "Enter"])?;
        Ok(())
    }

    fn run_command(&self, command: &str) -> Result<(), MuxError> {
        let window = self.ensure_window(&self.agent_label)?;
        self.focus(&window.id)?;
        let target = format!("{}:{}", self.session, window.id);
        let _ = self.tmux(&["send-keys", "-t", &target, command, "Enter"])?;
        Ok(())
    }

    fn close_window(&self, window: &str) -> Result<(), MuxError> {
        let target = format!("{}:{}", self.session, window);
        let _ = self.tmux(&["kill-window", "-t", &target])?;
        Ok(())
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>, MuxError> {
        let out = self.tmux(&[
            "list-windows",
            "-t",
            &self.session,
            "-F",
            "#{window_index}|#{window_name}",
        ])?;
        self.parse_windows(&out)
    }

    fn list_panes(&self) -> Result<Vec<PaneInfo>, MuxError> {
        let out = self.tmux(&[
            "list-panes",
            "-t",
            &self.session,
            "-F",
            "#{window_index}|#{pane_id}",
        ])?;
        self.parse_panes(&out)
    }

    fn attach_shell(&self, session: &str) -> Result<String, MuxError> {
        Ok(format!(
            "exec tmux attach -t {} -r",
            super::shell_single_quote(session)
        ))
    }

    fn session_size(&self, session: &str) -> Result<Option<(u32, u32)>, MuxError> {
        // The widest/tallest pane bounds what a single client must show. tmux
        // has no client-side chrome, so the returned size is the pane size
        // verbatim. Best-effort: a failing list degrades to Ok(None).
        let out = match self.tmux(&[
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_width}|#{pane_height}",
        ]) {
            Ok(out) => out,
            Err(_) => return Ok(None),
        };
        let mut cols = 0u32;
        let mut rows = 0u32;
        for line in out.lines() {
            let mut fields = line.split('|');
            if let (Some(width), Some(height)) = (fields.next(), fields.next()) {
                if let (Ok(width), Ok(height)) =
                    (width.trim().parse::<u32>(), height.trim().parse::<u32>())
                {
                    cols = cols.max(width);
                    rows = rows.max(height);
                }
            }
        }
        if cols == 0 || rows == 0 {
            return Ok(None);
        }
        Ok(Some((cols, rows)))
    }
}
