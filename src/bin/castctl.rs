//! castctl — operator smoke-test binary for the milestone-1 device test.
//!
//! Usage: castctl [--ping] [--image] [--type CONTENT-TYPE] <device-ip> <url>
//!
//! Uses the given device address (port 8009), launches the Default Media
//! Receiver (CC1AD845) and sends the Cast v2 media/load for the URL. The
//! stream type is derived from the content type: `image/*` → NONE,
//! `video/mp4` → BUFFERED, anything `*mpegurl*`/m3u8 → LIVE.
//!
//! The ladder for the milestone-2 operator test: `--image` (a single JPEG —
//! the simplest possible cast, proves the cast leg before any video), then a
//! small MP4 (`--type video/mp4`), then HLS.
//! Prints a clear PASS/FAIL line and exits non-zero on failure.
//!
//! Built without the `cast` feature it compiles but sends no session.
//!
//! `--ping` flag: checks TCP reachability to device:port 8009 with 3s timeout.
//! Prints "reachable" (exit 0) or "unreachable: <err>" (exit 1).

use std::env;
use std::process::ExitCode;
use std::time::Duration;

use cast_tv_terminal::bin::ping_device;
use cast_tv_terminal::cast::DeviceAddr;

/// Map a MIME content type to the Cast stream type the DMR expects.
/// Cast-only: without the feature the binary can never load media, so the
/// helper (and its callers) are compiled out of the default build — keeping
/// `cargo clippy -- -D warnings` free of dead-code lints.
#[cfg(feature = "cast")]
fn stream_type_for(content_type: &str) -> &'static str {
    if content_type.starts_with("image/") {
        "NONE"
    } else if content_type == "video/mp4" {
        "BUFFERED"
    } else {
        "LIVE" // HLS playlists (application/vnd.apple.mpegurl, application/x-mpegURL)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    // Check for --ping flag first (works standalone, no url needed)
    if let Some(first) = args.get(1) {
        if first == "--ping" {
            if let Some(ip) = args.get(2) {
                let reachable = ping_device(ip, 8009, Duration::from_secs(3));
                return if reachable {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                };
            } else {
                eprintln!("usage: castctl --ping <device-ip>");
                return ExitCode::from(2);
            }
        }
    }

    // Parse: [--image] [--type CT] <device-ip> <url>
    // Canonical HLS type by default — the DMR rejects the legacy
    // application/x-mpegURL for a custom-sender LOAD (confirmed on-device).
    let mut content_type = "application/vnd.apple.mpegurl".to_string();
    let mut ip: Option<String> = None;
    let mut url: Option<String> = None;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--image" => content_type = "image/jpeg".to_string(),
            "--ping" => {
                // Handled above
            }
            "--type" => match iter.next() {
                Some(ct) => content_type = ct.clone(),
                None => {
                    eprintln!("castctl: --type needs a value");
                    return ExitCode::from(2);
                }
            },
            other => {
                if ip.is_none() {
                    ip = Some(other.to_string());
                } else if url.is_none() {
                    url = Some(other.to_string());
                } else {
                    eprintln!("castctl: unexpected argument: {other}");
                    return ExitCode::from(2);
                }
            }
        }
    }
    let (Some(ip), Some(url)) = (ip, url) else {
        eprintln!("usage: castctl [--ping] [--image] [--type CONTENT-TYPE] <device-ip> <url>");
        return ExitCode::from(2);
    };

    // castctl talks to the device directly via the session (no discovery —
    // the operator supplies the address); `Sender` stays mirror's abstraction.
    let device = DeviceAddr::new(ip);

    #[cfg(feature = "cast")]
    {
        let stream_type = stream_type_for(&content_type);
        println!(
            "castctl: loading {content_type} ({stream_type}) {url} onto {}:8009",
            device.host
        );
        // Parse the derived stream type back into rust_cast's enum.
        let st = match stream_type {
            "NONE" => cast_tv_terminal::cast::session::StreamType::None,
            "BUFFERED" => cast_tv_terminal::cast::session::StreamType::Buffered,
            _ => cast_tv_terminal::cast::session::StreamType::Live,
        };
        let result =
            cast_tv_terminal::cast::session::send_media_load(&device, &url, &content_type, st);
        match result {
            Ok(()) => println!("castctl: PASS — media load sent for {url}"),
            Err(e) => {
                eprintln!("castctl: FAIL — {e}");
                return ExitCode::FAILURE;
            }
        }
        ExitCode::SUCCESS
    }

    #[cfg(not(feature = "cast"))]
    {
        // Consume the parsed content type so the default build has no
        // unused-assignment lints (the value is only meaningful with cast).
        let _ = (&device, &url, &content_type);
        println!(
            "castctl: built without the cast feature — no session will be sent; rebuild with --features cast"
        );
        ExitCode::FAILURE
    }
}
