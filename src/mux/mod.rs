//! Terminal multiplexer abstraction (spec-03 R2): a shared `Mux` trait with
//! exactly two drivers — herdr (socket CLI, JSON stdout) and tmux (formatted
//! `-F` output) — selected by the `MUX` env value in [`open`].
//!
//! All display work in the MCP server goes through this trait, never a
//! protocol-specific CLI. Construction is lazy: [`open`] and the driver
//! constructors never touch the socket/server, so a missing socket surfaces as
//! an `Err(MuxError)` on the first command, not as a failure to start.

use std::sync::Arc;

use crate::mcp::runner::Runner;

pub mod herdr;
pub mod tmux;

/// Which multiplexer backend a driver talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxKind {
    Herdr,
    Tmux,
}

impl MuxKind {
    /// Parse the `MUX` env value. Unknown values are a hard config error that
    /// names the accepted values.
    pub fn parse(value: &str) -> Result<Self, MuxError> {
        match value {
            "herdr" => Ok(MuxKind::Herdr),
            "tmux" => Ok(MuxKind::Tmux),
            other => Err(MuxError::Config {
                detail: format!("unknown MUX value '{other}'; accepted values: herdr, tmux"),
            }),
        }
    }
}

/// A window (herdr "tab", tmux "window").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// Driver-specific window id (`w1:t1` for herdr, `0` for tmux).
    pub id: String,
    /// Window name/label (the `--label` of a herdr tab, `#{window_name}`).
    pub label: String,
}

/// A pane inside a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    /// Driver-specific pane id (`w1:p1` for herdr, `%5` for tmux).
    pub id: String,
    /// The window this pane belongs to.
    pub window_id: String,
}

/// Errors from the mux layer. Every variant carries the raw command output
/// where it exists, so a wrong premise is diagnosable rather than silent.
#[derive(Debug, thiserror::Error)]
pub enum MuxError {
    /// A mux command exited non-zero (e.g. herdr could not reach its socket).
    #[error(
        "mux command failed: {command}\nstatus: {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    Command {
        command: String,
        status: i32,
        stdout: String,
        stderr: String,
    },
    /// A mux command returned output that could not be parsed as the contract.
    #[error("mux returned unparseable output for {command}:\n{raw}")]
    Parse { command: String, raw: String },
    /// The mux backend could not be spawned at all (binary missing etc).
    #[error("mux backend unavailable: {detail}")]
    Missing { detail: String },
    /// A mux-level runtime failure (e.g. a created window never appeared).
    #[error("mux runtime error: {detail}")]
    Runtime { detail: String },
    /// Invalid configuration.
    #[error("mux configuration error: {detail}")]
    Config { detail: String },
}

/// The terminal-multiplexer contract both drivers satisfy. Every method may
/// touch the socket/server; construction never does (lazy failure).
pub trait Mux: Send + Sync {
    /// Idempotent find-or-create of the window named `label`; returns it.
    fn ensure_window(&self, label: &str) -> Result<WindowInfo, MuxError>;
    /// Focus a window by id so its content is visible.
    fn focus(&self, window: &str) -> Result<(), MuxError>;
    /// Print `text` into the agent window's shell (shell-quoted by the driver).
    fn send_text(&self, text: &str) -> Result<(), MuxError>;
    /// Run `command` verbatim in the agent window's shell.
    fn run_command(&self, command: &str) -> Result<(), MuxError>;
    /// Close a window by id.
    fn close_window(&self, window: &str) -> Result<(), MuxError>;
    /// All windows.
    fn list_windows(&self) -> Result<Vec<WindowInfo>, MuxError>;
    /// All panes.
    fn list_panes(&self) -> Result<Vec<PaneInfo>, MuxError>;
    /// Shell snippet to attach a new client to `session` (for the display
    /// xterm); the caller runs it with `exec`.
    fn attach_shell(&self, session: &str) -> Result<String, MuxError>;
    /// The client terminal size (cols×rows) that shows the *entire* `session`,
    /// or `None` when the backend cannot determine it (the caller falls back to
    /// the configured `TV_TERMINAL`). Best-effort: a driver returns `Ok(None)`
    /// rather than an error for a session it cannot size (server down,
    /// unparseable output), so auto-detection never blocks a mirror that would
    /// otherwise attach. herdr must add its own UI chrome to the pane area
    /// (observed live: `x:4,y:1`), so the returned size already includes it.
    fn session_size(&self, session: &str) -> Result<Option<(u32, u32)>, MuxError>;
}

/// Build the driver selected by the `MUX` env value (default `herdr`). This
/// never touches the socket/server — the first command discovers a missing
/// socket and fails there, not here.
pub fn open(
    mux: &str,
    runner: Arc<dyn Runner>,
    session: &str,
    socket: &str,
    workspace: &str,
    agent_label: &str,
) -> Result<Box<dyn Mux>, MuxError> {
    match MuxKind::parse(mux)? {
        MuxKind::Herdr => Ok(Box::new(herdr::HerdrMux::new(
            runner,
            socket,
            workspace,
            agent_label,
        ))),
        MuxKind::Tmux => Ok(Box::new(tmux::TmuxMux::new(runner, session, agent_label))),
    }
}

/// Escape `text` for use inside a single-quoted shell word: control characters
/// are stripped except `\n`/`\t`, a literal `'` becomes `'\''`, and the whole
/// thing is wrapped in `'…'`. The one place arbitrary agent text becomes safe
/// to hand to a window's shell (spec-03 R4).
pub fn shell_single_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        if ch.is_control() && ch != '\n' && ch != '\t' {
            continue;
        }
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
