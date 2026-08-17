//! MCP-over-stdio server (spec-03). Part 1 declared the seams; part 2
//! delivers the server surface on top of them: the [`McpServer`] struct, the
//! seven tools, and `serve_stdio()`. This is the ONLY file in `src/mcp` that
//! imports rmcp (R1/R10). Every operational failure becomes an `is_error`
//! tool result; only genuinely unexpected internal state maps to a JSON-RPC
//! error. A failing tool never terminates the server.

pub mod cast;
pub mod config;
pub mod display;
pub mod errors;
pub mod runner;
pub mod status;

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData};
use rmcp::schemars;
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};

use crate::mux::Mux;

use self::cast::{CastPort, CastUrlArgs};
use self::config::Config;
use self::errors::McpServerError;
use self::runner::Runner;

/// The MCP server: the seven tools plus the stdio entrypoint wiring. Holds
/// every seam as an `Arc` so tests inject fakes and the bin injects
/// production implementations.
#[derive(Clone)]
pub struct McpServer {
    pub(crate) config: Arc<Config>,
    pub(crate) runner: Arc<dyn Runner>,
    pub(crate) mux: Arc<dyn Mux>,
    pub(crate) cast_port: CastPort,
}

/// Tool arguments for `cast_url`.
#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
pub struct CastUrlParams {
    /// URL of the media to cast (HLS playlist, MP4, image, ...).
    pub url: String,
    /// Content type of the media, e.g. `application/vnd.apple.mpegurl`.
    #[serde(rename = "contentType")]
    pub content_type: String,
}

/// Tool arguments for `cast_text`.
#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
pub struct CastTextParams {
    /// The text to show on the TV.
    pub text: String,
}

/// Tool arguments for `run_command`.
#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
pub struct RunCommandParams {
    /// The command to run verbatim in the agent window's shell.
    pub command: String,
}

/// Tool arguments for `set_font_size`.
#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
pub struct SetFontSizeParams {
    /// The font size in points; must be in 6..=32.
    pub pts: i32,
}

/// Tool arguments for `restore`.
#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
pub struct RestoreParams {
    /// Spawn a fresh detached cycle loop (default true).
    #[serde(rename = "restartCycle")]
    pub restart_cycle: bool,
}

/// Tool arguments for `mirror_session`.
#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
pub struct MirrorSessionParams {
    /// The session name to mirror on the TV.
    pub session: String,
    /// Optional window to focus before the relaunch.
    pub window: Option<String>,
}

impl McpServer {
    /// Wire the seams into a server. Tests inject fakes; the bin injects the
    /// production implementations.
    pub fn new(
        config: Arc<Config>,
        runner: Arc<dyn Runner>,
        mux: Arc<dyn Mux>,
        cast_port: CastPort,
    ) -> Self {
        Self {
            config,
            runner,
            mux,
            cast_port,
        }
    }

    /// R1 — serve the MCP protocol over stdio until the client disconnects.
    /// stdout carries only the protocol; everything else goes to the stderr
    /// logger installed by the entrypoint (N4).
    pub async fn serve_stdio(self) -> Result<(), McpServerError> {
        let running = self
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| McpServerError::Internal(format!("mcp server failed to start: {e}")))?;
        running
            .waiting()
            .await
            .map_err(|e| McpServerError::Internal(format!("mcp server task failed: {e}")))?;
        Ok(())
    }

    /// R4 — find-or-create the agent window idempotently, send the escaped
    /// text into it (quoting is the driver's job, via part 1's
    /// `shell_single_quote`), then focus it so the text is on the TV.
    fn cast_text_impl(&self, text: &str) -> Result<String, McpServerError> {
        let window = self.mux.ensure_window(&self.config.mux_agent_label)?;
        self.mux.send_text(text)?;
        self.mux.focus(&window.id)?;
        Ok(format!("text sent to window '{}' on the TV", window.id))
    }

    /// R5 — run the command verbatim (never escaped) in the agent window and
    /// focus it; its output appears on the TV.
    fn run_command_impl(&self, command: &str) -> Result<String, McpServerError> {
        let window = self.mux.ensure_window(&self.config.mux_agent_label)?;
        self.mux.run_command(command)?;
        self.mux.focus(&window.id)?;
        Ok(format!("command running in window '{}'", window.id))
    }
}

#[tool_router(vis = "pub")]
impl McpServer {
    /// R3 — load a media URL onto the configured Chromecast via the cast
    /// port. The tool is always listed; a build without the `cast` feature
    /// gets an `is_error` result naming the missing feature, never a crash.
    #[tool(
        description = "Cast a media URL (HLS playlist, MP4, or image) to the TV via the configured Chromecast"
    )]
    pub async fn cast_url(
        &self,
        Parameters(args): Parameters<CastUrlParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let port_args = CastUrlArgs {
            url: args.url,
            content_type: args.content_type,
        };
        match (self.cast_port)(port_args) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) => tool_error(error),
        }
    }

    /// R4 — show agent text on the TV through the dedicated `agent` window.
    #[tool(description = "Show agent text on the TV in the dedicated agent window")]
    pub async fn cast_text(
        &self,
        Parameters(args): Parameters<CastTextParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.cast_text_impl(&args.text) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) => tool_error(error),
        }
    }

    /// R5 — run a shell command whose output appears on the TV.
    #[tool(
        description = "Run a shell command verbatim in the agent window; its output appears on the TV"
    )]
    pub async fn run_command(
        &self,
        Parameters(args): Parameters<RunCommandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.run_command_impl(&args.command) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) => tool_error(error),
        }
    }

    /// R6 — relaunch the framebuffer xterm with a new font size.
    #[tool(description = "Relaunch the display xterm with a new font size in points (6..=32)")]
    pub async fn set_font_size(
        &self,
        Parameters(args): Parameters<SetFontSizeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.set_font_size_impl(args.pts) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) => tool_error(error),
        }
    }

    /// R7 — one JSON text block describing the live pipeline state.
    #[tool(description = "Report the live TV pipeline state as one JSON text block")]
    pub async fn pipeline_status(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            self.pipeline_status_json(),
        )]))
    }

    /// R8 — return the TV to the cycling view.
    #[tool(
        description = "Restore the TV to the cycling view; restart the cycle loop unless disabled"
    )]
    pub async fn restore(
        &self,
        Parameters(args): Parameters<RestoreParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.restore_impl(args.restart_cycle) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) => tool_error(error),
        }
    }

    /// R9 — mirror any running terminal session on the TV.
    #[tool(description = "Mirror a running terminal session on the TV via the display xterm")]
    pub async fn mirror_session(
        &self,
        Parameters(args): Parameters<MirrorSessionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.mirror_session_impl(&args.session, args.window.as_deref()) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) => tool_error(error),
        }
    }
}

/// R10 — the server handler. `#[tool_handler]` fills in `call_tool`,
/// `list_tools`, `get_tool` and `get_info` (name `cast-tv-terminal`, the
/// crate version) from the router above.
#[tool_handler(name = "cast-tv-terminal", version = "0.1.0")]
impl ServerHandler for McpServer {}

/// R10 — operational failures (mux/cast/runner/args/config) become ordinary
/// `is_error` tool results the client renders; only genuinely unexpected
/// internal state maps to a JSON-RPC internal error.
fn tool_error(error: McpServerError) -> Result<CallToolResult, ErrorData> {
    match error {
        McpServerError::Internal(detail) => Err(ErrorData::internal_error(
            format!("internal error: {detail}"),
            None,
        )),
        other => Ok(CallToolResult::error(vec![ContentBlock::text(
            other.to_string(),
        )])),
    }
}
