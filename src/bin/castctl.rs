//! castctl — operator smoke-test binary for the milestone-1 device test.
//!
//! Usage: castctl <device-ip> <hls-url>
//!
//! Uses the given device address (port 8009), launches the Default Media
//! Receiver (CC1AD845) and sends the Cast v2 media/load for the HLS URL.
//! Prints a clear PASS/FAIL line and exits non-zero on failure.
//!
//! Built without the `cast` feature it compiles but sends no session.

use std::env;
use std::process::ExitCode;

use cast_tv_terminal::cast::{DeviceAddr, Sender};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: castctl <device-ip> <hls-url>");
        return ExitCode::from(2);
    }
    let ip = args[1].clone();
    let url = args[2].clone();

    #[cfg(feature = "cast")]
    println!("castctl: loading HLS {url} onto {ip}:8009");

    let mut sender = Sender::new(Box::new(move || Ok(DeviceAddr::new(ip.clone()))));
    let result = sender.send_load(&url);

    #[cfg(feature = "cast")]
    {
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
        let _ = &result;
        println!(
            "castctl: built without the cast feature — no session will be sent; rebuild with --features cast"
        );
        ExitCode::FAILURE
    }
}
