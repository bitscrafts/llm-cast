//! castctl — operator smoke-test binary for the milestone-1 device test.
//!
//! Usage: castctl [--image] <device-ip> <url>
//!
//! Uses the given device address (port 8009), launches the Default Media
//! Receiver (CC1AD845) and sends the Cast v2 media/load for the URL. With
//! `--image` the URL is a single JPEG (the simplest possible cast — proves the
//! cast leg before any video); without it, an HLS stream URL.
//! Prints a clear PASS/FAIL line and exits non-zero on failure.
//!
//! Built without the `cast` feature it compiles but sends no session.

use std::env;
use std::process::ExitCode;

use cast_tv_terminal::cast::{DeviceAddr, Sender};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    // [--image] <device-ip> <url> — 3 or 4 args total.
    if args.len() != 3 && args.len() != 4 {
        eprintln!("usage: castctl [--image] <device-ip> <url>");
        return ExitCode::from(2);
    }
    let (image_mode, ip, url) = if args.len() == 4 && args[1] == "--image" {
        (true, args[2].clone(), args[3].clone())
    } else {
        (false, args[1].clone(), args[2].clone())
    };

    let mut sender = Sender::new(Box::new(move || Ok(DeviceAddr::new(ip.clone()))));

    #[cfg(feature = "cast")]
    {
        let result = if image_mode {
            println!("castctl: loading image {url} onto {ip}:8009");
            let device = DeviceAddr::new(ip);
            cast_tv_terminal::cast::session::send_image_load(&device, &url)
        } else {
            println!("castctl: loading HLS {url} onto {ip}:8009");
            sender.send_load(&url)
        };
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
        let _ = (&image_mode, &url, &mut sender);
        println!(
            "castctl: built without the cast feature — no session will be sent; rebuild with --features cast"
        );
        ExitCode::FAILURE
    }
}
