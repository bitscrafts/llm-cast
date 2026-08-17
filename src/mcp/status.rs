//! Pipeline status collector (spec-03 R7): one JSON text block describing the
//! mux session, the live processes, and the HLS output. Every field degrades
//! to an absent/null marker — the collector never blocks and never panics.

use serde_json::json;

use super::display::XTERM_TITLE;
use super::McpServer;

impl McpServer {
    /// R7 — collect the pipeline state into a single JSON text block:
    /// mux session + windows/panes, live processes (Xvfb, the display xterm
    /// incl. its current `-fs`, ffmpeg x11grab, hls_server, the cycle loop),
    /// and the HLS dir state.
    pub fn pipeline_status_json(&self) -> String {
        let windows = self.mux.list_windows().ok().map(|list| {
            list.iter()
                .map(|w| json!({ "id": w.id, "label": w.label }))
                .collect::<Vec<_>>()
        });
        let panes = self.mux.list_panes().ok().map(|list| {
            list.iter()
                .map(|p| json!({ "id": p.id, "window_id": p.window_id }))
                .collect::<Vec<_>>()
        });
        // Fetch order matches the JSON field order below: xvfb first, then the
        // display xterm, ffmpeg, hls_server, and the cycle loop.
        let xvfb = self.pgrep_pids("Xvfb");
        let xterm = self
            .pgrep_full(XTERM_TITLE)
            .map(|(pids, font_size)| json!({ "pids": pids, "font_size": font_size }));
        let ffmpeg = self.pgrep_pids("ffmpeg");
        let hls_server = self.pgrep_pids("hls_server");
        let cycle_loop = self.pgrep_pids("herdr tab focus");
        json!({
            "mux": {
                "session": self.config.mux_session,
                "socket": self.config.mux_socket,
                "windows": windows,
                "panes": panes,
            },
            "processes": {
                "xvfb": xvfb,
                "display_xterm": xterm,
                "ffmpeg": ffmpeg,
                "hls_server": hls_server,
                "cycle_loop": cycle_loop,
            },
            "hls": hls_state(&self.config.hls_dir),
        })
        .to_string()
    }
}

/// R7 — HLS output dir state: playlist presence + tail, segment count, newest
/// segment. Every piece degrades to an absent/null marker; a missing dir is
/// `present: false`, never an error.
fn hls_state(dir: &str) -> serde_json::Value {
    let path = std::path::Path::new(dir);
    if !path.is_dir() {
        return json!({
            "dir": dir,
            "present": false,
            "playlist_present": false,
            "segment_count": null,
            "last_segment": null,
            "playlist_tail": null,
        });
    }
    let mut playlist_present = false;
    let mut playlist_tail: Option<String> = None;
    let mut segments: Vec<(String, std::time::SystemTime)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
            if !is_file {
                continue;
            }
            if name.ends_with(".m3u8") && !playlist_present {
                playlist_present = true;
                playlist_tail = std::fs::read_to_string(entry.path()).ok().map(|content| {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = lines.len().saturating_sub(5);
                    lines[start..].join("\n")
                });
            } else if name.ends_with(".ts") {
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                segments.push((name, mtime));
            }
        }
    }
    segments.sort_by_key(|b| std::cmp::Reverse(b.1));
    let last_segment = segments.first().map(|(name, _)| name.as_str());
    json!({
        "dir": dir,
        "present": true,
        "playlist_present": playlist_present,
        "playlist_tail": playlist_tail,
        "segment_count": segments.len(),
        "last_segment": last_segment,
    })
}
