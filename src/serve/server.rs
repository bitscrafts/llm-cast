//! HLS HTTP server (R5): serves the live playlist at `/live.m3u8` and media
//! segments at `/segment/<name>` with CORS enabled, so the Chromecast
//! Default Media Receiver (CC1AD845) can fetch the stream cross-origin from
//! `http://<host>:8080/live.m3u8`.
//!
//! The router reads live from a [`MediaStore`] (`crate::serve::store`) — the
//! encoder's output dir in production, a seeded in-memory map in tests and
//! the dry-run. The tower-http CORS layer emits the
//! `Access-Control-Allow-Origin` header on every response.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};

use super::store::MediaStore;

/// The CORS response header the [`CorsLayer`] emits on every response.
pub const CORS_ALLOW_ORIGIN: &str = "Access-Control-Allow-Origin";

/// GET /live.m3u8 — the live HLS playlist from the store, or 404 before the
/// encoder has published anything.
async fn playlist(State(store): State<Arc<dyn MediaStore>>) -> Response {
    match store.playlist() {
        Some(text) => (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            text,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// GET /segment/<name> — one HLS media segment from the store, or 404.
async fn segment(Path(name): Path<String>, State(store): State<Arc<dyn MediaStore>>) -> Response {
    match store.segment(&name) {
        Some(bytes) => ([(header::CONTENT_TYPE, "video/mp2t")], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Build the HLS router over `store` with the CORS layer applied.
///
/// The layer allows any origin, so the Default Media Receiver (whose fetch
/// carries no app-specific origin) still receives the
/// [`CORS_ALLOW_ORIGIN`] response header.
pub fn app(store: Arc<dyn MediaStore>) -> Router {
    Router::new()
        .route("/live.m3u8", get(playlist))
        .route("/segment/:name", get(segment))
        .with_state(store)
        .layer(CorsLayer::new().allow_origin(AllowOrigin::any()))
}

/// Serve the store-backed HLS router on an already-bound `listener` until it
/// fails. The async entry point: `pub fn serve_hls(store, listener)` — the
/// caller (mirror, tests) provides the bound listener so tests can bind
/// `127.0.0.1:0`; it runs `axum::serve(listener, app(store))`.
pub async fn serve_hls(store: Arc<dyn MediaStore>, listener: tokio::net::TcpListener) {
    if let Err(e) = axum::serve(listener, app(store)).await {
        log::warn!("serve_hls: server error: {e}");
    }
}
