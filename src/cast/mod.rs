//! Cast side (R6): Cast v2 `media/load` sender toward the Default Media
//! Receiver (CC1AD845).

pub mod discovery;
pub mod sender;

#[cfg(feature = "cast")]
pub mod session;

#[cfg(feature = "mdns")]
pub use discovery::MdnsResolver;
pub use discovery::{
    resolve_device, resolve_with, DeviceResolver, DiscoveredDevice, DiscoveryError,
    DiscoverySource, StaticResolver,
};
pub use sender::{CastError, DeviceAddr, Sender};
