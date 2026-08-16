//! herdr driver (spec-03 R2): talks to a herdr session's socket via its CLI
//! (`herdr tab list/create/focus/close`, `herdr pane list/run`), with JSON on
//! stdout and `HERDR_SOCKET_PATH` selecting the session on every invocation.
//!
//! Verified against the live tv-demo session (2026-08-16): `tab list` →
//! `result.tabs[].tab_id|label`, `pane list` → `result.panes[].pane_id|tab_id`.
//! The create response shape is deliberately not parsed — ids are always
//! learned by re-listing and matching the label.

use std::sync::Arc;

use crate::mcp::runner::{herdr_env_keys, Runner};

use super::{Mux, MuxError, PaneInfo, WindowInfo};

/// Driver for the herdr socket CLI.
pub struct HerdrMux {
    runner: Arc<dyn Runner>,
    socket: String,
    workspace: String,
    agent_label: String,
    /// Inherited `HERDR_*` env keys captured at construction; stripped on every
    /// call so a child can never drive the operator's default session.
    herdr_env_keys: Vec<String>,
}

impl HerdrMux {
    pub fn new(runner: Arc<dyn Runner>, socket: &str, workspace: &str, agent_label: &str) -> Self {
        Self {
            runner,
            socket: socket.to_string(),
            workspace: workspace.to_string(),
            agent_label: agent_label.to_string(),
            herdr_env_keys: herdr_env_keys(),
        }
    }

    /// Run `herdr <args>` against the configured socket; non-zero exits and
    /// unparseable JSON become [`MuxError`]s carrying the raw output.
    fn herdr_json(&self, args: &[&str]) -> Result<serde_json::Value, MuxError> {
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push("herdr");
        argv.extend_from_slice(args);
        let env = vec![("HERDR_SOCKET_PATH".to_string(), self.socket.clone())];
        let remove_env: Vec<&str> = self.herdr_env_keys.iter().map(String::as_str).collect();
        let outcome = self
            .runner
            .run(&argv, &env, &remove_env)
            .map_err(|e| MuxError::Missing {
                detail: format!("spawn herdr: {e}"),
            })?;
        if outcome.status != 0 {
            return Err(MuxError::Command {
                command: argv.join(" "),
                status: outcome.status,
                stdout: outcome.stdout,
                stderr: outcome.stderr,
            });
        }
        serde_json::from_str(&outcome.stdout).map_err(|_| MuxError::Parse {
            command: argv.join(" "),
            raw: outcome.stdout,
        })
    }

    /// The pane in `window_id`, or a runtime error.
    fn pane_in(&self, window_id: &str) -> Result<PaneInfo, MuxError> {
        self.list_panes()?
            .into_iter()
            .find(|p| p.window_id == window_id)
            .ok_or_else(|| MuxError::Runtime {
                detail: format!("no pane found in window '{window_id}'"),
            })
    }
}

impl Mux for HerdrMux {
    fn ensure_window(&self, label: &str) -> Result<WindowInfo, MuxError> {
        if let Some(found) = self.list_windows()?.into_iter().find(|w| w.label == label) {
            return Ok(found);
        }
        // Create then always re-list: the create response shape is unverified,
        // so the id is learned by matching the label afterwards.
        let _ = self.herdr_json(&[
            "tab",
            "create",
            "--workspace",
            &self.workspace,
            "--label",
            label,
            "--no-focus",
        ])?;
        self.list_windows()?
            .into_iter()
            .find(|w| w.label == label)
            .ok_or_else(|| MuxError::Runtime {
                detail: format!("created tab '{label}' but could not find it by label"),
            })
    }

    fn focus(&self, window: &str) -> Result<(), MuxError> {
        let _ = self.herdr_json(&["tab", "focus", window])?;
        Ok(())
    }

    fn send_text(&self, text: &str) -> Result<(), MuxError> {
        let window = self.ensure_window(&self.agent_label)?;
        let pane = self.pane_in(&window.id)?;
        self.focus(&window.id)?;
        let script = format!("printf '%s\\n' {}", super::shell_single_quote(text));
        let _ = self.herdr_json(&["pane", "run", &pane.id, &script])?;
        Ok(())
    }

    fn run_command(&self, command: &str) -> Result<(), MuxError> {
        let window = self.ensure_window(&self.agent_label)?;
        let pane = self.pane_in(&window.id)?;
        self.focus(&window.id)?;
        let _ = self.herdr_json(&["pane", "run", &pane.id, command])?;
        Ok(())
    }

    fn close_window(&self, window: &str) -> Result<(), MuxError> {
        let _ = self.herdr_json(&["tab", "close", window])?;
        Ok(())
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>, MuxError> {
        let command = "herdr tab list";
        let value = self.herdr_json(&["tab", "list"])?;
        let tabs = value
            .get("result")
            .and_then(|r| r.get("tabs"))
            .and_then(|t| t.as_array())
            .ok_or_else(|| MuxError::Parse {
                command: command.to_string(),
                raw: value.to_string(),
            })?;
        let mut out = Vec::with_capacity(tabs.len());
        for tab in tabs {
            let id = tab
                .get("tab_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MuxError::Parse {
                    command: command.to_string(),
                    raw: value.to_string(),
                })?;
            let label = tab
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            out.push(WindowInfo {
                id: id.to_string(),
                label: label.to_string(),
            });
        }
        Ok(out)
    }

    fn list_panes(&self) -> Result<Vec<PaneInfo>, MuxError> {
        let command = "herdr pane list";
        let value = self.herdr_json(&["pane", "list"])?;
        let panes = value
            .get("result")
            .and_then(|r| r.get("panes"))
            .and_then(|p| p.as_array())
            .ok_or_else(|| MuxError::Parse {
                command: command.to_string(),
                raw: value.to_string(),
            })?;
        let mut out = Vec::with_capacity(panes.len());
        for pane in panes {
            let id = pane
                .get("pane_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MuxError::Parse {
                    command: command.to_string(),
                    raw: value.to_string(),
                })?;
            let window_id =
                pane.get("tab_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MuxError::Parse {
                        command: command.to_string(),
                        raw: value.to_string(),
                    })?;
            out.push(PaneInfo {
                id: id.to_string(),
                window_id: window_id.to_string(),
            });
        }
        Ok(out)
    }

    fn attach_shell(&self, session: &str) -> Result<String, MuxError> {
        Ok(format!(
            "exec herdr --session {}",
            super::shell_single_quote(session)
        ))
    }
}
