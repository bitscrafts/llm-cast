//! MCP-over-stdio server entrypoint (spec-03 R1). stdout carries ONLY the
//! JSON-RPC protocol; every diagnostic goes to stderr through a dedicated
//! logger (N4). Wiring: `Config::from_env()`, `mux::open()`, the production
//! cast port, then `serve_stdio()`.

use std::io::Write;
use std::sync::Arc;

use cast_tv_terminal::mcp::cast::production_cast_port;
use cast_tv_terminal::mcp::config::Config;
use cast_tv_terminal::mcp::runner::{ProcRunner, Runner};
use cast_tv_terminal::mcp::McpServer;
use cast_tv_terminal::mux::{self, Mux};

/// Stderr-only logger: the server process must never write to stdout outside
/// the MCP protocol stream.
struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let _ = writeln!(std::io::stderr(), "[{}] {}", record.level(), record.args());
    }

    fn flush(&self) {}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logger: &'static StderrLogger = Box::leak(Box::new(StderrLogger));
    log::set_logger(logger).map_err(|e| format!("logger init failed: {e}"))?;
    log::set_max_level(log::LevelFilter::Info);

    let config = Arc::new(Config::from_env());
    let runner: Arc<dyn Runner> = Arc::new(ProcRunner::new());
    let mux: Arc<dyn Mux> = Arc::from(mux::open(
        &config.mux,
        runner.clone(),
        &config.mux_session,
        &config.mux_socket,
        &config.mux_workspace,
        &config.mux_agent_label,
    )?);
    let cast_port = production_cast_port(config.cast_device.clone());
    let server = McpServer::new(config.clone(), runner, mux, cast_port);
    log::info!(
        "mcp-server ready: mux={} session={}",
        config.mux,
        config.mux_session
    );
    server.serve_stdio().await?;
    Ok(())
}
