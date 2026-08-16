//! H.264 encode pipeline (R4): `appsrc -> vaapih264enc -> hlsmux`.
//!
//! The real pipeline needs the optional `gstreamer` feature (system GStreamer
//! dev packages); under default features this module compiles with the codec
//! constant only. The `gstreamer` feature flag must be enabled to call
//! [`build_pipeline`].

/// H.264 encoder used for the cast stream (VA-API hardware encode).
pub const H264_ENCODER: &str = "h264";

/// Build the GStreamer pipeline that turns rasterized frames (pushed into
/// `appsrc`) into an HLS playlist + H.264 segments (`hlsmux`).
///
/// Gated behind the `gstreamer` feature: the crates are optional because
/// they require system GStreamer development packages. The pipeline is
/// assembled at runtime, so missing plugins or a missing VA-API device
/// surface as errors — never panics.
#[cfg(feature = "gstreamer")]
pub fn build_pipeline() -> Result<gstreamer::Pipeline, String> {
    use gstreamer::prelude::*;

    gstreamer::init().map_err(|e| e.to_string())?;
    let bin = gstreamer::parse_launch(
        "appsrc name=src format=time is-live=true \
         ! videoconvert ! vaapih264enc \
         ! hlsmux location=live.m3u8",
    )
    .map_err(|e| e.to_string())?;
    // parse_launch of a pipeline string yields a Bin whose top element is
    // the Pipeline itself; a failed downcast is a genuine error.
    bin.downcast::<gstreamer::Pipeline>()
        .map_err(|_| "parse_launch did not yield a Pipeline".to_string())
}
