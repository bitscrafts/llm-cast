//! Process seam (spec-03 N4/N5): every subprocess the server spawns goes
//! through the [`Runner`] trait. [`ProcRunner`] strips inherited `HERDR_*`
//! keys so a child can never silently drive the operator's default herdr
//! session, and `spawn_detached` nulls stdio and sets its own process group
//! so a child can neither corrupt the MCP protocol nor die with the server.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use super::errors::McpServerError;

/// Captured stdout/stderr and exit status of a completed subprocess.
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs subprocesses for the mux drivers and display tools.
pub trait Runner: Send + Sync {
    /// Run to completion; capture stdout/stderr. `remove_env` keys are
    /// stripped, then `env` applied, before exec.
    fn run(
        &self,
        argv: &[&str],
        env: &[(String, String)],
        remove_env: &[&str],
    ) -> Result<CommandOutcome, McpServerError>;

    /// Detached spawn: stdio nulled, its own process group; returns the pid.
    fn spawn_detached(
        &self,
        argv: &[&str],
        env: &[(String, String)],
        remove_env: &[&str],
    ) -> Result<u32, McpServerError>;
}

/// The `HERDR_*` keys present in this process's environment. Both
/// [`ProcRunner`] and the herdr mux driver strip them from every child.
pub fn herdr_env_keys() -> Vec<String> {
    std::env::vars()
        .filter(|(k, _)| k.starts_with("HERDR_"))
        .map(|(k, _)| k)
        .collect()
}

/// Production [`Runner`]: `std::process::Command` with the nested-herdr env
/// keys removed and the per-call env applied.
pub struct ProcRunner {
    herdr_keys: Vec<String>,
}

impl ProcRunner {
    pub fn new() -> Self {
        Self {
            herdr_keys: herdr_env_keys(),
        }
    }
}

impl Default for ProcRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner for ProcRunner {
    fn run(
        &self,
        argv: &[&str],
        env: &[(String, String)],
        remove_env: &[&str],
    ) -> Result<CommandOutcome, McpServerError> {
        let mut cmd = build_command(argv, env, remove_env, &self.herdr_keys);
        let output = cmd.output().map_err(|e| runner_error(argv, e))?;
        Ok(CommandOutcome {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn spawn_detached(
        &self,
        argv: &[&str],
        env: &[(String, String)],
        remove_env: &[&str],
    ) -> Result<u32, McpServerError> {
        let mut cmd = build_command(argv, env, remove_env, &self.herdr_keys);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd.process_group(0);
        let child = cmd.spawn().map_err(|e| runner_error(argv, e))?;
        Ok(child.id())
    }
}

fn build_command(
    argv: &[&str],
    env: &[(String, String)],
    remove_env: &[&str],
    herdr_keys: &[String],
) -> Command {
    let mut cmd = Command::new(argv[0]);
    cmd.args(&argv[1..]);
    for key in herdr_keys {
        cmd.env_remove(key);
    }
    for key in remove_env {
        cmd.env_remove(key);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd
}

fn runner_error(argv: &[&str], source: std::io::Error) -> McpServerError {
    McpServerError::Runner {
        argv: argv.iter().map(|s| s.to_string()).collect(),
        source,
    }
}
