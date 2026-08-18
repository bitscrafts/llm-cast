//! Tests for castctl --ping functionality
//!
//! TDD Contract from spec-05: https://github.com/lagunadoc/pi/blob/main/projects/chromecast-tv-mirror/specs/05-castctl-ping.md

use std::net::TcpListener;
use std::thread;
use std::time::Duration;

/// test_ping_format_reachable — host that connects
/// expects function returns `true`, prints "reachable"
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
    let result = cast_tv_terminal::bin::ping_device("127.0.0.1", port, Duration::from_secs(3));

    assert!(result, "Expected ping to succeed for reachable device");
}

/// test_ping_format_unreachable — unroutable IP
/// expects function returns `false`, prints "unreachable"
#[test]
fn test_ping_format_unreachable() {
    // Use an unroutable IP (private range that won't respond)
    // 10.255.255.1 is in the private range and won't have anything listening
    let result = cast_tv_terminal::bin::ping_device("10.255.255.1", 8009, Duration::from_secs(3));

    assert!(!result, "Expected ping to fail for unreachable device");
}

/// test_ping_connect_timeout_bounded — connect to an unroutable IP
/// expects completes within ~5s (bounded, not hanging)
#[test]
fn test_ping_connect_timeout_bounded() {
    // Use an IP that will timeout (private range, no service)
    let start = std::time::Instant::now();

    let result = cast_tv_terminal::bin::ping_device("10.255.255.1", 8009, Duration::from_secs(3));

    let elapsed = start.elapsed();

    assert!(!result, "Expected ping to fail for unreachable device");
    // Should complete within timeout + small buffer (3s + some overhead)
    assert!(
        elapsed < Duration::from_secs(5),
        "Ping took too long: {:?}",
        elapsed
    );
}
