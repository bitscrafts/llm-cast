//! mirror — operator binary: run the whole cast path (part 6).
//!
//! Usage: mirror --source <file> [--bind A:P] [--size WxH] [--outdir DIR]
//!               [--encoder x264|vaapi] [--device IP] [--url-base URL]
//!               [--no-cast]
//!
//! Reads the tmux/herdr `pipe-pane` output file, renders the pane to RGBA
//! at 8 px/cell, encodes it to HLS, serves it over HTTP, and (with the
//! `cast` feature + `--device`) loads the live stream onto a Chromecast.
//! Default features run the dry-run path (NullEncoder + MapStore) so the
//! serving leg is verifiable in-container: `curl http://127.0.0.1:8080/
//! live.m3u8` returns 200.
//!
//! Exit codes: 0 clean run / --help; 2 usage error; non-zero on fatal
//! startup errors. A cast failure is logged and never fatal (R11) — mirror
//! keeps serving until it is stopped.

use std::env;
use std::process::ExitCode;
use std::sync::Arc;

use cast_tv_terminal::capture::bridge::Bridge;
use cast_tv_terminal::capture::pipe::PipeSource;
use cast_tv_terminal::emu::Emulator;
use cast_tv_terminal::pipeline::{Pipeline, PipelineConfig};
use cast_tv_terminal::serve::server;
use cast_tv_terminal::serve::store::MediaStore;

#[cfg(feature = "cast")]
use cast_tv_terminal::cast::{DeviceAddr, Sender};
#[cfg(feature = "gstreamer")]
use cast_tv_terminal::encode::pipe::GstEncoder;
#[cfg(not(feature = "gstreamer"))]
use cast_tv_terminal::encode::pipe::NullEncoder;
#[cfg(feature = "gstreamer")]
use cast_tv_terminal::serve::store::DirStore;
#[cfg(not(feature = "gstreamer"))]
use cast_tv_terminal::serve::store::MapStore;

/// Default grid size: 160 cols × 45 rows → 1280×360 px canvas.
const DEFAULT_W: u16 = 160;
const DEFAULT_H: u16 = 45;
/// Default bind address.
const DEFAULT_BIND: &str = "127.0.0.1:8080";
/// Encode framerate used by the GStreamer pipeline.
#[cfg(feature = "gstreamer")]
const FPS: u32 = 10;

fn print_usage() {
    println!("usage: mirror --source <file> [--bind A:P] [--size WxH] [--outdir DIR]");
    println!("              [--encoder x264|vaapi] [--device IP] [--url-base URL] [--audio-source <fragment>] [--no-cast]");
    println!();
    println!("  --source <file>   tmux/herdr pipe-pane output file (required)");
    println!("  --bind A:P        HTTP listen address (default {DEFAULT_BIND})");
    println!("  --size WxH        grid size in cells (default {DEFAULT_W}x{DEFAULT_H})");
    println!("  --outdir DIR      HLS output dir (required with the gstreamer feature)");
    println!("  --encoder ENC     x264 (default) or vaapi (gstreamer feature only)");
    println!("  --device IP       Chromecast to load the stream onto (cast feature)");
    println!("  --url-base URL    public stream URL, e.g. http://<LAN-IP>:8080/live.m3u8");
    println!(
        "  --audio-source <fragment>  GStreamer audio launch fragment (gstreamer feature only)"
    );
    println!("  --no-cast         skip the cast leg entirely");
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    // Manual arg parse (castctl style — no clap).
    let mut source: Option<String> = None;
    let mut bind_host = "127.0.0.1".to_string();
    let mut bind_port: u16 = 8080;
    let mut width = DEFAULT_W;
    let mut height = DEFAULT_H;
    let mut outdir: Option<String> = None;
    let mut encoder = "x264".to_string();
    let mut device: Option<String> = None;
    let mut url_base: Option<String> = None;
    let mut no_cast = false;
    // spec-06 part 2: --audio-source <fragment>. Free-form GStreamer launch
    // fragment forwarded to the encoder seam (part 1). None → silent AAC.
    // On a default-features build (NullEncoder) the flag is accepted-but-inert.
    let mut audio_source: Option<String> = None;

    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--source" => source = iter.next().map(|s| s.to_string()),
            "--bind" => {
                let Some(value) = iter.next() else {
                    eprintln!("mirror: --bind needs a value");
                    return ExitCode::from(2);
                };
                let Some((host, port)) = value.rsplit_once(':') else {
                    eprintln!("mirror: --bind must be HOST:PORT, got {value}");
                    return ExitCode::from(2);
                };
                let Ok(port) = port.parse::<u16>() else {
                    eprintln!("mirror: --bind port must be 1-65535, got {port}");
                    return ExitCode::from(2);
                };
                bind_host = host.to_string();
                bind_port = port;
            }
            "--size" => {
                let Some(value) = iter.next() else {
                    eprintln!("mirror: --size needs a value");
                    return ExitCode::from(2);
                };
                let Some((w, h)) = value.split_once('x') else {
                    eprintln!("mirror: --size must be WxH, got {value}");
                    return ExitCode::from(2);
                };
                let (Ok(w), Ok(h)) = (w.parse::<u16>(), h.parse::<u16>()) else {
                    eprintln!("mirror: --size must be WxH with 1-65535 dims, got {value}");
                    return ExitCode::from(2);
                };
                width = w.max(1);
                height = h.max(1);
            }
            "--outdir" => outdir = iter.next().map(|s| s.to_string()),
            "--encoder" => {
                let Some(value) = iter.next() else {
                    eprintln!("mirror: --encoder needs a value");
                    return ExitCode::from(2);
                };
                if value != "x264" && value != "vaapi" {
                    eprintln!("mirror: --encoder must be x264 or vaapi, got {value}");
                    return ExitCode::from(2);
                }
                encoder = value.to_string();
            }
            "--device" => device = iter.next().map(|s| s.to_string()),
            "--url-base" => url_base = iter.next().map(|s| s.to_string()),
            "--audio-source" => {
                // R1: missing value → usage error (exit 2), consistent with
                // --source/--bind/--size. `--no-cast` after a bare
                // `--audio-source` is NOT consumed as the fragment: an
                // argument starting with `--` is treated as a missing value
                // so operators don't silently swallow the next flag.
                let next = iter.next();
                let is_flag = next.as_ref().is_some_and(|s| s.starts_with("--"));
                let Some(value) = next.filter(|_| !is_flag) else {
                    eprintln!("mirror: --audio-source needs a value");
                    return ExitCode::from(2);
                };
                audio_source = Some(value.to_string());
            }
            "--no-cast" => no_cast = true,
            other => {
                eprintln!("mirror: unknown argument: {other}");
                return ExitCode::from(2);
            }
        }
    }

    let source_path = match source {
        Some(path) => path,
        None => {
            eprintln!("mirror: --source <file> is required");
            return ExitCode::from(2);
        }
    };

    // The URL the HLS stream is served at — what the cast LOAD targets.
    let url = match &url_base {
        Some(base) => base.clone(),
        None => {
            let wildcard = bind_host == "0.0.0.0" || bind_host == "::" || bind_host.is_empty();
            if device.is_some() && !no_cast && wildcard {
                eprintln!(
                    "mirror: --url-base is required when binding a wildcard host and casting"
                );
                return ExitCode::from(2);
            }
            format!("http://{bind_host}:{bind_port}/live.m3u8")
        }
    };

    // Capture side: the pipe-pane output file.
    let pipe = match PipeSource::open(&source_path) {
        Ok(pipe) => pipe,
        Err(e) => {
            eprintln!("mirror: cannot open source {source_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let emu = Emulator::with_size(width, height);
    let bridge = Bridge::new(pipe, emu);

    // The artifact store the HTTP server reads from.
    #[cfg(feature = "gstreamer")]
    let store: Arc<dyn MediaStore> = {
        let dir = match &outdir {
            Some(dir) => dir.clone(),
            None => {
                eprintln!("mirror: --outdir DIR is required with the gstreamer feature");
                return ExitCode::from(2);
            }
        };
        Arc::new(DirStore::new(dir))
    };

    #[cfg(not(feature = "gstreamer"))]
    let store: Arc<dyn MediaStore> = {
        let _ = &outdir;
        Arc::new(MapStore::seeded(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n#EXTINF:2.0,\nseg0.ts\n#EXT-X-ENDLIST\n",
            "seg0.ts",
            b"DRY-RUN-SEGMENT-0000000000000001".to_vec(),
        ))
    };

    // The pipeline: capture → emu → rasterize → encode.
    #[cfg(feature = "gstreamer")]
    let mut pipeline = {
        let outdir = match outdir {
            Some(dir) => dir,
            None => {
                eprintln!("mirror: --outdir DIR is required with the gstreamer feature");
                return ExitCode::from(2);
            }
        };
        if let Err(e) = std::fs::create_dir_all(std::path::Path::new(&outdir).join("segment")) {
            eprintln!("mirror: cannot create {outdir}/segment: {e}");
            return ExitCode::FAILURE;
        }
        // Absolute segment URLs: ROOT = url-base with /live.m3u8 → /segment,
        // so the device fetches http://host:8080/segment/seg_00000.ts.
        let root = match url.strip_suffix("/live.m3u8") {
            Some(prefix) => format!("{prefix}/segment"),
            None => url.clone(),
        };
        let gst = match GstEncoder::new(
            &encoder,
            width as usize * 8,
            height as usize * 8,
            FPS,
            &outdir,
            &root,
            url.clone(),
            // spec-06 part 2: forward the operator-supplied audio launch
            // fragment to the encoder seam (part 1). None → silent AAC.
            audio_source.as_deref(),
        ) {
            Ok(gst) => gst,
            Err(e) => {
                eprintln!("mirror: encoder init failed: {e}");
                return ExitCode::FAILURE;
            }
        };
        // Keepalive cadence = one frame per declared FPS: when the pane is
        // static the coordinator still submits the last screen at this rate,
        // so the HLS timeline stays continuous (10 fps) instead of collapsing
        // to a 1 fps gap-every-second stream that the player can't sustain.
        let config = PipelineConfig {
            keepalive_ms: 1000 / FPS as u64,
            ..Default::default()
        };
        Pipeline::new(bridge, gst, config)
    };

    #[cfg(not(feature = "gstreamer"))]
    let mut pipeline = {
        // R4: --audio-source is accepted-but-inert on a default-features
        // build (NullEncoder has no GStreamer audio leg). Reference it so
        // the parsed value isn't dead code; it is intentionally unused here.
        let _ = &audio_source;
        let _ = (&outdir, &encoder);
        Pipeline::new(
            bridge,
            NullEncoder::new(url.clone()),
            PipelineConfig::default(),
        )
    };

    // HTTP: one tokio runtime drives the HLS server, the shutdown signal,
    // and (on its own thread) the pipeline. The listener is bound *inside*
    // the runtime — tokio rejects std sockets created outside it, and
    // `from_std` of such a socket panics at runtime.
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("mirror: cannot create tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match rt.block_on(tokio::net::TcpListener::bind((
        bind_host.as_str(),
        bind_port,
    ))) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("mirror: cannot bind {bind_host}:{bind_port}: {e}");
            return ExitCode::FAILURE;
        }
    };
    rt.spawn(async move {
        server::serve_hls(store, listener).await;
    });
    println!("mirror: serving HLS at {url}");

    // Cast leg: once, after the server is up; failure is non-fatal (R11).
    if let Some(ip) = &device {
        if no_cast {
            println!("mirror: --no-cast: skipping the cast leg");
        } else {
            cast_to(ip, &url);
        }
    }

    println!("mirror: pipeline running (Ctrl-C to stop)");
    // The pipeline loop sleeps synchronously; drive it on its own thread so
    // the main thread keeps `rt` polled — the HLS server task only makes
    // progress while the runtime is being driven.
    let pipeline_thread = std::thread::spawn(move || pipeline.run());
    rt.block_on(async {
        tokio::signal::ctrl_c().await.ok();
    });
    let _ = pipeline_thread.join();
    println!("mirror: stopped");
    ExitCode::SUCCESS
}

/// Send one HLS LOAD to the device; any failure is logged, never fatal.
fn cast_to(ip: &str, url: &str) {
    #[cfg(feature = "cast")]
    {
        let ip = ip.to_string();
        println!("mirror: loading {url} onto {ip}:8009");
        let mut sender = Sender::new(Box::new(move || Ok(DeviceAddr::new(ip.clone()))));
        match sender.send_load(url) {
            Ok(()) => println!("mirror: cast load sent"),
            Err(e) => eprintln!("mirror: cast failed (non-fatal, serving continues): {e}"),
        }
    }
    #[cfg(not(feature = "cast"))]
    {
        let _ = (ip, url);
        println!(
            "mirror: built without the cast feature — no session will be sent; rebuild with --features cast"
        );
    }
}
