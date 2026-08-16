//! Cast side (R6): Cast v2 `media/load` sender toward the Default Media
//! Receiver (CC1AD845).

pub mod sender;

#[cfg(feature = "cast")]
pub mod session;

pub use sender::{CastError, DeviceAddr, Sender};
