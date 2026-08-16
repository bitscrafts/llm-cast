//! Cast sender (R6): builds the Cast v2 `media/load` payload and sends it to
//! a discovered Chromecast via the Default Media Receiver.
//!
//! Compiles under default features: [`Sender::build_media_load_request`] is
//! pure, and device discovery is injected so tests never touch the network.
//! The real rust_cast session is gated behind the optional `cast` feature.

use serde_json::Value;

/// Errors from the cast sender.
#[derive(Debug, thiserror::Error)]
pub enum CastError {
    /// No device was found during discovery.
    #[error("device discovery failed: {0}")]
    DiscoveryFailed(String),
    /// The discovered device could not be reached.
    #[error("device unreachable: {0}")]
    Unreachable(String),
    /// The cast session or the `media/load` request failed.
    #[error("cast session failed: {0}")]
    Session(String),
}

/// LAN address of a discovered Chromecast (listens on 8009, TLS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAddr {
    /// Host name or IP address of the device on the LAN.
    pub host: String,
    /// Port of the Cast v2 TLS endpoint.
    pub port: u16,
}

impl DeviceAddr {
    /// `host` at the standard Chromecast port 8009.
    pub fn new(host: impl Into<String>) -> Self {
        DeviceAddr {
            host: host.into(),
            port: 8009,
        }
    }
}

/// Device discovery: resolves a Chromecast on the LAN to its address, or
/// fails.
///
/// Injected as a `Box<dyn FnMut ...>` so callers (and tests) can substitute
/// any strategy — including one that always fails.
pub type Discovery = Box<dyn FnMut() -> Result<DeviceAddr, CastError>>;

/// Sends an HLS stream URL to a Chromecast (Default Media Receiver).
pub struct Sender {
    discovery: Discovery,
}

impl Sender {
    /// A sender that finds its device through `discovery`.
    pub fn new(discovery: Discovery) -> Self {
        Sender { discovery }
    }

    /// Build the Cast v2 `media/load` request body: `"type": "LOAD"` plus the
    /// HLS media descriptor under `media`.
    ///
    /// `application/vnd.apple.mpegurl` (canonical) is required — the DMR
    /// rejects the legacy `application/x-mpegURL` for a custom-sender LOAD and
    /// never fetches the manifest (confirmed on-device 2026-08-16).
    pub fn build_media_load_request(url: &str) -> Value {
        serde_json::json!({
            "type": "LOAD",
            "media": {
                "contentId": url,
                "contentType": "application/vnd.apple.mpegurl",
                "streamType": "LIVE"
            }
        })
    }

    /// Send `url` to the device for playback on the TV.
    ///
    /// Runs discovery first; any failure surfaces as [`CastError`]. The real
    /// rust_cast session + `media/load` onto the device runs behind the
    /// `cast` feature (see `session`).
    pub fn send_load(&mut self, url: &str) -> Result<(), CastError> {
        let _payload = Self::build_media_load_request(url);
        let device = (self.discovery)()?;
        #[cfg(feature = "cast")]
        {
            self.session_media_load(&device, url)?;
        }
        #[cfg(not(feature = "cast"))]
        {
            let _ = &device;
        }
        Ok(())
    }

    /// Real device session: connect via rust_cast and issue the
    /// `media/load` request (Default Media Receiver CC1AD845).
    #[cfg(feature = "cast")]
    fn session_media_load(&mut self, device: &DeviceAddr, url: &str) -> Result<(), CastError> {
        crate::cast::session::send_load(device, url)
    }
}
