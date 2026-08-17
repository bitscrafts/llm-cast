//! Display sizing model (adaptive resolution): given the target frame
//! (`TV_RESOLUTION`), the logical terminal size (`TV_TERMINAL` cols×rows) and a
//! symmetric edge margin, compute the xterm font size and the centered geometry
//! so the terminal always fits inside the frame with safe margins. The cell
//! calibration is measured against DejaVu Sans Mono (the display xterm's font):
//! cell_w ≈ 0.83×pts, cell_h ≈ 1.71×pts (verified live at -fs 11 and -fs 13).
//!
//! Pure and unwrap-free: every fallible input is rejected by the parsers and
//! every computation degrades to a clamped result, never a panic (the meta-test
//! scans `src/mcp` for `.unwrap()`/`.expect()`).

/// Measured cell width per font point (px/pt) for DejaVu Sans Mono.
pub const CELL_W_PER_PT: f64 = 0.83;
/// Measured cell height per font point (px/pt) for DejaVu Sans Mono.
pub const CELL_H_PER_PT: f64 = 1.71;
/// Smallest usable font size, matching the `set_font_size` contract (6..=32).
pub const MIN_FONT_PTS: f64 = 6.0;

/// A display frame size, e.g. `TV_RESOLUTION=1280x800`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    /// Parse `"1920x1080"`; reject anything but `\d+x\d+` with non-zero sizes.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (width, height) = parse_dims(spec, "resolution")?;
        if width == 0 || height == 0 {
            return Err(format!("resolution must be non-zero: {spec}"));
        }
        Ok(Self { width, height })
    }
}

/// A logical terminal size, e.g. `TV_TERMINAL=140x40`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u32,
    pub rows: u32,
}

impl TerminalSize {
    /// Parse `"116x34"`; reject anything but `\d+x\d+` with non-zero sizes.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (cols, rows) = parse_dims(spec, "terminal size")?;
        if cols == 0 || rows == 0 {
            return Err(format!("terminal size must be non-zero: {spec}"));
        }
        Ok(Self { cols, rows })
    }
}

/// Split `\d+x\d+`; shared by both parsers. Rejects extra `x` parts and
/// non-numeric tokens without panicking.
fn parse_dims(spec: &str, what: &str) -> Result<(u32, u32), String> {
    let mut parts = spec.split('x');
    let a = parts.next().unwrap_or_default();
    let b = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return Err(format!("{what} must be `WxH`, got `{spec}`"));
    }
    let a = a
        .parse::<u32>()
        .map_err(|_| format!("{what} must be `WxH`, got `{spec}`"))?;
    let b = b
        .parse::<u32>()
        .map_err(|_| format!("{what} must be `WxH`, got `{spec}`"))?;
    Ok((a, b))
}

/// The computed display xterm spec: the fractional font size for `-fs` plus the
/// centered geometry string for `-geometry` (`CxR+ox+oy`). The font the argv
/// carries and the geometry it was derived from are always the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XtermSpec {
    pub font_pts: String,
    pub geometry: String,
}

/// The full auto-fit: choose the largest font (floored to 0.1pt, clamped to
/// at least [`MIN_FONT_PTS`]) that keeps `cols×rows` inside the margin inset,
/// then center the resulting window. A round-up that overflowed the height
/// budget is impossible because the font is floored, never rounded.
pub fn fit(frame: &Resolution, term: &TerminalSize, margin: f64) -> Result<XtermSpec, String> {
    validate_margin(margin)?;
    let avail_w = frame.width as f64 * (1.0 - 2.0 * margin);
    let avail_h = frame.height as f64 * (1.0 - 2.0 * margin);
    let font_w = avail_w / (term.cols as f64 * CELL_W_PER_PT);
    let font_h = avail_h / (term.rows as f64 * CELL_H_PER_PT);
    let font = quantize_font(font_w.min(font_h));
    Ok(XtermSpec {
        font_pts: format_font(font),
        geometry: build_geometry(frame, term, font),
    })
}

/// The centered geometry for an already-fixed font size (used by
/// `set_font_size`): same quantization + centering as [`fit`], with the font
/// supplied by the caller instead of chosen.
pub fn geometry_at(
    frame: &Resolution,
    term: &TerminalSize,
    margin: f64,
    font_pts: f64,
) -> Result<String, String> {
    validate_margin(margin)?;
    let font = quantize_font(font_pts);
    Ok(build_geometry(frame, term, font))
}

fn validate_margin(margin: f64) -> Result<(), String> {
    if !(margin > 0.0 && margin < 0.5) {
        return Err(format!("margin must be in (0, 0.5), got {margin}"));
    }
    Ok(())
}

/// Floor to the nearest 0.1pt, clamped to at least [`MIN_FONT_PTS`].
fn quantize_font(font: f64) -> f64 {
    let tenth = (font * 10.0).floor() / 10.0;
    tenth.max(MIN_FONT_PTS)
}

/// `CxR+ox+oy` centering the window (at the given font) inside the frame;
/// offsets are floored and clamped to ≥ 0, so a window larger than the frame
/// pins to the origin rather than going negative.
fn build_geometry(frame: &Resolution, term: &TerminalSize, font: f64) -> String {
    let win_w = (term.cols as f64 * font * CELL_W_PER_PT).floor();
    let win_h = (term.rows as f64 * font * CELL_H_PER_PT).floor();
    let off_x = ((frame.width as f64 - win_w) / 2.0).floor().max(0.0) as u32;
    let off_y = ((frame.height as f64 - win_h) / 2.0).floor().max(0.0) as u32;
    format!("{}x{}+{}+{}", term.cols, term.rows, off_x, off_y)
}

/// The font the way xterm's `-fs` expects it: one decimal place (the `faceSize`
/// resource is a float, so `-fs 10.4` renders).
fn format_font(font: f64) -> String {
    format!("{font:.1}")
}
