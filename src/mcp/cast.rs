//! Cast seam foundation (spec-03 R3): the argument shape, the stream-type
//! mapping, and the [`CastPort`] closure that a tool calls to put media on the
//! TV. rust_cast types appear only inside the `#[cfg(feature = "cast")]` body.

use std::sync::Arc;

use super::errors::McpServerError;

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

/// The production [`CastPort`]: a real `send_media_load` behind the `cast`
/// feature; a stub error naming the missing feature otherwise.
pub fn production_cast_port(device: String) -> CastPort {
    #[cfg(feature = "cast")]
    {
        use crate::cast::sender::DeviceAddr;
        use crate::cast::session::{send_media_load, StreamType};

        Arc::new(move |args: CastUrlArgs| {
            let stream_type = match stream_type_for(&args.content_type) {
                StreamKind::None => StreamType::None,
                StreamKind::Buffered => StreamType::Buffered,
                StreamKind::Live => StreamType::Live,
            };
            let addr = DeviceAddr::new(device.clone());
            send_media_load(&addr, &args.url, &args.content_type, stream_type)
                .map_err(|e| McpServerError::Cast(format!("media load to {device}: {e}")))?;
            Ok(format!(
                "cast requested to {device}: {} ({})",
                args.url, args.content_type
            ))
        })
    }
    #[cfg(not(feature = "cast"))]
    {
        let _ = device;
        Arc::new(|_args: CastUrlArgs| {
            Err(McpServerError::Cast(
                "cannot cast: built without the cast feature".to_string(),
            ))
        })
    }
}
