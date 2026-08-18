//! Pipeline status collector (spec-03 R7): one JSON text block describing the
//! mux session, the live processes, and the HLS output. Every field degrades
//! to an absent/null marker — the collector never blocks and never panics.

use serde_json::json;

use super::display::{xterm_font_size, xvfb_resolution, XTERM_TITLE};
use super::McpServer;
use crate::cast::{DiscoveredDevice, DiscoverySource};

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
        let xvfb = self.pgrep_full("Xvfb");
        let xvfb_pids = xvfb.as_ref().map(|(pids, _)| pids.clone());
        let detected_resolution = xvfb
            .as_ref()
            .and_then(|(_, cmdlines)| xvfb_resolution(cmdlines));
        let xterm = self.pgrep_full(XTERM_TITLE).map(
            |(pids, cmdlines)| json!({ "pids": pids, "font_size": xterm_font_size(&cmdlines) }),
        );
        let ffmpeg = self.pgrep_pids("ffmpeg");
        let hls_server = self.pgrep_pids("hls_server");
        let cycle_loop = self.pgrep_pids("herdr tab focus");
        // The effective display settings (runtime overlay over the env config)
        // and the mirrored session's auto-detected size, so a client can see
        // what the display xterm will actually run at and where it came from.
        let (res_str, term_str, margin, geometry) = self.display_settings();
        let last_session = self
            .last_session_guard()
            .clone()
            .unwrap_or_else(|| self.config.mux_session.clone());
        let session_size = self.mux.session_size(&last_session).ok().flatten();
        let (session_term, session_source) = match session_size {
            Some((cols, rows)) if cols > 0 && rows > 0 => (format!("{cols}x{rows}"), "detected"),
            _ => (term_str.clone(), "config"),
        };
        let cast = cast_block(&self.config.cast_device, &self.discovered_device_guard());
        json!({
            "mux": {
                "session": self.config.mux_session,
                "socket": self.config.mux_socket,
                "windows": windows,
                "panes": panes,
            },
            "processes": {
                "xvfb": xvfb_pids,
                "display_xterm": xterm,
                "ffmpeg": ffmpeg,
                "hls_server": hls_server,
                "cycle_loop": cycle_loop,
            },
            "resolution": {
                "configured": self.config.tv_resolution.clone(),
                "xvfb": detected_resolution,
                "mismatch": detected_resolution
                    .as_deref()
                    .map(|detected| detected != self.config.tv_resolution)
                    .unwrap_or(false),
            },
            "session": {
                "name": last_session,
                "detected": session_size.map(|(cols, rows)| format!("{cols}x{rows}")),
                "effective_terminal": session_term,
                "source": session_source,
            },
            "display": {
                "resolution": res_str,
                "terminal": term_str,
                "margin": margin,
                "geometry": geometry,
            },
            "hls": hls_state(&self.config.hls_dir),
            "cast": cast,
        })
        .to_string()
    }
}

/// R6 — the `cast` status block: always present. `configured_device` is the
/// raw config string; `discovered_device` is `"host:port"` when a device was
/// resolved (null otherwise); `source` is `"mdns"`, `"config"`, or `"unknown"`
/// when no device has been resolved yet.
fn cast_block(configured_device: &str, discovered: &Option<DiscoveredDevice>) -> serde_json::Value {
    match discovered {
        Some(dev) => {
            let source = match dev.source {
                DiscoverySource::Mdns => "mdns",
                DiscoverySource::Config => "config",
            };
            json!({
                "configured_device": configured_device,
                "discovered_device": format!("{}:{}", dev.host, dev.port),
                "source": source,
            })
        }
        None => json!({
            "configured_device": configured_device,
            "discovered_device": serde_json::Value::Null,
            "source": "unknown",
        }),
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
