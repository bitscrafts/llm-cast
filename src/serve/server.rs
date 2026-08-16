//! HLS HTTP server (R5): serves the live playlist at `/live.m3u8` and media
//! segments at `/segment/<name>` with CORS enabled, so the Chromecast
//! Default Media Receiver (CC1AD845) can fetch the stream cross-origin from
//! `http://<host>:8080/live.m3u8`.
//!
//! The router is in-memory: a fixed playlist and one static segment blob
//! stand in for encoder output (the encoder, `crate::encode`, publishes
//! fresh bytes here in a later part). The tower-http CORS layer emits the
//! `Access-Control-Allow-Origin` header on every response.

use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// The live HLS playlist served at `/live.m3u8`.
pub const PLAYLIST: &str = "#EXTM3U\n\
                            #EXT-X-VERSION:3\n\
                            #EXT-X-TARGETDURATION:2\n\
                            #EXT-X-MEDIA-SEQUENCE:0\n\
                            #EXTINF:2.0,\n\
                            segment/seg0.ts\n\
                            #EXT-X-ENDLIST\n";

/// The single in-memory media segment (a stand-in for real encoder output).
pub const SEGMENT_BYTES: &[u8] = b"GSTREAMER-H264-SEGMENT-0000000000000001";

/// The CORS response header the [`CorsLayer`] emits on every response.
pub const CORS_ALLOW_ORIGIN: &str = "Access-Control-Allow-Origin";

/// Static segment registry: name -> bytes. The encoder will replace this
/// with live output in a later part.
fn segment_bytes(name: &str) -> Option<&'static [u8]> {
    match name {
        "seg0.ts" => Some(SEGMENT_BYTES),
        _ => None,
    }
}

/// GET /live.m3u8 — the HLS playlist, with CORS applied by the layer.
async fn playlist() -> Response {
    (
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        PLAYLIST,
    )
        .into_response()
}

/// GET /segment/<name> — one HLS media segment, or 404 for unknown names.
async fn segment(Path(name): Path<String>) -> Response {
    match segment_bytes(&name) {
        Some(bytes) => ([(header::CONTENT_TYPE, "video/mp2t")], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Build the HLS router with the CORS layer applied.
///
/// The layer allows any origin, so the Default Media Receiver (whose fetch
/// carries no app-specific origin) still receives the
/// [`CORS_ALLOW_ORIGIN`] response header.
pub fn app() -> Router {
    Router::new()
        .route("/live.m3u8", get(playlist))
        .route("/segment/:name", get(segment))
        .layer(CorsLayer::new().allow_origin(AllowOrigin::any()))
}
