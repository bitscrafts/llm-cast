//! spec-03 part 1 — mux module (dual driver), process/config/cast seams.
//!
//! Every test except `test_runner_removes_herdr_env` uses the scripted
//! `FakeRunner`; the acceptance test exercises the real `ProcRunner` against
//! the process environment (N4/N5).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cast_tv_terminal::mcp::cast::{
    production_cast_port, stream_type_for, CastPort, CastUrlArgs, StreamKind,
};
use cast_tv_terminal::mcp::config::Config;
use cast_tv_terminal::mcp::errors::McpServerError;
use cast_tv_terminal::mcp::runner::{CommandOutcome, ProcRunner, Runner};
use cast_tv_terminal::mcp::{
    CastUrlParams, McpServer, MirrorSessionParams, RestoreParams, SetFontSizeParams,
};
use cast_tv_terminal::mux::herdr::HerdrMux;
use cast_tv_terminal::mux::tmux::TmuxMux;
use cast_tv_terminal::mux::{open, shell_single_quote, Mux, MuxError, PaneInfo, WindowInfo};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;

/// Serializes tests that mutate the process environment (parallel test threads
/// share one process env).
static ENV_MUTEX: Mutex<()> = Mutex::new(());

// ===========================================================================
// FakeRunner — scripted outcomes + full call log
// ===========================================================================

#[derive(Debug, Clone)]
struct Call {
    argv: Vec<String>,
    env: Vec<(String, String)>,
    remove_env: Vec<String>,
}

/// Scripted runner: returns queued outcomes in order and logs every call.
struct FakeRunner {
    queue: Mutex<VecDeque<CommandOutcome>>,
    log: Mutex<Vec<Call>>,
}

impl FakeRunner {
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            log: Mutex::new(Vec::new()),
        }
    }

    fn push(&self, status: i32, stdout: &str, stderr: &str) {
        self.queue.lock().unwrap().push_back(CommandOutcome {
            status,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        });
    }

    fn calls(&self) -> Vec<Call> {
        self.log.lock().unwrap().clone()
    }
}

impl Runner for FakeRunner {
    fn run(
        &self,
        argv: &[&str],
        env: &[(String, String)],
        remove_env: &[&str],
    ) -> Result<CommandOutcome, McpServerError> {
        self.log.lock().unwrap().push(Call {
            argv: argv.iter().map(|s| s.to_string()).collect(),
            env: env.to_vec(),
            remove_env: remove_env.iter().map(|s| s.to_string()).collect(),
        });
        self.queue
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| McpServerError::Internal("FakeRunner queue exhausted".to_string()))
    }

    fn spawn_detached(
        &self,
        argv: &[&str],
        env: &[(String, String)],
        remove_env: &[&str],
    ) -> Result<u32, McpServerError> {
        self.run(argv, env, remove_env)?;
        Ok(4242)
    }
}

fn herdr_driver(runner: Arc<FakeRunner>) -> HerdrMux {
    HerdrMux::new(runner, "/run/herdr/tv-demo.sock", "w1", "agent")
}

fn tmux_driver(runner: Arc<FakeRunner>) -> TmuxMux {
    TmuxMux::new(runner, "tv-demo", "agent")
}

// ===========================================================================
// R4 — shell_single_quote
// ===========================================================================

#[test]
fn test_shell_single_quote() {
    assert_eq!(shell_single_quote("it's here"), "'it'\\''s here'");
    // control chars are stripped except \n and \t
    assert_eq!(shell_single_quote("a\x00b\x1bc"), "'abc'");
    assert_eq!(shell_single_quote("x\ny\tz"), "'x\ny\tz'");
    assert_eq!(shell_single_quote(""), "''");
}

// ===========================================================================
// R2 — mux contract against BOTH drivers
// ===========================================================================

#[test]
fn test_mux_contract_both_drivers() {
    // --- list parity: the same logical windows/panes from each driver format
    let herdr = Arc::new(FakeRunner::new());
    herdr.push(
        0,
        r#"{"id":"cli:tab:list","result":{"tabs":[
            {"tab_id":"w1:t1","label":"htop"},
            {"tab_id":"w1:t2","label":"watch"}],"type":"tab_list"}}"#,
        "",
    );
    herdr.push(
        0,
        r#"{"id":"cli:pane:list","result":{"panes":[
            {"pane_id":"w1:p1","tab_id":"w1:t1"},
            {"pane_id":"w1:p2","tab_id":"w1:t2"}],"type":"pane_list"}}"#,
        "",
    );
    let h = herdr_driver(herdr);
    let hw = h.list_windows().unwrap();
    let hp = h.list_panes().unwrap();

    let tmux = Arc::new(FakeRunner::new());
    tmux.push(0, "0|htop\n1|watch", "");
    tmux.push(0, "0|%5\n1|%6", "");
    let t = tmux_driver(tmux);
    let tw = t.list_windows().unwrap();
    let tp = t.list_panes().unwrap();

    // same counts, same labels, panes map into their window
    assert_eq!(hw.len(), tw.len());
    assert_eq!(
        hw.iter().map(|w| w.label.as_str()).collect::<Vec<_>>(),
        tw.iter().map(|w| w.label.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(hp.len(), tp.len());
    assert_eq!(hp[0].window_id, hw[0].id);
    assert_eq!(hp[1].window_id, hw[1].id);
    assert_eq!(tp[0].window_id, tw[0].id);
    assert_eq!(tp[1].window_id, tw[1].id);

    // --- send_text through both drivers on an already-existing agent window
    let herdr2 = Arc::new(FakeRunner::new());
    herdr2.push(
        0,
        r#"{"id":"cli:tab:list","result":{"tabs":[
            {"tab_id":"w1:t9","label":"agent"}],"type":"tab_list"}}"#,
        "",
    );
    herdr2.push(
        0,
        r#"{"id":"cli:pane:list","result":{"panes":[
            {"pane_id":"w1:p9","tab_id":"w1:t9"}],"type":"pane_list"}}"#,
        "",
    );
    herdr2.push(0, r#"{"id":"cli:tab:focus","result":{}}"#, "");
    herdr2.push(0, r#"{"id":"cli:pane:run","result":{}}"#, "");
    let h2 = herdr_driver(herdr2.clone());
    h2.send_text("hi").unwrap();

    let tmux2 = Arc::new(FakeRunner::new());
    tmux2.push(0, "3|agent", "");
    tmux2.push(0, "{}", "");
    tmux2.push(0, "{}", "");
    let t2 = tmux_driver(tmux2.clone());
    t2.send_text("hi").unwrap();

    // per-driver argv: herdr uses tab/pane subcommands, tmux uses tmux verbs
    let h2calls = herdr2.calls();
    let t2calls = tmux2.calls();
    assert!(h2calls.iter().any(|c| c.argv == ["herdr", "tab", "list"]));
    assert!(h2calls
        .iter()
        .any(|c| c.argv == ["herdr", "pane", "run", "w1:p9", r"printf '%s\n' 'hi'"]));
    assert!(t2calls.iter().any(|c| c.argv
        == [
            "tmux",
            "list-windows",
            "-t",
            "tv-demo",
            "-F",
            "#{window_index}|#{window_name}"
        ]));
    assert!(t2calls.iter().any(|c| c.argv
        == [
            "tmux",
            "send-keys",
            "-t",
            "tv-demo:3",
            r"printf '%s\n' 'hi'",
            "Enter"
        ]));

    // --- tmux ensure_window CREATE path (empty list → new-window → re-list)
    let tmux3 = Arc::new(FakeRunner::new());
    tmux3.push(0, "", "");
    tmux3.push(0, "3: agent*", "");
    tmux3.push(0, "2|bash\n3|agent", "");
    let t3 = tmux_driver(tmux3);
    let created = t3.ensure_window("agent").unwrap();
    assert_eq!(created.id, "3");
    assert_eq!(created.label, "agent");
}

// ===========================================================================
// R2 — malformed output surfaces as Err(MuxError), never a panic
// ===========================================================================

#[test]
fn test_mux_malformed_output() {
    let herdr = Arc::new(FakeRunner::new());
    herdr.push(0, "not json at all", "");
    let h = herdr_driver(herdr);
    let err = h.list_windows().unwrap_err();
    assert!(matches!(&err, MuxError::Parse { .. }));
    assert!(err.to_string().contains("not json at all"));

    let tmux = Arc::new(FakeRunner::new());
    tmux.push(3, "", "tmux: error: can't find session: tv-demo");
    let t = tmux_driver(tmux);
    let err = t.list_windows().unwrap_err();
    assert!(matches!(&err, MuxError::Command { .. }));
    assert!(err.to_string().contains("can't find session"));
}

// ===========================================================================
// R2 regression — real herdr `pane run` prints NOTHING on success
// ===========================================================================

// Verified live against the tv-demo session (2026-08-17): `herdr pane run`
// executes the command inside the pane's terminal and emits empty stdout
// (rc=0). The fake shim masked this by returning `{"result":{}}`; send_text /
// run_command must accept empty output and judge only the exit status.
#[test]
fn test_herdr_pane_run_empty_stdout_ok() {
    let runner = Arc::new(FakeRunner::new());
    runner.push(
        0,
        r#"{"id":"cli:tab:list","result":{"tabs":[
            {"tab_id":"w1:t9","label":"agent"}],"type":"tab_list"}}"#,
        "",
    );
    runner.push(
        0,
        r#"{"id":"cli:pane:list","result":{"panes":[
            {"pane_id":"w1:p9","tab_id":"w1:t9"}],"type":"pane_list"}}"#,
        "",
    );
    runner.push(0, r#"{"id":"cli:tab:focus","result":{}}"#, "");
    runner.push(0, "", ""); // pane run: empty stdout, rc 0 — the live herdr shape
    let driver = herdr_driver(runner.clone());
    driver.send_text("hello").unwrap();
    assert!(runner
        .calls()
        .iter()
        .any(|c| { c.argv.len() >= 4 && c.argv[..4] == ["herdr", "pane", "run", "w1:p9"] }));

    // and a failing pane run still surfaces as a Command error
    let runner2 = Arc::new(FakeRunner::new());
    runner2.push(
        0,
        r#"{"id":"cli:tab:list","result":{"tabs":[
            {"tab_id":"w1:t9","label":"agent"}],"type":"tab_list"}}"#,
        "",
    );
    runner2.push(
        0,
        r#"{"id":"cli:pane:list","result":{"panes":[
            {"pane_id":"w1:p9","tab_id":"w1:t9"}],"type":"pane_list"}}"#,
        "",
    );
    runner2.push(0, r#"{"id":"cli:tab:focus","result":{}}"#, "");
    runner2.push(9, "", "pane run failed");
    let err = herdr_driver(runner2).send_text("hello").unwrap_err();
    assert!(matches!(&err, MuxError::Command { status: 9, .. }));
}

// ===========================================================================
// R2/R4 — herdr exact argv, socket env, and inherited HERDR_* stripping
// ===========================================================================

#[test]
fn test_herdr_commands_and_env() {
    let _guard = ENV_MUTEX.lock().unwrap();
    std::env::set_var("HERDR_ENV", "1");
    std::env::set_var("HERDR_SOCKET_PATH", "/ops/operator-default.sock");

    let runner = Arc::new(FakeRunner::new());
    // send_text on a fresh session: create the agent tab, discover its pane
    runner.push(
        0,
        r#"{"id":"cli:tab:list","result":{"tabs":[],"type":"tab_list"}}"#,
        "",
    );
    runner.push(0, r#"{"id":"cli:tab:create","result":{}}"#, "");
    runner.push(
        0,
        r#"{"id":"cli:tab:list","result":{"tabs":[
            {"tab_id":"w1:t9","label":"agent"}],"type":"tab_list"}}"#,
        "",
    );
    runner.push(
        0,
        r#"{"id":"cli:pane:list","result":{"panes":[
            {"pane_id":"w1:p9","tab_id":"w1:t9"}],"type":"pane_list"}}"#,
        "",
    );
    runner.push(0, r#"{"id":"cli:tab:focus","result":{}}"#, "");
    runner.push(0, r#"{"id":"cli:pane:run","result":{}}"#, "");

    let driver = herdr_driver(runner.clone());
    driver.send_text("it's here").unwrap();

    let calls = runner.calls();
    assert_eq!(calls.len(), 6);
    assert_eq!(calls[0].argv, ["herdr", "tab", "list"]);
    assert_eq!(
        calls[1].argv,
        [
            "herdr",
            "tab",
            "create",
            "--workspace",
            "w1",
            "--label",
            "agent",
            "--no-focus"
        ]
    );
    assert_eq!(calls[2].argv, ["herdr", "tab", "list"]);
    assert_eq!(calls[3].argv, ["herdr", "pane", "list"]);
    assert_eq!(calls[4].argv, ["herdr", "tab", "focus", "w1:t9"]);
    assert_eq!(calls[5].argv[..4], ["herdr", "pane", "run", "w1:p9"]);
    assert_eq!(calls[5].argv[4], r"printf '%s\n' 'it'\''s here'");

    // every call: socket env set, inherited HERDR_* keys removed
    for call in &calls {
        assert!(
            call.env.contains(&(
                "HERDR_SOCKET_PATH".to_string(),
                "/run/herdr/tv-demo.sock".to_string()
            )),
            "missing socket env in {call:?}"
        );
        assert!(
            call.remove_env.contains(&"HERDR_ENV".to_string()),
            "missing HERDR_ENV in {call:?}"
        );
        assert!(
            call.remove_env.contains(&"HERDR_SOCKET_PATH".to_string()),
            "missing HERDR_SOCKET_PATH in {call:?}"
        );
    }

    std::env::remove_var("HERDR_ENV");
    std::env::remove_var("HERDR_SOCKET_PATH");
}

// ===========================================================================
// R2 — mux::open selects a driver lazily, rejects unknown MUX values
// ===========================================================================

#[test]
fn test_mux_open_selects_driver_and_rejects_unknown() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let runner: Arc<dyn Runner> = Arc::new(FakeRunner::new());
    let err = match open("tmate", runner.clone(), "tv-demo", "/sock", "w1", "agent") {
        Ok(_) => panic!("unknown MUX must error"),
        Err(e) => e,
    };
    assert!(matches!(&err, MuxError::Config { .. }));
    assert!(err.to_string().contains("herdr"));
    assert!(err.to_string().contains("tmux"));

    // known kinds construct without touching the socket (lazy)
    let _ = open("herdr", runner.clone(), "tv-demo", "/sock", "w1", "agent").unwrap();
    let _ = open("tmux", runner.clone(), "tv-demo", "/sock", "w1", "agent").unwrap();
}

// ===========================================================================
// R3 — stream type mapping
// ===========================================================================

#[test]
fn test_stream_type_for_mapping() {
    assert_eq!(stream_type_for("image/jpeg"), StreamKind::None);
    assert_eq!(stream_type_for("IMAGE/JPEG"), StreamKind::None); // case-insensitive
    assert_eq!(stream_type_for("video/mp4"), StreamKind::Buffered);
    assert_eq!(
        stream_type_for("application/vnd.apple.mpegurl"),
        StreamKind::Live
    );
    assert_eq!(stream_type_for("video/mp2t"), StreamKind::Live);
}

// ===========================================================================
// R3 — the cast port stub without the cast feature
// ===========================================================================

#[cfg(not(feature = "cast"))]
#[test]
fn test_cast_port_stub_without_cast() {
    let port = production_cast_port("10.10.10.208".to_string());
    let err = port(CastUrlArgs {
        url: "http://10.10.10.217:18080/live.m3u8".to_string(),
        content_type: "application/vnd.apple.mpegurl".to_string(),
    })
    .unwrap_err();
    assert!(err.to_string().contains("without the cast feature"));
}

// ===========================================================================
// N6 — config defaults match the live tv-demo stack
// ===========================================================================

#[test]
fn test_config_from_env_defaults() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let keys = [
        "MUX",
        "MUX_SESSION",
        "MUX_SOCKET",
        "MUX_WORKSPACE",
        "MUX_AGENT_LABEL",
        "MUX_CYCLE_LABELS",
        "MUX_FOCUS_SECS",
        "CAST_DEVICE",
        "HLS_DIR",
        "CYCLE_PID_FILE",
        "X_DISPLAY",
        "XTERM_GEOMETRY",
    ];
    let saved: Vec<(String, Option<String>)> = keys
        .iter()
        .map(|k| (k.to_string(), std::env::var(k).ok()))
        .collect();
    for k in &keys {
        std::env::remove_var(k);
    }

    let cfg = Config::from_env();
    let home = std::env::var("HOME").unwrap_or_default();
    assert_eq!(cfg.mux, "herdr");
    assert_eq!(cfg.mux_session, "tv-demo");
    assert_eq!(
        cfg.mux_socket,
        format!("{home}/.config/herdr/sessions/tv-demo/herdr.sock")
    );
    assert_eq!(cfg.mux_workspace, "w1");
    assert_eq!(cfg.mux_agent_label, "agent");
    assert_eq!(cfg.mux_cycle_labels, "1,watch");
    assert_eq!(cfg.mux_focus_secs, 10);
    assert_eq!(cfg.cast_device, "10.10.10.208");
    assert_eq!(cfg.hls_dir, "/tmp/m2/xhls");
    assert_eq!(cfg.cycle_pid_file, "/tmp/m2/tv_cycle.pid");
    assert_eq!(cfg.x_display, ":99");
    assert_eq!(cfg.xterm_geometry, "116x32+0+0");

    for (k, v) in saved {
        match v {
            Some(v) => std::env::set_var(k, v),
            None => std::env::remove_var(&k),
        }
    }
}

// ===========================================================================
// N4/N5 (ACCEPTANCE) — the real ProcRunner strips inherited HERDR_* keys
// ===========================================================================

#[test]
fn test_runner_removes_herdr_env() {
    let _guard = ENV_MUTEX.lock().unwrap();
    std::env::set_var("HERDR_ENV", "1");
    std::env::set_var("HERDR_SOCKET_PATH", "/ops/operator-default.sock");

    let runner = ProcRunner::new();

    // run(): the child's env must not contain the inherited HERDR_* keys
    let outcome = runner.run(&["sh", "-c", "env"], &[], &[]).unwrap();
    assert!(
        !outcome.stdout.contains("HERDR_ENV="),
        "child inherited HERDR_ENV:\n{}",
        outcome.stdout
    );
    assert!(
        !outcome.stdout.contains("HERDR_SOCKET_PATH="),
        "child inherited HERDR_SOCKET_PATH:\n{}",
        outcome.stdout
    );

    // spawn_detached(): valid pid, null stdio + own process group, env stripped
    let scratch = std::env::temp_dir().join(format!("mcp_runner_env_{}", std::process::id()));
    let script = format!("env > {}", scratch.display());
    let pid = runner
        .spawn_detached(&["sh", "-c", &script], &[], &[])
        .unwrap();
    assert!(pid > 0);
    wait_for_file(&scratch);
    let env_out = std::fs::read_to_string(&scratch).unwrap();
    assert!(
        !env_out.contains("HERDR_ENV="),
        "detached child inherited HERDR_ENV:\n{env_out}"
    );
    assert!(
        !env_out.contains("HERDR_SOCKET_PATH="),
        "detached child inherited HERDR_SOCKET_PATH:\n{env_out}"
    );
    let _ = std::fs::remove_file(&scratch);

    std::env::remove_var("HERDR_ENV");
    std::env::remove_var("HERDR_SOCKET_PATH");
}

fn wait_for_file(path: &std::path::Path) {
    for _ in 0..200 {
        if let Ok(s) = std::fs::read_to_string(path) {
            if !s.is_empty() {
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

// ===========================================================================
// spec-03 part 2 — the McpServer tool surface (R1-R10). Every test uses the
// fakes; NO test spawns a real process, touches a real socket, or kills a
// real xterm (G7).
// ===========================================================================

/// FakeMux: records tool-level calls, serves scripted windows/panes and an
/// attach shell, and can be told to fail (for degradation paths).
struct FakeMux {
    windows: Mutex<Vec<WindowInfo>>,
    panes: Mutex<Vec<PaneInfo>>,
    log: Mutex<Vec<String>>,
    attach: Mutex<String>,
    fail: Mutex<bool>,
}

impl FakeMux {
    fn new() -> Self {
        Self {
            windows: Mutex::new(Vec::new()),
            panes: Mutex::new(Vec::new()),
            log: Mutex::new(Vec::new()),
            attach: Mutex::new(String::new()),
            fail: Mutex::new(false),
        }
    }

    fn set_windows(&self, windows: Vec<WindowInfo>) {
        *self.windows.lock().unwrap() = windows;
    }

    fn set_panes(&self, panes: Vec<PaneInfo>) {
        *self.panes.lock().unwrap() = panes;
    }

    fn set_fail(&self, fail: bool) {
        *self.fail.lock().unwrap() = fail;
    }

    fn calls(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl Mux for FakeMux {
    fn ensure_window(&self, label: &str) -> Result<WindowInfo, MuxError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("ensure_window:{label}"));
        Ok(WindowInfo {
            id: "w1:t9".to_string(),
            label: label.to_string(),
        })
    }

    fn focus(&self, window: &str) -> Result<(), MuxError> {
        self.log.lock().unwrap().push(format!("focus:{window}"));
        Ok(())
    }

    fn send_text(&self, text: &str) -> Result<(), MuxError> {
        self.log.lock().unwrap().push(format!("send_text:{text}"));
        Ok(())
    }

    fn run_command(&self, command: &str) -> Result<(), MuxError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("run_command:{command}"));
        Ok(())
    }

    fn close_window(&self, window: &str) -> Result<(), MuxError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("close_window:{window}"));
        Ok(())
    }

    fn list_windows(&self) -> Result<Vec<WindowInfo>, MuxError> {
        self.log.lock().unwrap().push("list_windows".to_string());
        if *self.fail.lock().unwrap() {
            return Err(MuxError::Runtime {
                detail: "scripted failure".to_string(),
            });
        }
        Ok(self.windows.lock().unwrap().clone())
    }

    fn list_panes(&self) -> Result<Vec<PaneInfo>, MuxError> {
        self.log.lock().unwrap().push("list_panes".to_string());
        if *self.fail.lock().unwrap() {
            return Err(MuxError::Runtime {
                detail: "scripted failure".to_string(),
            });
        }
        Ok(self.panes.lock().unwrap().clone())
    }

    fn attach_shell(&self, session: &str) -> Result<String, MuxError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("attach_shell:{session}"));
        let scripted = self.attach.lock().unwrap().clone();
        if scripted.is_empty() {
            Ok(format!(
                "exec herdr --session {}",
                shell_single_quote(session)
            ))
        } else {
            Ok(scripted)
        }
    }
}

/// The config the part-2 tests wire up; defaults match the live tv-demo stack
/// except the pid file, which every test overrides with a scratch path.
fn test_config() -> Config {
    Config {
        mux: "herdr".to_string(),
        mux_session: "tv-demo".to_string(),
        mux_socket: "/run/herdr/tv-demo.sock".to_string(),
        mux_workspace: "w1".to_string(),
        mux_agent_label: "agent".to_string(),
        mux_cycle_labels: "1,watch".to_string(),
        mux_focus_secs: 10,
        cast_device: "10.10.10.208".to_string(),
        hls_dir: "/tmp/m2/xhls".to_string(),
        cycle_pid_file: "/tmp/m2/tv_cycle.pid".to_string(),
        x_display: ":99".to_string(),
        xterm_geometry: "116x32+0+0".to_string(),
    }
}

/// A cast port that must never be called in this test.
fn unused_cast_port() -> CastPort {
    Arc::new(|_args: CastUrlArgs| Err(McpServerError::Cast("unused in this test".to_string())))
}

/// The success-text of a tool result, or a panic with the block when absent.
fn content_text(result: &CallToolResult) -> String {
    match &result.content[0] {
        rmcp::model::ContentBlock::Text(t) => t.text.clone(),
        other => panic!("unexpected content block: {other:?}"),
    }
}

// ===========================================================================
// R1,R2 — all seven tools registered on the router
// ===========================================================================

#[test]
fn test_tool_router_registers_all_tools() {
    let router = McpServer::tool_router();
    let names = [
        "cast_url",
        "cast_text",
        "run_command",
        "set_font_size",
        "pipeline_status",
        "restore",
        "mirror_session",
    ];
    for name in names {
        assert!(router.has_route(name), "missing tool route for {name}");
    }
    let router_tools = router.list_all();
    let listed: Vec<&str> = router_tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(listed.len(), 7, "exactly seven tools, got {listed:?}");
}

// ===========================================================================
// R3 — cast_url forwards url + content type to the CastPort closure
// ===========================================================================

/// What `cast_url` is recorded to have forwarded: `(device@port, url, ct, st)`.
type CastRecord = (String, String, String, StreamKind);

#[tokio::test]
async fn test_cast_url_forwards_to_port() {
    let config = Arc::new(test_config());
    let recorded: Arc<Mutex<Vec<CastRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let device = config.cast_device.clone();
    let recorded_capture = recorded.clone();
    let port: CastPort = Arc::new(move |args: CastUrlArgs| {
        recorded_capture.lock().unwrap().push((
            format!("{device}@8009"),
            args.url.clone(),
            args.content_type.clone(),
            stream_type_for(&args.content_type),
        ));
        Ok(format!(
            "cast requested to {device}: {} ({})",
            args.url, args.content_type
        ))
    });

    let server = McpServer::new(
        config,
        Arc::new(FakeRunner::new()),
        Arc::new(FakeMux::new()),
        port,
    );
    let result = server
        .cast_url(Parameters(CastUrlParams {
            url: "http://10.10.10.217:18080/live.m3u8".to_string(),
            content_type: "application/vnd.apple.mpegurl".to_string(),
        }))
        .await
        .unwrap();

    assert!(
        !result.is_error.unwrap_or(false),
        "must be a success result"
    );
    assert!(
        content_text(&result).contains("cast requested to"),
        "the port's success text must be surfaced: {:?}",
        result.content
    );
    let log = recorded.lock().unwrap();
    assert_eq!(log.len(), 1, "the port must be called exactly once");
    let (host, url, ct, st) = &log[0];
    assert_eq!(host, "10.10.10.208@8009");
    assert_eq!(url, "http://10.10.10.217:18080/live.m3u8");
    assert_eq!(ct, "application/vnd.apple.mpegurl");
    assert_eq!(*st, StreamKind::Live);
}

// ===========================================================================
// R6 — set_font_size relaunch sequence and range rejection
// ===========================================================================

// The guard must stay held across the `.await`: the env vars set below must
// remain stable while `set_font_size_impl` reads `herdr_env_keys()`. No
// awaited future ever locks `ENV_MUTEX` again, so this cannot deadlock.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn test_set_font_size_relaunch() {
    let _guard = ENV_MUTEX.lock().unwrap();
    std::env::set_var("HERDR_ENV", "1");
    std::env::set_var("HERDR_SOCKET_PATH", "/ops/operator-default.sock");

    let runner = Arc::new(FakeRunner::new());
    runner.push(0, "1234", ""); // pgrep -f herdr-tv → one xterm
    runner.push(0, "", ""); // kill 1234
    runner.push(0, "", ""); // xterm spawn_detached

    let server = McpServer::new(
        Arc::new(test_config()),
        runner.clone(),
        Arc::new(FakeMux::new()),
        unused_cast_port(),
    );
    let result = server
        .set_font_size(Parameters(SetFontSizeParams { pts: 15 }))
        .await
        .unwrap();
    assert!(
        !result.is_error.unwrap_or(false),
        "must be a success result"
    );
    assert!(content_text(&result).contains("font size 15"));

    let calls = runner.calls();
    assert_eq!(calls[0].argv, ["pgrep", "-f", "herdr-tv"]);
    assert_eq!(calls[1].argv, ["kill", "1234"]);
    let spawn = &calls[2];
    assert_eq!(spawn.argv[0], "xterm", "the xterm is relaunched: {spawn:?}");
    assert!(
        spawn.argv.windows(2).any(|w| w[0] == "-fs" && w[1] == "15"),
        "spawn argv must carry -fs 15: {:?}",
        spawn.argv
    );
    assert!(
        spawn.argv.iter().any(|a| a == "116x32+0+0"),
        "spawn argv must carry the geometry: {:?}",
        spawn.argv
    );
    assert!(
        spawn
            .env
            .contains(&("DISPLAY".to_string(), ":99".to_string())),
        "xterm must get DISPLAY=:99: {:?}",
        spawn.env
    );
    assert!(
        spawn.remove_env.contains(&"HERDR_ENV".to_string()),
        "HERDR_ENV must be removed from the child: {:?}",
        spawn.remove_env
    );
    assert!(
        spawn.remove_env.contains(&"HERDR_SOCKET_PATH".to_string()),
        "HERDR_SOCKET_PATH must be removed from the child: {:?}",
        spawn.remove_env
    );

    std::env::remove_var("HERDR_ENV");
    std::env::remove_var("HERDR_SOCKET_PATH");
}

#[tokio::test]
async fn test_set_font_size_rejects_range() {
    let runner = Arc::new(FakeRunner::new()); // empty queue: any call would error
    let server = McpServer::new(
        Arc::new(test_config()),
        runner.clone(),
        Arc::new(FakeMux::new()),
        unused_cast_port(),
    );
    for pts in [100, 5, 0, -1] {
        let result = server
            .set_font_size(Parameters(SetFontSizeParams { pts }))
            .await
            .unwrap();
        assert!(
            result.is_error.unwrap_or(false),
            "pts={pts} must be an is_error result"
        );
        assert!(
            content_text(&result).contains("6..=32"),
            "the error must name the valid range: {:?}",
            result.content
        );
    }
    assert!(
        runner.calls().is_empty(),
        "no side effects for out-of-range pts"
    );
}

// ===========================================================================
// R8 — restore: kill the cycle loop, respawn it, write the pid, focus
// ===========================================================================

#[tokio::test]
async fn test_restore_focus_and_cycle() {
    let pid_file = std::env::temp_dir().join(format!("mcp_cycle_{}.pid", std::process::id()));
    let _ = std::fs::remove_file(&pid_file);
    std::fs::write(&pid_file, "9999\n").unwrap();

    let mut config = test_config();
    config.cycle_pid_file = pid_file.display().to_string();

    let runner = Arc::new(FakeRunner::new());
    runner.push(0, "", ""); // kill 9999 (pid file)
    runner.push(0, "8888", ""); // pgrep -f cycle loop → one match
    runner.push(0, "", ""); // kill 8888
    runner.push(0, "", ""); // spawn_detached cycle loop

    let mux = Arc::new(FakeMux::new());
    let server = McpServer::new(
        Arc::new(config),
        runner.clone(),
        mux.clone(),
        unused_cast_port(),
    );
    let result = server
        .restore(Parameters(RestoreParams {
            restart_cycle: true,
        }))
        .await
        .unwrap();
    assert!(
        !result.is_error.unwrap_or(false),
        "must be a success result"
    );

    let calls = runner.calls();
    assert_eq!(calls.len(), 4, "kill+pgrep+kill+spawn: {calls:?}");
    assert_eq!(calls[0].argv, ["kill", "9999"], "pid file pid killed first");
    assert_eq!(
        calls[1].argv,
        ["pgrep", "-f", "herdr tab focus"],
        "pgrep fallback for the cycle loop"
    );
    assert_eq!(calls[2].argv, ["kill", "8888"]);
    assert_eq!(calls[3].argv[0..2], ["bash", "-c"]);
    let loop_cmd = &calls[3].argv[2];
    assert!(
        loop_cmd.contains("/run/herdr/tv-demo.sock"),
        "the loop must carry the socket: {loop_cmd}"
    );
    assert!(
        loop_cmd.contains("tab focus w1:t1"),
        "the loop must focus the first cycle tab: {loop_cmd}"
    );
    assert!(
        loop_cmd.contains("sleep 10"),
        "the loop must sleep: {loop_cmd}"
    );

    // the fresh pid was recorded
    let written = std::fs::read_to_string(&pid_file).unwrap();
    assert_eq!(written.trim(), "4242", "spawn_detached pid must be written");

    // the first cycle window is focused through the mux
    assert!(
        mux.calls().contains(&"focus:w1:t1".to_string()),
        "first cycle window must be focused: {:?}",
        mux.calls()
    );

    // restart_cycle=false → kill only, no spawn
    let runner2 = Arc::new(FakeRunner::new());
    runner2.push(0, "", ""); // kill 4242 (pid file)
    runner2.push(1, "", ""); // pgrep → no match (absence, not an error)
    let mux2 = Arc::new(FakeMux::new());
    let server2 = McpServer::new(
        Arc::new(test_config_with_pid(&pid_file)),
        runner2.clone(),
        mux2.clone(),
        unused_cast_port(),
    );
    let result = server2
        .restore(Parameters(RestoreParams {
            restart_cycle: false,
        }))
        .await
        .unwrap();
    assert!(
        !result.is_error.unwrap_or(false),
        "must be a success result"
    );
    assert!(
        !runner2
            .calls()
            .iter()
            .any(|c| c.argv.first().map(String::as_str) == Some("bash")),
        "restart_cycle=false must not spawn the loop"
    );
    assert!(
        mux2.calls().contains(&"focus:w1:t1".to_string()),
        "focus still happens without a respawn"
    );

    let _ = std::fs::remove_file(&pid_file);
}

fn test_config_with_pid(pid_file: &std::path::Path) -> Config {
    let mut config = test_config();
    config.cycle_pid_file = pid_file.display().to_string();
    config
}

// ===========================================================================
// R9 — mirror_session: kill the xterm, optionally focus, relaunch attached
// ===========================================================================

#[tokio::test]
async fn test_mirror_session_relaunch() {
    // herdr variant, no window arg: kill then spawn `exec herdr --session`
    let runner = Arc::new(FakeRunner::new());
    runner.push(0, "1234", ""); // pgrep -f herdr-tv
    runner.push(0, "", ""); // kill 1234
    runner.push(0, "", ""); // xterm spawn_detached
    let mux = Arc::new(FakeMux::new());
    let server = McpServer::new(
        Arc::new(test_config()),
        runner.clone(),
        mux.clone(),
        unused_cast_port(),
    );
    let result = server
        .mirror_session(Parameters(MirrorSessionParams {
            session: "demo".to_string(),
            window: None,
        }))
        .await
        .unwrap();
    assert!(
        !result.is_error.unwrap_or(false),
        "must be a success result"
    );

    let calls = runner.calls();
    assert_eq!(calls[0].argv, ["pgrep", "-f", "herdr-tv"]);
    assert_eq!(calls[1].argv, ["kill", "1234"]);
    let spawn = &calls[2].argv;
    let attach = spawn.iter().position(|a| a == "-e").unwrap();
    assert_eq!(spawn[attach], "-e");
    assert_eq!(spawn[attach + 1], "/bin/sh");
    assert_eq!(spawn[attach + 2], "-c");
    assert_eq!(
        spawn[attach + 3],
        "exec herdr --session 'demo'",
        "the attach shell comes from the herdr driver: {spawn:?}"
    );
    assert!(
        !mux.calls().iter().any(|c| c.starts_with("focus:")),
        "no window arg → no focus: {:?}",
        mux.calls()
    );

    // a window arg → the mux focuses it before the relaunch
    let runner2 = Arc::new(FakeRunner::new());
    runner2.push(1, "", ""); // pgrep → no match (absence)
    runner2.push(0, "", ""); // xterm spawn_detached
    let mux2 = Arc::new(FakeMux::new());
    let server2 = McpServer::new(
        Arc::new(test_config()),
        runner2.clone(),
        mux2.clone(),
        unused_cast_port(),
    );
    let result = server2
        .mirror_session(Parameters(MirrorSessionParams {
            session: "demo".to_string(),
            window: Some("w1:t1".to_string()),
        }))
        .await
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    assert!(
        mux2.calls().contains(&"focus:w1:t1".to_string()),
        "the target window must be focused: {:?}",
        mux2.calls()
    );

    // tmux variant: the attach shell comes from the tmux driver (readonly -r
    // baked in by part 1), never from the tool
    let runner3 = Arc::new(FakeRunner::new());
    runner3.push(0, "1234", ""); // pgrep
    runner3.push(0, "", ""); // kill
    runner3.push(0, "", ""); // spawn
    let tmux_mux: Arc<dyn Mux> = Arc::new(TmuxMux::new(runner3.clone(), "tv-demo", "agent"));
    let server3 = McpServer::new(
        Arc::new(test_config()),
        runner3.clone(),
        tmux_mux,
        unused_cast_port(),
    );
    let result = server3
        .mirror_session(Parameters(MirrorSessionParams {
            session: "demo".to_string(),
            window: None,
        }))
        .await
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let spawn = &runner3.calls()[2].argv;
    let attach = spawn.iter().position(|a| a == "-e").unwrap();
    assert_eq!(
        spawn[attach + 3],
        "exec tmux attach -t 'demo' -r",
        "tmux attach shell: {spawn:?}"
    );
}

// ===========================================================================
// R7 — pipeline_status: one JSON block, every field degrades to null
// ===========================================================================

#[tokio::test]
async fn test_pipeline_status_json() {
    let scratch = std::env::temp_dir().join(format!("mcp_status_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    // playlist with 7 lines (tail = last 5) and two segments, seg-0002 newer
    std::fs::write(
        scratch.join("live.m3u8"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\n",
    )
    .unwrap();
    let seg1 = scratch.join("seg-0001.ts");
    let seg2 = scratch.join("seg-0002.ts");
    std::fs::write(&seg1, "a").unwrap();
    std::fs::write(&seg2, "b").unwrap();
    let times = std::fs::FileTimes::new()
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60));
    std::fs::File::open(&seg2)
        .unwrap()
        .set_times(times)
        .unwrap();

    let mut config = test_config();
    config.hls_dir = scratch.display().to_string();

    let runner = Arc::new(FakeRunner::new());
    runner.push(0, "1001", ""); // Xvfb
    runner.push(
        0,
        "2002 xterm -class XTerm -fa 'DejaVu Sans Mono' -fs 13 -geometry 116x32+0+0 -T herdr-tv -e /bin/sh -c 'exec herdr --session tv-demo'",
        "",
    ); // display xterm (pgrep -af)
    runner.push(0, "3003", ""); // ffmpeg
    runner.push(0, "4004", ""); // hls_server
    runner.push(1, "", ""); // cycle loop: no match

    let mux = Arc::new(FakeMux::new());
    mux.set_windows(vec![
        WindowInfo {
            id: "w1:t1".to_string(),
            label: "htop".to_string(),
        },
        WindowInfo {
            id: "w1:t2".to_string(),
            label: "watch".to_string(),
        },
    ]);
    mux.set_panes(vec![PaneInfo {
        id: "w1:p1".to_string(),
        window_id: "w1:t1".to_string(),
    }]);

    let server = McpServer::new(Arc::new(config), runner.clone(), mux, unused_cast_port());
    let result = server.pipeline_status().await.unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let text = content_text(&result);
    let v: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("status must be valid JSON ({e}): {text}"));

    // mux section: session + scripted tabs/panes
    assert_eq!(v["mux"]["session"], "tv-demo");
    let tabs = v["mux"]["windows"].as_array().unwrap();
    assert_eq!(tabs.len(), 2, "tabs must be populated: {text}");
    assert_eq!(tabs[0]["label"], "htop");
    assert_eq!(tabs[1]["id"], "w1:t2");
    assert_eq!(v["mux"]["panes"][0]["window_id"], "w1:t1");

    // processes section
    assert_eq!(v["processes"]["xvfb"][0], "1001");
    assert_eq!(v["processes"]["display_xterm"]["pids"][0], "2002");
    assert_eq!(
        v["processes"]["display_xterm"]["font_size"], 13,
        "the xterm's current -fs must be read from its cmdline"
    );
    assert_eq!(v["processes"]["ffmpeg"][0], "3003");
    assert_eq!(v["processes"]["hls_server"][0], "4004");
    assert_eq!(
        v["processes"]["cycle_loop"],
        serde_json::Value::Null,
        "no cycle loop → null, not an error"
    );

    // hls section
    assert_eq!(v["hls"]["present"], true);
    assert_eq!(v["hls"]["playlist_present"], true);
    assert_eq!(v["hls"]["segment_count"], 2);
    assert_eq!(v["hls"]["last_segment"], "seg-0002.ts");
    assert_eq!(
        v["hls"]["playlist_tail"].as_str().unwrap(),
        "line3\nline4\nline5\nline6\nline7"
    );

    // missing pieces: an HLS dir without a playlist, a failing mux, and a
    // missing HLS dir → absent/null markers, no panic
    let empty_dir = scratch.join("empty");
    std::fs::create_dir_all(&empty_dir).unwrap();
    let mut config2 = test_config();
    config2.hls_dir = empty_dir.display().to_string();
    let runner2 = Arc::new(FakeRunner::new());
    for _ in 0..5 {
        runner2.push(1, "", ""); // every pgrep: no match
    }
    let mux2 = Arc::new(FakeMux::new());
    mux2.set_fail(true); // mux listing fails → null
    let server2 = McpServer::new(Arc::new(config2), runner2.clone(), mux2, unused_cast_port());
    let result2 = server2.pipeline_status().await.unwrap();
    let v2: serde_json::Value = serde_json::from_str(&content_text(&result2)).unwrap();
    assert_eq!(v2["hls"]["playlist_present"], false);
    assert_eq!(v2["hls"]["playlist_tail"], serde_json::Value::Null);
    assert_eq!(v2["hls"]["last_segment"], serde_json::Value::Null);
    assert_eq!(v2["hls"]["segment_count"], 0);
    assert_eq!(v2["processes"]["xvfb"], serde_json::Value::Null);
    assert_eq!(v2["mux"]["windows"], serde_json::Value::Null);
    assert_eq!(v2["mux"]["panes"], serde_json::Value::Null);

    let missing_dir = scratch.join("does-not-exist");
    let mut config3 = test_config();
    config3.hls_dir = missing_dir.display().to_string();
    let runner3 = Arc::new(FakeRunner::new());
    for _ in 0..5 {
        runner3.push(1, "", "");
    }
    let server3 = McpServer::new(
        Arc::new(config3),
        runner3.clone(),
        Arc::new(FakeMux::new()),
        unused_cast_port(),
    );
    let result3 = server3.pipeline_status().await.unwrap();
    let v3: serde_json::Value = serde_json::from_str(&content_text(&result3)).unwrap();
    assert_eq!(v3["hls"]["present"], false);
    assert_eq!(v3["hls"]["segment_count"], serde_json::Value::Null);

    let _ = std::fs::remove_dir_all(&scratch);
}

// ===========================================================================
// spec-03 part 3 — E2E over a real stdio pipe (R1, R10, N4). Both tests spawn
// the BUILT mcp-server binary against the fake herdr shim
// (tests/fixtures/fake-herdr.sh, reached via a `herdr` symlink on PATH) and a
// scratch HLS dir, then drive newline-delimited JSON-RPC over the child's
// stdin/stdout — asserting on what the MCP client actually receives, not on
// what the server thinks it sent.
// ===========================================================================

use serde_json::{json, Value};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// The absolute path of the fake herdr shim inside the repo.
fn fixture_herdr() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake-herdr.sh")
}

/// The absolute path of the built mcp-server binary (cargo sets this env var
/// for integration tests when a `[[bin]] mcp-server` exists).
fn mcp_server_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mcp-server")
}

/// Per-test scratch environment: bin dir (herdr symlink), scratch HLS dir,
/// fake socket path, and the FAKE_LOG the shim appends to. Everything lives
/// under a unique temp dir so the two tests can run in parallel.
struct E2eFixture {
    scratch: PathBuf,
    hls_dir: PathBuf,
    bin_dir: PathBuf,
    socket: PathBuf,
    fake_log: PathBuf,
}

impl E2eFixture {
    /// `fail_socket=true` requests a socket path containing the `fail` marker,
    /// which makes the shim exit non-zero on every command (R10).
    fn new(fail_socket: bool) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let scratch = std::env::temp_dir().join(format!("mcp_e2e_{}_{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let hls_dir = scratch.join("hls");
        std::fs::create_dir_all(&hls_dir).unwrap();
        // a plausible live HLS playlist + segment so the scratch dir looks alive
        std::fs::write(
            hls_dir.join("live.m3u8"),
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:2,\nseg-0001.ts\n",
        )
        .unwrap();
        std::fs::write(hls_dir.join("seg-0001.ts"), b"x").unwrap();
        let bin_dir = scratch.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let shim = fixture_herdr();
        assert!(
            shim.is_file() && is_executable(&shim),
            "fake herdr shim must exist and be executable: {}",
            shim.display()
        );
        symlink(&shim, bin_dir.join("herdr")).unwrap();
        let socket = if fail_socket {
            scratch.join("fail.sock")
        } else {
            scratch.join("fake.sock")
        };
        Self {
            scratch: scratch.clone(),
            hls_dir,
            bin_dir,
            socket,
            fake_log: scratch.join("fake-herdr.log"),
        }
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// A spawned mcp-server with stdio piped, plus a background task draining
/// stderr into a file so a verbose child can never fill the stderr pipe.
struct E2eServer {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    stderr_file: PathBuf,
}

impl E2eServer {
    async fn spawn(fx: &E2eFixture) -> Self {
        let mut path = fx.bin_dir.display().to_string();
        if let Ok(existing) = std::env::var("PATH") {
            path = format!("{path}:{existing}");
        }
        let mut child = Command::new(mcp_server_bin())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("MUX", "herdr")
            .env("MUX_SESSION", "tv-demo")
            .env("MUX_SOCKET", &fx.socket)
            .env("HLS_DIR", &fx.hls_dir)
            .env("FAKE_LOG", &fx.fake_log)
            .env("PATH", path)
            .kill_on_drop(true)
            .spawn()
            .expect("spawn mcp-server");
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");
        let mut stderr = child.stderr.take().expect("child stderr");
        let stderr_file = fx.scratch.join("server-stderr.log");
        let stderr_path = stderr_file.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf).await;
            let _ = std::fs::write(&stderr_path, buf);
        });
        Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            stderr_file,
        }
    }

    /// Write one newline-delimited JSON-RPC request and read the response
    /// line; any non-JSON byte on stdout is a protocol-corruption failure.
    async fn request(&mut self, id: u64, method: &str, params: Value) -> Result<Value, String> {
        let frame = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send_raw(&frame).await;
        self.read_response().await
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn notify(&mut self, method: &str) {
        let frame = json!({"jsonrpc": "2.0", "method": method, "params": {}});
        self.send_raw(&frame).await;
    }

    async fn send_raw(&mut self, frame: &Value) {
        let mut line = serde_json::to_string(frame).unwrap();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    /// Read exactly one line from stdout and parse it as JSON-RPC. EOF is a
    /// dead-server failure; unparseable content is a stdout-corruption
    /// failure; a read that never completes is a hang failure.
    async fn read_response(&mut self) -> Result<Value, String> {
        let mut line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            self.reader.read_line(&mut line),
        )
        .await
        .map_err(|_| {
            format!(
                "timed out waiting for a response; last stderr:\n{}",
                self.stderr_tail()
            )
        })?
        .map_err(|e| format!("read error: {e}"))?;
        if line.is_empty() {
            return Err(format!(
                "server closed stdout (EOF) — process state:\n{}",
                self.stderr_tail()
            ));
        }
        serde_json::from_str(line.trim()).map_err(|e| {
            format!(
                "stdout carried a non-JSON-RPC line {line:?} ({e}); stderr:\n{}",
                self.stderr_tail()
            )
        })
    }

    /// The process is still running (not exited).
    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// The child's stderr so far (the background drain task writes it).
    fn stderr_tail(&self) -> String {
        std::fs::read_to_string(&self.stderr_file).unwrap_or_default()
    }

    async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

/// Drive initialize → notifications/initialized → tools/list; assert the
/// handshake contract and return the tools/list response.
async fn e2e_handshake(server: &mut E2eServer) -> Value {
    let init = server
        .request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "e2e-test", "version": "0.0.1"}
            }),
        )
        .await
        .expect("initialize must be answered");
    assert_eq!(init["id"], 1, "initialize response id: {init}");
    assert!(
        init.get("error").is_none(),
        "initialize must not be a JSON-RPC error: {init}"
    );
    assert_eq!(
        init["result"]["serverInfo"]["name"], "cast-tv-terminal",
        "serverInfo.name must be cast-tv-terminal: {init}"
    );
    assert!(
        init["result"]["capabilities"]["tools"].is_object(),
        "capabilities.tools must be present: {init}"
    );

    server.notify("notifications/initialized").await;

    let list = server
        .request(2, "tools/list", json!({}))
        .await
        .expect("tools/list must be answered");
    assert_eq!(list["id"], 2, "tools/list response id: {list}");
    assert!(
        list.get("error").is_none(),
        "tools/list must not be a JSON-RPC error: {list}"
    );
    list
}

/// All seven tool names, in any order.
fn assert_all_seven_tools(list: &Value) {
    let tools = list["result"]["tools"]
        .as_array()
        .expect("tools/list result.tools must be an array");
    let mut names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool name must be a string"))
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "cast_text",
            "cast_url",
            "mirror_session",
            "pipeline_status",
            "restore",
            "run_command",
            "set_font_size"
        ],
        "tools/list must return all seven tools: {list}"
    );
}

// ===========================================================================
// R1 — the built binary over a real stdio pipe: handshake, seven tools, and a
// cast_text whose child invocations land in FAKE_LOG (N4: stdout stayed pure
// JSON-RPC or the line parser above would have failed).
// ===========================================================================

#[tokio::test]
async fn test_e2e_stdio_handshake() {
    let fx = E2eFixture::new(false);
    let mut server = E2eServer::spawn(&fx).await;

    let list = e2e_handshake(&mut server).await;
    assert_all_seven_tools(&list);

    let call = server
        .request(
            3,
            "tools/call",
            json!({"name": "cast_text", "arguments": {"text": "hello from e2e"}}),
        )
        .await
        .expect("cast_text must be answered");
    assert_eq!(call["id"], 3, "tools/call response id: {call}");
    assert!(
        call.get("error").is_none(),
        "cast_text must not be a JSON-RPC error: {call}"
    );
    assert_ne!(
        call["result"]["isError"], true,
        "cast_text must be a success result: {call}"
    );
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("cast_text content text: {call}"));
    assert!(
        text.contains("text sent to window"),
        "success text must name the window: {text}"
    );

    // the mux commands really ran: FAKE_LOG has the pane run invocation
    let log = std::fs::read_to_string(&fx.fake_log)
        .unwrap_or_else(|_| panic!("FAKE_LOG must exist at {}", fx.fake_log.display()));
    assert!(
        log.lines()
            .any(|l| l.contains("pane run") && l.contains("hello from e2e")),
        "FAKE_LOG must contain the cast_text pane run invocation:\n{log}"
    );
    assert!(
        log.lines().any(|l| l == "tab focus w1:t1"),
        "FAKE_LOG must contain the focus invocation:\n{log}"
    );

    assert!(server.is_alive(), "the server must still be alive");
    let _ = std::fs::remove_dir_all(&fx.scratch);
    server.shutdown().await;
}

// ===========================================================================
// R10 (ACCEPTANCE) — a failing tool call must yield a well-formed is_error
// result (never a JSON-RPC error, never a hang), and a subsequent tools/list
// must still be answered: the process survived. A server that println!s to
// stdout or panics on a missing socket fails this test.
// ===========================================================================

#[tokio::test]
async fn test_e2e_tool_error_keeps_server_alive() {
    let fx = E2eFixture::new(true); // socket path carries the `fail` marker
    let mut server = E2eServer::spawn(&fx).await;

    let list = e2e_handshake(&mut server).await;
    assert_all_seven_tools(&list);

    let call = server
        .request(
            3,
            "tools/call",
            json!({"name": "cast_text", "arguments": {"text": "boom"}}),
        )
        .await
        .expect("the failing cast_text must still be answered, not hang");
    assert_eq!(call["id"], 3, "tools/call response id: {call}");
    assert!(
        call.get("error").is_none(),
        "a failing tool must be a tool result, NOT a JSON-RPC error: {call}"
    );
    assert_eq!(
        call["result"]["isError"], true,
        "the failing call must carry isError=true: {call}"
    );
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("is_error content text: {call}"));
    assert!(
        text.contains("mux command failed") || text.contains("failed"),
        "the error text must describe the failure: {text}"
    );

    // the process survived: a second tools/list is answered
    let list2 = server
        .request(4, "tools/list", json!({}))
        .await
        .expect("tools/list after the failure must still be answered");
    assert_eq!(list2["id"], 4, "post-failure tools/list id: {list2}");
    assert_all_seven_tools(&list2);

    assert!(
        server.is_alive(),
        "the server process must be alive after the failed call"
    );
    let _ = std::fs::remove_dir_all(&fx.scratch);
    server.shutdown().await;
}
