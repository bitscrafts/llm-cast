//! Real rust_cast device session (R6, part 5): connect to a discovered
//! device over TLS, launch the Default Media Receiver (CC1AD845) and issue
//! the Cast v2 `media/load` carrying the HLS URL.
//!
//! `#[cfg(feature = "cast")]` module: rust_cast is an optional dependency, so
//! this file only compiles when the `cast` feature is enabled.

use std::str::FromStr;

use rust_cast::channels::media::{Media, StreamType};
use rust_cast::channels::receiver::{Application, CastDeviceApp};
use rust_cast::CastDevice;

use super::sender::{CastError, DeviceAddr};

/// Connect to `device` over TLS and launch the Default Media Receiver, wiring
/// the connection + heartbeat channels and the app transport (the DMR's media
/// controller only accepts media commands on a connected transport — without
/// this the LOAD is silently dropped). Every `rust_cast::Error` is mapped to
/// [`CastError::Session`]; the session never panics.
fn connect_and_launch(device: &DeviceAddr) -> Result<(CastDevice, Application), CastError> {
    // 1. TLS connect. Chromecast certs are self-signed, so host verification
    //    is disabled — `connect_without_host_verification` is expected here.
    let cast_device = CastDevice::connect_without_host_verification(&device.host, device.port)
        .map_err(|e| CastError::Session(format!("connect {}:{}: {e}", device.host, device.port)))?;

    // 2. Wire the channels: connection to the receiver, heartbeat ping, then
    //    launch the Default Media Receiver app for its session id.
    cast_device
        .connection
        .connect("receiver-0")
        .map_err(|e| CastError::Session(format!("connection channel: {e}")))?;
    cast_device
        .heartbeat
        .ping()
        .map_err(|e| CastError::Session(format!("heartbeat ping: {e}")))?;

    let app = CastDeviceApp::from_str("CC1AD845")
        .map_err(|_| CastError::Session("invalid app id CC1AD845".to_string()))?;
    let application = cast_device
        .receiver
        .launch_app(&app)
        .map_err(|e| CastError::Session(format!("launch CC1AD845: {e}")))?;

    // 2b. Establish the app's transport connection — verified against
    //     rust_cast's canonical example: launch_app ->
    //     connection.connect(transport_id) -> media.load.
    cast_device
        .connection
        .connect(&application.transport_id)
        .map_err(|e| CastError::Session(format!("connect app transport: {e}")))?;

    Ok((cast_device, application))
}

/// Connect to `device` and load the HLS `url` onto it via the Default Media
/// Receiver. `stream_type` is [`StreamType::Live`] and the content type is
/// `application/x-mpegURL` (the DMR's HLS player).
pub fn send_load(device: &DeviceAddr, url: &str) -> Result<(), CastError> {
    let (cast_device, application) = connect_and_launch(device)?;

    // 3. media/load — mirrors `Sender::build_media_load_request` (HLS, LIVE);
    //    rust_cast builds the wire message from these fields. The destination
    //    is the launched app's transport protocol (e.g. `web-1`).
    let media = Media {
        content_id: url.to_string(),
        stream_type: StreamType::Live,
        content_type: "application/x-mpegURL".to_string(),
        metadata: None,
        duration: None,
    };
    cast_device
        .media
        .load(&application.transport_id, &application.session_id, &media)
        .map_err(|e| CastError::Session(format!("media load: {e}")))?;
    Ok(())
}

/// Load a single static image onto `device` via the Default Media Receiver.
/// The simplest possible cast — one fetch, no streaming, no playlist: the DMR
/// renders the photo full-screen. Used as the ground-truth operator test that
/// the cast leg (connect → launch → load) works end to end before any video.
pub fn send_image_load(device: &DeviceAddr, url: &str) -> Result<(), CastError> {
    let (cast_device, application) = connect_and_launch(device)?;

    // media/load with `StreamType::None` (the device auto-selects) and the
    // image MIME; no duration for a still.
    let media = Media {
        content_id: url.to_string(),
        stream_type: StreamType::None,
        content_type: "image/jpeg".to_string(),
        metadata: None,
        duration: None,
    };
    cast_device
        .media
        .load(&application.transport_id, &application.session_id, &media)
        .map_err(|e| CastError::Session(format!("image load: {e}")))?;
    Ok(())
}
