//! spec-04 part 1 — self-contained mDNS discovery resolver module.
//!
//! Zero wiring: just the trait, the error type, the two infallible/fallback
//! impls, and the total [`resolve_device`] entrypoint. Parts 2 and 3 wire this
//! into the cast port and the mcp-server.
//!
//! Design: discovery is **best-effort, always**. [`resolve_device`] never
//! returns `Err` and never panics — on any mDNS failure (or when the `mdns`
//! feature is off) it logs a `warn!` and falls back to [`StaticResolver`],
//! which returns the configured host at the standard Chromecast port 8009.
//!
//! No panicking calls (unwrap/expect/panic-macro) live here (N1).

use log::warn;

/// Where a discovered device came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySource {
    /// Resolved on the LAN via mDNS (`_googlecast._tcp`).
    Mdns,
    /// Taken verbatim from configuration (the fallback).
    Config,
}

/// A device resolved by a [`DeviceResolver`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    /// Host name or IP address of the device on the LAN.
    pub host: String,
    /// Port of the Cast v2 TLS endpoint (8009 for Chromecast).
    pub port: u16,
    /// How the device was found.
    pub source: DiscoverySource,
}

/// Errors a [`DeviceResolver`] may yield. All are caught by [`resolve_device`];
/// they never escape to a caller.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DiscoveryError {
    /// The browse timed out before any service resolved.
    #[error("mDNS discovery timed out after {0}s")]
    Timeout(u64),
    /// The mDNS socket could not be opened (no multicast route, sandbox, ...).
    #[error("mDNS socket error: {0}")]
    Socket(String),
    /// No devices were found on the LAN.
    #[error("no chromecast devices found")]
    NoDevices,
}

/// Resolve a Chromecast on the LAN to a [`DiscoveredDevice`].
///
/// Object-safe so a real mDNS browser and a test fake can both be `Box<dyn>`.
pub trait DeviceResolver: Send + Sync {
    /// Run one resolution attempt. Best-effort: callers must tolerate `Err`.
    fn resolve(&self) -> Result<DiscoveredDevice, DiscoveryError>;
}

/// Infallible resolver that returns the configured host at port 8009.
///
/// This is the fallback when mDNS is unavailable, disabled, or finds nothing.
#[derive(Debug, Clone)]
pub struct StaticResolver {
    /// The configured host (IP or name) to return verbatim.
    pub host: String,
}

impl StaticResolver {
    /// A static resolver returning `host` at the standard Cast port 8009.
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into() }
    }
}

impl DeviceResolver for StaticResolver {
    fn resolve(&self) -> Result<DiscoveredDevice, DiscoveryError> {
        Ok(DiscoveredDevice {
            host: self.host.clone(),
            port: 8009,
            source: DiscoverySource::Config,
        })
    }
}

// ---------------------------------------------------------------------------
// mDNS resolver — feature-gated. Default builds pull no mDNS code (N2).
// ---------------------------------------------------------------------------

/// Browse `_googlecast._tcp` on the LAN via `mdns-sd` and resolve a device.
///
/// Only compiled when the `mdns` feature is enabled. On any error, timeout, or
/// empty LAN, [`resolve_device`] falls back to [`StaticResolver`].
#[cfg(feature = "mdns")]
pub struct MdnsResolver {
    /// The configured device name (matched against discovered friendly names).
    pub config_device: String,
    /// How long to wait for a `ServiceResolved` before giving up, in seconds.
    pub timeout_secs: u64,
}

#[cfg(feature = "mdns")]
impl MdnsResolver {
    /// A new mDNS resolver browsing for `config_device`, timing out after
    /// `timeout_secs` seconds.
    pub fn new(config_device: impl Into<String>, timeout_secs: u64) -> Self {
        Self {
            config_device: config_device.into(),
            timeout_secs,
        }
    }

    /// Extract a host string from a resolved service. Returns the first
    /// available address of the preferred service (the one whose TXT `fn`
    /// matches `config_device`), else the first resolved service's address.
    fn pick_host(&self, services: &[mdns_sd::ServiceInfo]) -> Option<String> {
        // Prefer the service whose friendly name (TXT `fn`) matches config.
        let preferred = services.iter().find(|svc| {
            if self.config_device.is_empty() {
                return false;
            }
            svc.get_property_val_str("fn") == Some(self.config_device.as_str())
        });
        let chosen = preferred.or_else(|| services.first())?;
        // A/AAAA records carry the host address; pick any available.
        chosen
            .get_addresses()
            .iter()
            .next()
            .map(|ip| ip.to_string())
    }
}

#[cfg(feature = "mdns")]
impl DeviceResolver for MdnsResolver {
    fn resolve(&self) -> Result<DiscoveredDevice, DiscoveryError> {
        use std::time::{Duration, Instant};

        let daemon =
            mdns_sd::ServiceDaemon::new().map_err(|e| DiscoveryError::Socket(e.to_string()))?;
        let receiver = daemon
            .browse("_googlecast._tcp")
            .map_err(|e| DiscoveryError::Socket(e.to_string()))?;

        let deadline = Instant::now() + Duration::from_secs(self.timeout_secs);
        let mut services: Vec<mdns_sd::ServiceInfo> = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let event = match receiver.recv_timeout(remaining) {
                Ok(ev) => ev,
                // Timeout or daemon-closed channel: stop browsing and judge by
                // whatever resolved so far. Both are expected on a quiet LAN.
                Err(_) => break,
            };
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                services.push(info);
                // Keep browsing until the deadline; more matches may arrive
                // and the preferred-name selection needs the full set.
            }
        }
        let _ = daemon.shutdown();

        let host = self.pick_host(&services).ok_or(DiscoveryError::NoDevices)?;
        Ok(DiscoveredDevice {
            host,
            port: 8009,
            source: DiscoverySource::Mdns,
        })
    }
}

// ---------------------------------------------------------------------------
// Total entrypoint — never Err, never panic (R4, G9).
// ---------------------------------------------------------------------------

/// Total device resolution: run `mdns` (when the feature is on) and fall back
/// to [`StaticResolver`] on ANY error. Never returns `Err`, never panics; logs
/// a `warn!` on fallback.
pub fn resolve_device(config_device: &str) -> DiscoveredDevice {
    resolve_with_inner(config_device, default_resolver(config_device))
}

/// Resolve using an explicit resolver (the seam tests exercise). On `Err`, log
/// a warning and fall back to [`StaticResolver`].
fn resolve_with_inner(
    config_device: &str,
    resolver: Option<Box<dyn DeviceResolver>>,
) -> DiscoveredDevice {
    if let Some(r) = resolver {
        match r.resolve() {
            Ok(device) => return device,
            Err(e) => warn!("mDNS discovery failed ({e}); falling back to configured host"),
        }
    }
    // StaticResolver is infallible; if it ever did error, degrade to the raw
    // configured host rather than propagate (G9: never Err, never panic).
    match StaticResolver::new(config_device).resolve() {
        Ok(d) => d,
        Err(_) => DiscoveredDevice {
            host: config_device.to_string(),
            port: 8009,
            source: DiscoverySource::Config,
        },
    }
}

/// Construct the production resolver for `resolve_device`: the mDNS browser
/// when the feature is on, `None` otherwise.
fn default_resolver(config_device: &str) -> Option<Box<dyn DeviceResolver>> {
    #[cfg(feature = "mdns")]
    {
        Some(Box::new(MdnsResolver::new(config_device, 5)))
    }
    #[cfg(not(feature = "mdns"))]
    {
        let _ = config_device;
        None
    }
}

/// Testable seam: resolve with an injected resolver, falling back to
/// [`StaticResolver`] on `Err`. Exposed for the integration tests; production
/// callers use [`resolve_device`].
pub fn resolve_with(resolver: Box<dyn DeviceResolver>, config_device: &str) -> DiscoveredDevice {
    resolve_with_inner(config_device, Some(resolver))
}
