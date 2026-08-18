//! Cast seam foundation (spec-03 R3): the argument shape, the stream-type
//! mapping, and the [`CastPort`] closure that a tool calls to put media on the
//! TV. rust_cast types appear only inside the `#[cfg(feature = "cast")]` body.

use std::sync::Arc;

use super::errors::McpServerError;
use crate::cast::{resolve_device, DiscoveredDevice};

/// What a `cast_url` tool invocation asks to play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastUrlArgs {
    /// URL of the media (HLS playlist, MP4, image, ...).
    pub url: String,
    /// Content type of the media, e.g. `application/vnd.apple.mpegurl`.
    pub content_type: String,
}

/// How the Default Media Receiver should treat the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    /// A still image (`image/*`).
    None,
    /// A seekable file (`video/mp4`).
    Buffered,
    /// A live/unbounded stream (HLS and everything else).
    Live,
}

/// Map a content type to the stream kind the DMR expects.
pub fn stream_type_for(content_type: &str) -> StreamKind {
    parse_stream_type(content_type)
}

/// Map a content type to the stream kind: `image/*` → [`StreamKind::None`],
/// `video/mp4` → [`StreamKind::Buffered`], everything else →
/// [`StreamKind::Live`].
pub fn parse_stream_type(content_type: &str) -> StreamKind {
    let ct = content_type.trim().to_ascii_lowercase();
    if ct.starts_with("image/") {
        StreamKind::None
    } else if ct == "video/mp4" {
        StreamKind::Buffered
    } else {
        StreamKind::Live
    }
}

/// The seam between a tool and the cast backend: given the URL + content type,
/// issue the media LOAD and return a human-facing result string. Injected, so
/// tests never touch a device.
pub type CastPort = Arc<dyn Fn(CastUrlArgs) -> Result<String, McpServerError> + Send + Sync>;

/// The production [`CastPort`] built from a resolved device: a real
/// `send_media_load` behind the `cast` feature; a stub error naming the
/// resolved host otherwise. The closure builds `DeviceAddr { host, port }`
/// per call from the resolved device instead of `DeviceAddr::new(device)`.
pub fn production_cast_port_with(device: DiscoveredDevice) -> CastPort {
    #[cfg(feature = "cast")]
    {
        use crate::cast::sender::DeviceAddr;
        use crate::cast::session::{send_media_load, StreamType};

        let host = device.host.clone();
        let port = device.port;
        Arc::new(move |args: CastUrlArgs| {
            let stream_type = match stream_type_for(&args.content_type) {
                StreamKind::None => StreamType::None,
                StreamKind::Buffered => StreamType::Buffered,
                StreamKind::Live => StreamType::Live,
            };
            let addr = DeviceAddr {
                host: host.clone(),
                port,
            };
            send_media_load(&addr, &args.url, &args.content_type, stream_type)
                .map_err(|e| McpServerError::Cast(format!("media load to {host}:{port}: {e}")))?;
            Ok(format!(
                "cast requested to {host}:{port}: {} ({})",
                args.url, args.content_type
            ))
        })
    }
    #[cfg(not(feature = "cast"))]
    {
        let host = device.host.clone();
        let port = device.port;
        Arc::new(move |_args: CastUrlArgs| {
            Err(McpServerError::Cast(format!(
                "cannot cast to {host}:{port}: built without the cast feature"
            )))
        })
    }
}

/// The production [`CastPort`]: resolve the configured device (mDNS when the
/// feature is on, else the static fallback) and build the closure from the
/// resolved device. Thin wrapper over [`production_cast_port_with`] so the
/// `mirror.rs`/`castctl.rs` one-arg call sites keep compiling unchanged (N5).
pub fn production_cast_port(device: String) -> CastPort {
    production_cast_port_with(resolve_device(&device))
}
