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

/// Columns of herdr UI chrome reserved inside the attaching client. Observed
/// live 2026-08-17: a `161x49` session laid out as `area {x:4, y:1, w:161,
/// h:49}` in a `165x50` client — so a client of `WxH` shows `(W-4)x(H-1)` of
/// pane, and a `WxH` pane needs a `(W+4)x(H+1)` client to be fully visible.
const CLIENT_CHROME_COLS: u32 = 4;
const CLIENT_CHROME_ROWS: u32 = 1;

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
        self.herdr_json_at(&self.socket, args)
    }

    /// `herdr_json` against an explicit socket — the one case that needs a
    /// different session than this driver's configured socket is `session_size`
    /// (the mirrored session, e.g. `default`, vs the agent session `tv-demo`).
    fn herdr_json_at(&self, socket: &str, args: &[&str]) -> Result<serde_json::Value, MuxError> {
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push("herdr");
        argv.extend_from_slice(args);
        let env = vec![("HERDR_SOCKET_PATH".to_string(), socket.to_string())];
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

    /// The socket of a named session from `herdr session list` (a global
    /// listing that does not depend on this driver's socket — verified live
    /// 2026-08-17). The socket is the final whitespace column of the row whose
    /// first column is the session name; a missing session → `None`.
    fn session_socket(&self, session: &str) -> Option<String> {
        let argv = vec!["herdr", "session", "list"];
        let remove_env: Vec<&str> = self.herdr_env_keys.iter().map(String::as_str).collect();
        let outcome = self
            .runner
            .run(&argv, &[], &remove_env)
            .ok()
            .filter(|o| o.status == 0)?;
        for line in outcome.stdout.lines() {
            let mut fields = line.split_whitespace();
            if fields.next() == Some(session) {
                return fields.last().map(str::to_string);
            }
        }
        None
    }

    /// Run a fire-and-forget `herdr` command where only the exit status
    /// matters. `pane run` writes the command's output into the pane's
    /// terminal and prints nothing to its own stdout (verified live against
    /// the tv-demo session 2026-08-17), so these calls must not require JSON.
    fn herdr_ok(&self, args: &[&str]) -> Result<(), MuxError> {
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
        Ok(())
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
        self.herdr_ok(&["pane", "run", &pane.id, &script])?;
        Ok(())
    }

    fn run_command(&self, command: &str) -> Result<(), MuxError> {
        let window = self.ensure_window(&self.agent_label)?;
        let pane = self.pane_in(&window.id)?;
        self.focus(&window.id)?;
        self.herdr_ok(&["pane", "run", &pane.id, command])?;
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

    fn session_size(&self, session: &str) -> Result<Option<(u32, u32)>, MuxError> {
        // Best-effort by design: any hiccup (session list unparseable, snapshot
        // empty, server down) degrades to Ok(None) so the caller falls back to
        // the configured TV_TERMINAL instead of failing the mirror.
        //
        // Known limitation (observed live 2026-08-17, rare ~1-5%): herdr's
        // server can occasionally return ANOTHER session's snapshot against a
        // socket (a `76x23` tv-demo snapshot through the `default` socket), so
        // the detected size is briefly the wrong session's. Both herdr sessions
        // share the `w1` workspace and overlapping pane/tab ids, so no reliable
        // cross-check exists; a wrong size here only ever misreports
        // `pipeline_status`, never the mirror launch (the mirror itself has
        // never been observed wrong), and the caller's config fallback covers
        // the degraded case.
        let socket = match self.session_socket(session) {
            Some(s) => s,
            None => return Ok(None),
        };
        let value = match self.herdr_json_at(&socket, &["api", "snapshot"]) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        let layouts = match value
            .get("result")
            .and_then(|r| r.get("snapshot"))
            .and_then(|snap| snap.get("layouts"))
            .and_then(|layouts| layouts.as_array())
        {
            Some(layouts) => layouts,
            None => return Ok(None),
        };
        let mut cols = 0u32;
        let mut rows = 0u32;
        for layout in layouts {
            let area = layout.get("area");
            if let (Some(width), Some(height)) = (
                area.and_then(|a| a.get("width")).and_then(|v| v.as_u64()),
                area.and_then(|a| a.get("height")).and_then(|v| v.as_u64()),
            ) {
                cols = cols.max(width as u32);
                rows = rows.max(height as u32);
            }
        }
        if cols == 0 || rows == 0 {
            return Ok(None);
        }
        // The client must be larger than the pane area by herdr's own chrome so
        // no pane is clipped.
        Ok(Some((cols + CLIENT_CHROME_COLS, rows + CLIENT_CHROME_ROWS)))
    }
}
