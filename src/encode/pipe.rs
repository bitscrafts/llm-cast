//! H.264 → HLS encode (R4): RGBA canvases in, an HLS stream out.
//!
//! Default features ship [`NullEncoder`] (records submissions, emits
//! nothing) so the whole wiring is testable in-container. The real
//! GStreamer encoder, [`GstEncoder`], is gated behind the optional
//! `gstreamer` feature: `appsrc -> videoconvert -> {x264enc|vaapih264enc} ->
//! hlssink2`, which writes ~1 s H.264 segments plus a live playlist into an
//! output dir. It needs the system GStreamer dev packages and runs only on
//! the operator's LAN host — never in-container.

/// One frame sink: a rasterized RGBA canvas is submitted, an HLS stream
/// comes out the other side. Default features ship [`NullEncoder`]; the
/// real GStreamer encoder is `#[cfg(feature = "gstreamer")]`.
pub trait Encode {
    /// Encode one RGBA canvas (`width*height*4` bytes, row-major).
    fn submit_frame(&mut self, rgba: &[u8], width: usize, height: usize)
        -> Result<(), EncodeError>;
    /// The URL the HLS stream is served at — what the cast LOAD targets.
    fn stream_url(&self) -> String;
}

/// Errors surfaced by an encoder.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    /// The GStreamer pipeline rejected the frame.
    #[error("gstreamer error: {0}")]
    Gst(String),
    /// The submitted buffer does not match the declared canvas.
    #[error("buffer error: {0}")]
    Buffer(String),
    /// The capture side failed while the coordinator stepped.
    #[error("capture error: {0}")]
    Capture(#[from] crate::capture::bridge::BridgeError),
}

/// Default-features encoder: records submissions, emits nothing. In-container
/// tests + dry-run validation prove the wiring minus the codec.
pub struct NullEncoder {
    submitted: usize,
    last_dims: (usize, usize),
    url: String,
}

impl NullEncoder {
    /// An encoder that reports `url` as its stream URL.
    pub fn new(url: String) -> Self {
        NullEncoder {
            submitted: 0,
            last_dims: (0, 0),
            url,
        }
    }

    /// How many canvases have been submitted.
    pub fn submitted(&self) -> usize {
        self.submitted
    }

    /// `(width, height)` of the last submitted canvas.
    pub fn last_dims(&self) -> (usize, usize) {
        self.last_dims
    }
}

impl Encode for NullEncoder {
    fn submit_frame(
        &mut self,
        rgba: &[u8],
        width: usize,
        height: usize,
    ) -> Result<(), EncodeError> {
        // A recording stub: sizes are not validated (the TDD contract feeds
        // a 4-byte buffer for an 8×8 canvas — only the count matters here).
        let _ = (rgba, width, height);
        self.submitted += 1;
        self.last_dims = (width, height);
        Ok(())
    }

    fn stream_url(&self) -> String {
        self.url.clone()
    }
}

/// Real H.264 → HLS encoder: pushes RGBA frames into an `appsrc` feeding
/// `videoconvert ! {x264enc|vaapih264enc} ! hlssink2` (see
/// [`build_pipeline`]). `submit_frame` pushes one buffer with a running
/// timestamp; `stream_url()` reports the URL the HLS output is served at.
#[cfg(feature = "gstreamer")]
pub struct GstEncoder {
    pipeline: gstreamer::Pipeline,
    appsrc: gstreamer_app::AppSrc,
    url: String,
    /// Zero-based frame counter: the presentation timeline of the stream.
    /// HLS wants PTS starting at ~0; the wall clock (do-timestamp) would stamp
    /// the first frame at container uptime (here ~3769 s), which the
    /// Chromecast's native player does not accept — it plays a frame or two
    /// then errors back to the idle screen.
    frame_idx: u64,
    /// Encode framerate — each frame advances PTS by 1e9/`fps` ns.
    fps: u32,
}

#[cfg(feature = "gstreamer")]
impl GstEncoder {
    /// Build the encoder: `encoder` is `"x264"` (software, default) or
    /// `"vaapi"` (VA-API hardware), the canvas is `width`×`height` at `fps`
    /// frames per second, HLS artifacts land in `outdir` (segment files plus
    /// `live.m3u8`), and the playlist lists segment URLs rooted at `root`.
    ///
    /// Every fallible call is mapped into [`EncodeError::Gst`] — no panics.
    pub fn new(
        encoder: &str,
        width: usize,
        height: usize,
        fps: u32,
        outdir: &str,
        root: &str,
        url: String,
        audio: Option<&str>,
    ) -> Result<Self, EncodeError> {
        use gstreamer::prelude::*;

        gstreamer::init().map_err(|e| EncodeError::Gst(e.to_string()))?;
        let (pipeline, appsrc) = build_pipeline(encoder, width, height, fps, outdir, root, audio)?;
        pipeline
            .set_state(gstreamer::State::Playing)
            .map_err(|e| EncodeError::Gst(format!("set_state(Playing): {e:?}")))?;
        Ok(GstEncoder {
            pipeline,
            appsrc,
            url,
            frame_idx: 0,
            fps,
        })
    }
}

#[cfg(feature = "gstreamer")]
impl Encode for GstEncoder {
    fn submit_frame(
        &mut self,
        rgba: &[u8],
        width: usize,
        height: usize,
    ) -> Result<(), EncodeError> {
        let expected = width
            .checked_mul(height)
            .and_then(|area| area.checked_mul(4))
            .ok_or_else(|| EncodeError::Buffer("canvas size overflow".to_string()))?;
        if rgba.len() < expected {
            return Err(EncodeError::Buffer(format!(
                "expected {expected} bytes, got {}",
                rgba.len()
            )));
        }

        // Timeline: a zero-based frame counter, NOT the pipeline clock. With
        // `do-timestamp` the first buffer is stamped at container uptime
        // (~3769 s here), which Chromecast's native player rejects after a
        // frame or two. PTS/DTS advance by 1e9/fps ns per frame so the muxed
        // TS carries a clean t=0 timeline like a normal HLS live stream.
        let pts_ns = self
            .frame_idx
            .checked_mul(1_000_000_000)
            .and_then(|v| v.checked_div(self.fps as u64))
            .ok_or_else(|| EncodeError::Buffer("pts overflow".to_string()))?;
        self.frame_idx += 1;
        let pts = gstreamer::ClockTime::from_nseconds(pts_ns);

        let mut buffer =
            gstreamer::Buffer::with_size(expected).map_err(|e| EncodeError::Gst(e.to_string()))?;
        {
            let buf = buffer
                .get_mut()
                .ok_or_else(|| EncodeError::Gst("buffer not writable".to_string()))?;
            buf.copy_from_slice(0, &rgba[..expected])
                .map_err(|_| EncodeError::Gst("buffer copy failed".to_string()))?;
            buf.set_pts(Some(pts));
            buf.set_dts(Some(pts));
        }

        self.appsrc
            .push_buffer(buffer)
            .map_err(|e| EncodeError::Gst(format!("appsrc push failed: {e}")))?;
        Ok(())
    }

    fn stream_url(&self) -> String {
        self.url.clone()
    }
}

#[cfg(feature = "gstreamer")]
impl Drop for GstEncoder {
    fn drop(&mut self) {
        use gstreamer::prelude::*;
        let _ = self.pipeline.set_state(gstreamer::State::Null);
    }
}

/// Build the real GStreamer pipeline:
///
/// ```text
/// appsrc ... ! videoconvert ! video/x-raw,format=I420 ! {x264enc ... | vaapih264enc} ! h264parse config-interval=-1 ! hlssink2 ...
/// ```
///
/// `video/x-raw,format=I420` forces 4:2:0 into x264 (RGBA would otherwise
/// negotiate a 4:4:4 YUV and produce an undecodable-on-Chromecast "High 4:4:4"
/// stream), and `h264parse config-interval=-1` repeats SPS/PPS before every
/// IDR frame, so a player joining mid-stream (the live playlist's window has
/// rotated past the stream-start headers) still receives parameter sets and
/// can decode.
///
/// `hlssink2` (gst-plugins-bad — there is no `hlsmux` element, the part-4
/// sketch was wrong) writes ~1 s H.264 segments into
/// `outdir/segment/seg_%05d.ts` and a live playlist at `outdir/live.m3u8`
/// (`target-duration=1`, no ENDLIST while running, rolling `max-files=30`
/// window). `root` becomes the playlist's absolute segment URL prefix via
/// `playlist-root`, so the device fetches `ROOT/seg_00000.ts` and our
/// `/segment/:name` route serves it.
/// Build the GStreamer launch string for the HLS pipeline. Pure (no
/// GStreamer init, no parse): factored out so the audio-source seam is
/// testable without instantiating a real pipeline — gstreamer-rs 0.22 has
/// no public API to recover the launch string from a parsed `Pipeline`.
///
/// `audio` is the AudioSource seam (spec-06 part 1): `None` → the silent
/// AAC leg (DMR mandate, byte-identical to the pre-seam string); `Some(frag)`
/// → the operator-supplied launch fragment, followed by `audioconvert !
/// audioresample` so any source negotiates to the muxer's input caps.
pub fn build_launch_string(
    encoder: &str,
    width: usize,
    height: usize,
    fps: u32,
    outdir: &str,
    root: &str,
    audio: Option<&str>,
) -> String {
    let enc = match encoder {
        "vaapi" => "vaapih264enc".to_string(),
        _ => {
            // The I420 caps filter (below) is what matters: without it,
            // videoconvert feeds x264 a 4:4:4 YUV (from the RGBA source) and
            // the stream encodes as "High 4:4:4" — which Chromecast's
            // hardware decoder cannot decode (it wants 4:2:0). Confirmed via
            // gst-launch tag: H.264 (High 4:4:4 Profile). x264 follows the
            // input chroma, so forced I420 -> 4:2:0 (Main/High 4:2:0), which
            // Chromecast decodes. (This x264enc has no `profile` property.)
            "x264enc tune=zerolatency speed-preset=veryfast bitrate=800 \
             key-int-max=30"
                .to_string()
        }
    };
    // Build the audio leg conditionally: None → silent AAC (DMR mandate),
    // Some(frag) → user-provided source.
    let audio_leg = match audio {
        None => "audiotestsrc is-live=true wave=silence ! audioconvert ! audioresample".to_string(),
        Some(f) => format!("{f} ! audioconvert ! audioresample"),
    };
    // A silent AAC track is MANDATORY: the Chromecast Default Media Receiver
    // refuses to play video-only HLS. On-device isolation test (2026-08-16):
    // the same film with audio fetched every segment and played; video-only
    // fetched nothing (VOD) or stalled after two segments (live). `hlssink2`
    // exposes separate request pads — `hls.video` / `hls.audio` — so the two
    // elementary streams are muxed to TS inside the element, no mpegtsmux.
    format!(
        "appsrc name=src format=time is-live=true \
         caps=\"video/x-raw,format=RGBA,width={width},height={height},framerate={fps}/1\" \
         ! videoconvert ! video/x-raw,format=I420 ! {enc} \
         ! h264parse config-interval=-1 ! hls.video \
         {audio_leg} \
         ! voaacenc bitrate=64000 ! aacparse ! hls.audio \
         hlssink2 name=hls location={outdir}/segment/seg_%05d.ts \
                  playlist-location={outdir}/live.m3u8 \
                  target-duration=1 max-files=30 playlist-root={root}"
    )
}

#[cfg(feature = "gstreamer")]
pub fn build_pipeline(
    encoder: &str,
    width: usize,
    height: usize,
    fps: u32,
    outdir: &str,
    root: &str,
    audio: Option<&str>,
) -> Result<(gstreamer::Pipeline, gstreamer_app::AppSrc), EncodeError> {
    use gstreamer::prelude::*;

    // `build_pipeline` is also exercised directly by the R5 test (a malformed
    // fragment must surface as `EncodeError::Gst`, never a "GStreamer has not
    // been initialized" panic). `gst_init_check` is idempotent, so calling it
    // here is safe even when `GstEncoder::new` already initialised the library.
    gstreamer::init().map_err(|e| EncodeError::Gst(e.to_string()))?;
    let launch = build_launch_string(encoder, width, height, fps, outdir, root, audio);
    // gstreamer-rs 0.22 re-exports `parse_launch` as `parse::launch`.
    let element = gstreamer::parse::launch(&launch).map_err(|e| EncodeError::Gst(e.to_string()))?;
    let pipeline = element
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| EncodeError::Gst("parse_launch did not yield a Pipeline".to_string()))?;
    let appsrc = pipeline
        .by_name("src")
        .and_then(|element| element.downcast::<gstreamer_app::AppSrc>().ok())
        .ok_or_else(|| EncodeError::Gst("appsrc not found in pipeline".to_string()))?;
    Ok((pipeline, appsrc))
}
