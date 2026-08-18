//! Binary helper functions for castctl and other operator tools.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Attempts a TCP connection to a Chromecast device to check reachability.
///
/// Connects to `<host>:<port>` with the given timeout.
/// Returns `true` if the connection succeeds (device is reachable),
/// `false` otherwise (print error to stderr).
///
/// # Arguments
/// * `host` - IP address or hostname of the Chromecast
/// * `port` - Port number (typically 8009 for Chromecast)
/// * `timeout` - Connection timeout duration
///
/// # Returns
/// * `true` if connection succeeded
/// * `false` if connection failed or timed out
pub fn ping_device(host: &str, port: u16, timeout: Duration) -> bool {
    // Resolve host:port to socket address
    let socket_addr = format!("{host}:{port}");

    match socket_addr.to_socket_addrs() {
        Ok(mut addrs) => {
            // Try the first address
            if let Some(addr) = addrs.next() {
                match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(_stream) => {
                        println!("reachable");
                        true
                    }
                    Err(e) => {
                        eprintln!("unreachable: {e}");
                        false
                    }
                }
            } else {
                eprintln!("unreachable: no address found for {host}:{port}");
                false
            }
        }
        Err(e) => {
            eprintln!("unreachable: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_ping_format_reachable() {
        // Start a listener on a random port
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let local_addr = listener.local_addr().unwrap();
        let port = local_addr.port();

        // Spawn a thread to accept connections
        thread::spawn(move || {
            let _ = listener.accept();
        });

        // Call the ping function with the reachable address
        let result = ping_device("127.0.0.1", port, Duration::from_secs(3));

        assert!(result, "Expected ping to succeed for reachable device");
    }

    #[test]
    fn test_ping_format_unreachable() {
        // Use an unroutable IP (private range that won't respond)
        let result = ping_device("10.255.255.1", 8009, Duration::from_secs(3));

        assert!(!result, "Expected ping to fail for unreachable device");
    }

    #[test]
    fn test_ping_connect_timeout_bounded() {
        let start = std::time::Instant::now();

        let result = ping_device("10.255.255.1", 8009, Duration::from_secs(3));

        let elapsed = start.elapsed();

        assert!(!result, "Expected ping to fail for unreachable device");
        // Should complete within timeout + small buffer
        assert!(
            elapsed < Duration::from_secs(5),
            "Ping took too long: {:?}",
            elapsed
        );
    }
}
