//! spec-03 part 1 — mux module (dual driver), process/config/cast seams.
//!
//! Every test except `test_runner_removes_herdr_env` uses the scripted
//! `FakeRunner`; the acceptance test exercises the real `ProcRunner` against
//! the process environment (N4/N5).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cast_tv_terminal::mcp::cast::{production_cast_port, stream_type_for, CastUrlArgs, StreamKind};
use cast_tv_terminal::mcp::config::Config;
use cast_tv_terminal::mcp::errors::McpServerError;
use cast_tv_terminal::mcp::runner::{CommandOutcome, ProcRunner, Runner};
use cast_tv_terminal::mux::herdr::HerdrMux;
use cast_tv_terminal::mux::tmux::TmuxMux;
use cast_tv_terminal::mux::{open, shell_single_quote, Mux, MuxError};

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
