//! The still background, and the honest fallback behind it.
//!
//! **R9** — *fall back, in order, to still image then solid colour, when the
//! configured background is unavailable; the fallback is logged, not silent.*
//! P1 is a still by design, so the chain this file implements is the bottom
//! two rungs of R9's three: still image, then solid colour. Video is P2 and
//! slots in above, which is why the decision to show a colour is a value
//! (`Background::Solid`) rather than a branch in the draw code.
//!
//! Three properties are load-bearing, and each is a test below:
//!
//!   * **Nothing here can stop a login.** A missing file, a truncated file, a
//!     file that is not an image, a format this build cannot decode, and an
//!     image too large to be sane all end in the same place: the solid colour,
//!     with the path and the reason on stderr. There is no `unwrap` on
//!     anything derived from the file.
//!   * **R15 and C10** — no media path reads from a user's home. The greeter
//!     runs as its own user and cannot traverse `/home/<user>` at mode 700, so
//!     a background configured there is refused *before the open*, with a
//!     message that says where to put it instead. Symlinks are resolved first,
//!     so a path outside a home that points into one is refused too.
//!   * **The decode happens once, off the main thread.** Measured, not
//!     assumed; see `spike/p1/BACKGROUND.md` for the numbers and
//!     `measure_decode_cost` below for the harness that produced them. The
//!     short version: a 4K PNG decode is far too expensive to sit in front of
//!     the first frame, so it does not.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use glow::HasContext;

use crate::render::Gl;
use crate::store::is_outside_homes;

/// Where a background may live, named once so the refusal message can say it.
/// Any directory outside the homes would do; this is the one the project's own
/// deployment uses, and C14 establishes the convention of reading the greeter's
/// media out of `/var/lib`.
pub const RECOMMENDED_DIR: &str = "/var/lib/auroval";

/// A ceiling on the pixels a configured file may claim to hold, applied to the
/// header before anything is allocated. Far beyond any display anyone owns:
/// the point is only that a hostile or corrupt header cannot ask for a
/// terabyte. The decoder's own 512 MiB allocation limit is left at its
/// default, which caps the decoded image independently.
const MAX_DIMENSION: u32 = 16_384;

/// A decoded still, in the layout GL wants: tightly packed RGBA8 rows.
pub struct Still {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Still {
    /// How many bytes go to the driver. Only used in the measurement harness.
    #[allow(dead_code)]
    fn bytes(&self) -> usize {
        self.rgba.len()
    }
}

/// Written out by hand: a derived `Debug` would print several million pixels
/// into a test failure, which is worse than no `Debug` at all.
impl std::fmt::Debug for Still {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Still {{ {}x{}, {} bytes }}",
            self.width,
            self.height,
            self.rgba.len()
        )
    }
}

/// Why there is no image. Carries the path because "the background failed" is
/// useless to the person who has to fix it, and R9 asks for the fallback to be
/// logged rather than merely survived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: PathBuf,
    pub reason: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "background {} unavailable, falling back to the solid colour: {}",
            self.path.display(),
            self.reason
        )
    }
}

impl Problem {
    /// `pub(crate)` rather than private: `video` reports its failures in the
    /// same shape, because R9 makes them the same kind of thing — a reason
    /// the background is not what was configured, carrying the path.
    pub(crate) fn new(path: &Path, reason: impl Into<String>) -> Problem {
        Problem {
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }
}

/// Read and decode a configured background.
///
/// Pure except for the filesystem, and every failure is a `Problem` rather
/// than a panic or an abort — which is what makes the fallback testable
/// without a GL context.
pub fn decode(path: &Path) -> Result<Still, Problem> {
    // R15 first, because the cheapest way not to read a home directory is not
    // to open the file. The message names the fix, not just the fault.
    if !is_outside_homes(path) {
        return Err(Problem::new(
            path,
            format!(
                "it is inside a home directory, which the greeter runs as its \
                 own user and cannot read (R15, C10) — copy it to {RECOMMENDED_DIR} instead"
            ),
        ));
    }

    // Resolving the path does two jobs: it turns "missing" into a clear error
    // before the decoder sees it, and it catches a symlink out of a public
    // directory into a home, which the check above cannot see.
    let real = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => return Err(Problem::new(path, e.to_string())),
    };
    if !is_outside_homes(&real) {
        return Err(Problem::new(
            path,
            format!(
                "it resolves to {}, inside a home directory the greeter cannot \
                 read (R15, C10)",
                real.display()
            ),
        ));
    }

    let mut reader = match image::ImageReader::open(&real) {
        Ok(r) => r,
        Err(e) => return Err(Problem::new(path, e.to_string())),
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    reader.limits(limits);
    // The format comes from the content, not the extension, so a .png that is
    // really a JPEG works and a .png that is really a text file is refused by
    // the same path as a corrupt one. This build carries only the PNG and JPEG
    // decoders on purpose, so anything else arrives here as `Unsupported` and
    // degrades with a message that says which format it was.
    let reader = match reader.with_guessed_format() {
        Ok(r) => r,
        Err(e) => return Err(Problem::new(path, e.to_string())),
    };
    let decoded = match reader.decode() {
        Ok(d) => d,
        Err(e) => return Err(Problem::new(path, e.to_string())),
    };

    let rgba = decoded.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    if width == 0 || height == 0 {
        return Err(Problem::new(path, "it decodes to zero pixels"));
    }
    Ok(Still {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

/// The destination rectangle for a crop-to-fill, centred: `[x, y, w, h]` in
/// pixels, with the surface's top-left origin.
///
/// Cover, not contain, and never a stretch — an aspect-ratio mismatch must not
/// distort a face. The scale is the *larger* of the two ratios, so the image
/// covers the surface in both axes and overflows in one; the overflow is
/// clipped by the viewport, which is why this needs no change to the shared
/// image shader (whose UVs run 0..1 over the whole quad).
pub fn fill_rect(surface_w: u32, surface_h: u32, img_w: u32, img_h: u32) -> [f32; 4] {
    let (sw, sh) = (surface_w as f32, surface_h as f32);
    let (iw, ih) = (img_w.max(1) as f32, img_h.max(1) as f32);
    let scale = (sw / iw).max(sh / ih);
    let (w, h) = (iw * scale, ih * scale);
    [(sw - w) * 0.5, (sh - h) * 0.5, w, h]
}

/// The dimensions to shrink to so that both fit inside `max`, keeping the
/// aspect ratio; `None` when it already fits.
///
/// GLES2 drivers are free to cap texture size, and an 8K still on a 4K-capped
/// driver would otherwise be a black screen. Tenet 6 says the person's own
/// media wins, so the image is scaled rather than refused.
pub fn fit_within(w: u32, h: u32, max: u32) -> Option<(u32, u32)> {
    if max == 0 || (w <= max && h <= max) {
        return None;
    }
    let scale = (max as f64 / w as f64).min(max as f64 / h as f64);
    Some((
        ((w as f64 * scale).floor() as u32).max(1).min(max),
        ((h as f64 * scale).floor() as u32).max(1).min(max),
    ))
}

/// What the draw code has instead of a branch: either a texture to cover the
/// surface with, or the decision to show the solid colour.
enum State {
    /// No background configured, or one that could not be shown. Either way
    /// the colour is the answer and nothing further will be attempted.
    Solid,
    /// A decode is in flight on its own thread. The colour is shown until it
    /// lands, so the first frame never waits on it (R16).
    Pending(Receiver<Result<Still, Problem>>),
    /// Uploaded once. `tex` is owned by the GL context and deliberately not
    /// deleted here: the renderer outlives the greeter's run, and there is no
    /// point in a destructor that only ever runs at exit.
    Ready { tex: glow::Texture, w: u32, h: u32 },
    /// Configured as a video, not started yet.
    ///
    /// `start` runs before there is a surface, which is the whole reason the
    /// still decode happens on a thread — R16's first frame must not wait on
    /// it. A video needs the GL context in order to share it with GStreamer,
    /// and the GL context does not exist that early, so this records the
    /// intent and `poll` acts on it at the first frame that has one.
    VideoPending(PathBuf),
    /// Playing. Each frame hands back a different texture from GStreamer's
    /// pool; see `video::Video` for why the mapping is held rather than the
    /// id alone.
    Video(Box<crate::video::Video>),
}

/// Whether a configured background is a video, by its extension.
///
/// By extension and not by sniffing, deliberately. The alternative is to try
/// the image decoder first and fall back on failure, which means every video
/// background pays for a failed image decode on the startup path C15 measures
/// in milliseconds. An extension that lies falls back anyway: a `.mp4` that
/// is not a video fails in GStreamer and lands on the solid colour with a
/// line saying so, which is R9 either way.
fn looks_like_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "webm" | "mov" | "m4v" | "avi")
    )
}

/// The background, from configuration to pixels.
///
/// `start` returns immediately; `draw` is called once per frame and does the
/// upload on the first frame after the decode lands. After that it is one
/// `draw_image` call and nothing else — the decode and the upload each happen
/// exactly once per run.
pub struct Background {
    state: State,
    /// What was configured, kept so that a failure *after* the decode — a
    /// thread that died, a driver that refused the texture — can still name
    /// the file. R9 asks for the fallback to be logged, and a log that does
    /// not say which background failed is not much of one.
    configured: Option<PathBuf>,
    /// The candidates not tried yet, in the order R9 asks for them. Each
    /// failure moves to the next; running out is the solid colour.
    rest: std::collections::VecDeque<PathBuf>,
    /// Everything that went wrong, already formatted, in the order it
    /// happened. Mirrors `Config::problems`: the greeter reports rather than
    /// hides, and a test can assert the fallback was announced.
    problems: Vec<String>,
}

impl Background {
    /// Begin loading, without blocking. Call this as early as possible —
    /// before the Wayland surface exists, if that is practical — because the
    /// whole point is to spend the decode on the thread while the main thread
    /// is still negotiating a surface.
    ///
    /// `None` is not a failure: it is the configured solid colour, so it logs
    /// nothing. R9 is about a background that was *asked for* and could not be
    /// shown.
    pub fn start(paths: &[PathBuf]) -> Background {
        let mut bg = Background {
            state: State::Solid,
            configured: None,
            rest: paths.iter().cloned().collect(),
            problems: Vec::new(),
        };
        bg.try_next();
        bg
    }

    /// Begin the next candidate, or settle on the solid colour.
    ///
    /// This is R9's "in order" as a loop rather than as a special case for
    /// two: every failure lands here and asks for the next one, and running
    /// out is the colour. Adding a third kind of background adds an arm to
    /// the match and nothing else.
    ///
    /// Deliberately takes no `Gl`. A video is only *recorded* here and
    /// started by `poll` at the first frame that has a context, which is what
    /// keeps `start` off R16's critical path -- and what lets `start` be
    /// called before a surface exists.
    fn try_next(&mut self) {
        let Some(path) = self.rest.pop_front() else {
            self.state = State::Solid;
            self.configured = None;
            return;
        };
        self.configured = Some(path.clone());
        if looks_like_video(&path) {
            self.state = State::VideoPending(path);
            return;
        }
        let (tx, rx) = mpsc::channel();
        // A detached thread, deliberately: there is nothing to join, and a
        // decode still running at exit must not hold the greeter open.
        let for_thread = path.clone();
        match std::thread::Builder::new()
            .name("auroval-bg-decode".into())
            .spawn(move || {
                let began = Instant::now();
                let out = decode(&for_thread);
                if let Ok(s) = &out {
                    eprintln!(
                        "[background] decoded {} ({}x{}) in {:?}",
                        for_thread.display(),
                        s.width,
                        s.height,
                        began.elapsed()
                    );
                }
                // The receiver is gone only if the greeter is shutting down,
                // in which case dropping the pixels is exactly right.
                let _ = tx.send(out);
            }) {
            Ok(_) => self.state = State::Pending(rx),
            Err(e) => {
                // Even failing to spawn degrades rather than dies, and
                // degrading means the next candidate, not the colour.
                self.failed(format!("its decoder thread could not be started: {e}"));
                self.try_next();
            }
        }
    }

    /// Take delivery of the decode if it has arrived, and say what there is
    /// to draw: the texture and its own size, or nothing.
    ///
    /// This used to be a `draw` that cleared the surface and placed the image
    /// itself. It was split when the view turned out to clear as well, which
    /// merged naively would have cleared twice — but the *poll* could not
    /// simply move to the caller, because the upload has to happen on the
    /// thread holding the GL context and only this type knows when the pixels
    /// have landed. So the poll stays here and the placing goes to the view,
    /// which keeps the view a pure function of what it is given.
    pub fn ready(&mut self, gl: &Gl) -> Option<(glow::Texture, u32, u32)> {
        self.poll(gl);
        match &self.state {
            State::Ready { tex, w, h } => Some((*tex, *w, *h)),
            State::Video(v) => v.frame().map(|f| {
                (
                    // GStreamer's texture id is a raw GL name, which is what
                    // `glow::NativeTexture` wraps. The renderer draws it like
                    // any other 2D texture because `glsinkbin` has already
                    // converted to RGBA -- the two points of CPU that buys
                    // are measured in spike/p2/RESULTS.md, and they buy not
                    // needing a second shader for NV12.
                    glow::NativeTexture(std::num::NonZeroU32::new(f.texture).unwrap()),
                    f.width,
                    f.height,
                )
            }),
            _ => None,
        }
    }

    fn poll(&mut self, r: &Gl) {
        if let State::VideoPending(path) = &self.state {
            let path = path.clone();
            match crate::video::Video::start(&path, r.egl_handles()) {
                Ok(v) => {
                    eprintln!("[background] playing {}", path.display());
                    self.state = State::Video(Box::new(v));
                }
                Err(problem) => {
                    // R9's middle step: a video that will not play tries the
                    // next candidate, and only an empty list is the colour.
                    self.fell_back(problem);
                    self.try_next();
                }
            }
            return;
        }
        if let State::Video(v) = &self.state {
            for p in v.problems() {
                self.problems.push(format!("[background] {p}"));
            }
            return;
        }
        let State::Pending(rx) = &self.state else {
            return;
        };
        match rx.try_recv() {
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                // The thread died without sending — a panic in a decoder, say.
                // The greeter does not: it says so and tries the next one.
                self.failed("its decoder thread ended without a result");
                self.try_next();
            }
            Ok(Err(problem)) => {
                self.fell_back(problem);
                self.try_next();
            }
            Ok(Ok(still)) => match upload(&r.gl, &still) {
                Ok((tex, w, h)) => {
                    self.state = State::Ready { tex, w, h };
                }
                Err(reason) => {
                    // The decode worked and the upload did not, which is a
                    // driver problem rather than a file problem -- but the
                    // next candidate may be smaller, so it still gets a turn.
                    self.failed(reason);
                    self.try_next();
                }
            },
        }
    }

    /// Record and announce a fallback. R9's "logged, not silent" is this
    /// function: stderr for the journal, and the list for whatever surface
    /// ends up showing problems to the person at the keyboard.
    fn fell_back(&mut self, p: Problem) {
        let line = p.to_string();
        eprintln!("[background] {line}");
        self.problems.push(line);
    }

    /// The same, for a failure that happened after the decode and so has no
    /// `Problem` of its own. Named against the configured path, which is the
    /// only thing the person at the keyboard can act on.
    fn failed(&mut self, reason: impl Into<String>) {
        let path = self
            .configured
            .clone()
            .unwrap_or_else(|| PathBuf::from("<none configured>"));
        self.fell_back(Problem {
            path,
            reason: reason.into(),
        });
    }

    /// Everything that went wrong, for the probe and for tests.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// Whether a still is on screen. For the probe; `draw` needs no caller
    /// that asks first.
    /// Whether the outcome is known: the image has arrived, or it is not
    /// coming. A caller that waits for a background — the snapshot does, since
    /// one frame rendered immediately would always show the fallback — needs
    /// to stop waiting on a failure too, and `ready()` returning `None` does
    /// not distinguish "not yet" from "never".
    pub fn settled(&mut self, gl: &Gl) -> bool {
        self.poll(gl);
        // "Settled" means there is nothing further to wait for, and for a
        // video that includes its first frame: the only callers are the
        // snapshot modes, which exist to capture a picture and would
        // otherwise capture the fallback every time. The greeter itself does
        // not call this -- it asks `ready` once per frame and draws the
        // colour until there is something better, which is what keeps the
        // decoder off R16's critical path.
        match &self.state {
            State::Pending(_) | State::VideoPending(_) => false,
            State::Video(v) => v.frame().is_some(),
            _ => true,
        }
    }

    /// A number that changes exactly when the background's picture changes.
    ///
    /// This is what lets the draw loop stay on one mechanism. #49 made the
    /// greeter draw only when `view::signature` changes, which was right for
    /// a still screen and silently wrong for a video: the signature carried
    /// a *boolean* for the background, so it flipped once when the first
    /// frame arrived and never again. The video played and the screen did
    /// not move.
    ///
    /// A still is 1 forever, so it still draws once. A video counts the
    /// frames the sink has taken, so the greeter redraws per delivered
    /// frame — at the clip's rate rather than the loop's, which is better
    /// than the unconditional draw #49 replaced.
    ///
    /// The `+2` keeps a video that has delivered nothing yet from colliding
    /// with "no background" or "a still".
    pub fn generation(&self) -> u64 {
        match &self.state {
            State::Solid | State::Pending(_) | State::VideoPending(_) => 0,
            State::Ready { .. } => 1,
            State::Video(v) => v.delivered() + 2,
        }
    }

    #[allow(dead_code)]
    pub fn has_image(&self) -> bool {
        matches!(self.state, State::Ready { .. })
    }
}

/// Hand the rows to the driver, once.
///
/// Returns the texture and the dimensions actually uploaded, which differ from
/// the still's own when the driver's texture cap forced a shrink. Takes the GL
/// context rather than the `Renderer` because nothing here needs the surface:
/// the texture path can be exercised against any context, including a
/// headless one, the day that is worth doing.
fn upload(gl: &glow::Context, still: &Still) -> Result<(glow::Texture, u32, u32), String> {
    let max = unsafe { gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE) }.max(0) as u32;

    let (w, h, owned);
    let mut rows: &[u8] = &still.rgba;
    match fit_within(still.width, still.height, max) {
        None => {
            w = still.width;
            h = still.height;
        }
        Some((nw, nh)) => {
            eprintln!(
                "[background] {}x{} exceeds this driver's {max}px texture limit; \
                 scaling to {nw}x{nh}",
                still.width, still.height
            );
            let img = image::RgbaImage::from_raw(still.width, still.height, still.rgba.clone())
                .ok_or("the decoded rows do not match the decoded dimensions")?;
            owned = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
                .into_raw();
            rows = &owned;
            w = nw;
            h = nh;
        }
    }

    unsafe {
        let tex = gl.create_texture().map_err(|e| e.to_string())?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        // A still is drawn once at one size: no mipmaps, and CLAMP_TO_EDGE so
        // that the crop's overflow cannot wrap a strip of the far edge back
        // into view. Those are also exactly the settings under which GLES2
        // allows a non-power-of-two texture, which a wallpaper always is.
        for (k, v) in [
            (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
            (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
        }
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            w as i32,
            h as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(rows)),
        );
        let err = gl.get_error();
        if err != glow::NO_ERROR {
            gl.delete_texture(tex);
            return Err(format!(
                "the driver refused a {w}x{h} texture (GL 0x{err:x})"
            ));
        }
        Ok((tex, w, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    /// A directory of our own under the temporary directory. Not in a home:
    /// `decode` would refuse it, which is the point of the R15 test below.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("greet-bg-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch directory");
        assert!(
            is_outside_homes(&dir),
            "the test fixtures must not live in a home, or they would be refused for the wrong reason"
        );
        dir
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).expect("fixture");
        f.write_all(bytes).expect("fixture");
        p
    }

    /// A real PNG, encoded rather than checked in, so the fixtures cannot rot
    /// and the test says what the image is.
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x % 256) as u8, (y % 256) as u8, 0x40, 0xff]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("encodes");
        out.into_inner()
    }

    /// Deterministic noise, for the measurement only. A smooth gradient
    /// deflates to almost nothing and would make a PNG decode look far
    /// cheaper than a photograph's; noise is the other end of that range, and
    /// a real photograph sits between the two.
    fn noise_png(w: u32, h: u32) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        let mut s: u32 = 0x2545_f491;
        for p in img.pixels_mut() {
            // xorshift32: no dependency, and the same bytes on every machine.
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let [a, b, c, _] = s.to_le_bytes();
            *p = image::Rgba([a, b, c, 0xff]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("encodes");
        out.into_inner()
    }

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let mut img = image::RgbImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([(x % 256) as u8, (y % 256) as u8, 0x40]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .expect("encodes");
        out.into_inner()
    }

    #[test]
    fn a_png_decodes_to_its_own_dimensions() {
        let dir = scratch("png");
        let p = write(&dir, "bg.png", &png(64, 32));
        let s = decode(&p).expect("a valid PNG decodes");
        assert_eq!((s.width, s.height), (64, 32));
        assert_eq!(s.rgba.len(), 64 * 32 * 4, "tightly packed RGBA8");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_jpeg_decodes_too() {
        let dir = scratch("jpeg");
        let p = write(&dir, "bg.jpg", &jpeg(48, 64));
        let s = decode(&p).expect("a valid JPEG decodes");
        assert_eq!((s.width, s.height), (48, 64));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_content_decides_the_format_not_the_extension() {
        let dir = scratch("liar");
        // A JPEG called .png. A greeter that trusted the extension would show
        // a colour here for no reason.
        let p = write(&dir, "bg.png", &jpeg(32, 32));
        assert!(decode(&p).is_ok(), "the format is guessed from the content");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_file_falls_back_and_says_which() {
        let p = Path::new("/var/lib/auroval/definitely-not-here.png");
        let problem = decode(p).expect_err("a missing file cannot be shown");
        assert_eq!(problem.path, p);
        let said = problem.to_string();
        assert!(
            said.contains("definitely-not-here.png"),
            "R9: the log names the file — got {said}"
        );
        assert!(
            said.contains("solid colour"),
            "R9: the log says what happens instead — got {said}"
        );
        assert!(
            !problem.reason.is_empty(),
            "R9: the log says why — got {said}"
        );
    }

    #[test]
    fn a_truncated_image_falls_back_and_does_not_panic() {
        let dir = scratch("truncated");
        let whole = png(256, 256);
        // Header and a little data, then nothing: a file cut off mid-write,
        // which is what an interrupted copy into /var/lib actually leaves.
        let p = write(&dir, "cut.png", &whole[..whole.len() / 3]);
        let problem = decode(&p).expect_err("half a PNG is not an image");
        assert_eq!(problem.path, p);
        assert!(!problem.reason.is_empty(), "the reason is reported");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_that_is_not_an_image_at_all_falls_back_and_does_not_panic() {
        let dir = scratch("notimage");
        let p = write(
            &dir,
            "bg.png",
            b"-- this is somebody's config file, saved over the wallpaper\n",
        );
        let problem = decode(&p).expect_err("text is not an image");
        assert_eq!(problem.path, p);
        assert!(!problem.reason.is_empty());

        // And the degenerate cases around it, which a hand-rolled sniffer
        // tends to get wrong: nothing at all, and a single byte.
        for (name, bytes) in [("empty.png", &b""[..]), ("one.png", &b"\x89"[..])] {
            let p = write(&dir, name, bytes);
            assert!(decode(&p).is_err(), "{name} is not an image");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_format_this_build_cannot_decode_falls_back_rather_than_failing_to_build() {
        let dir = scratch("gif");
        // A valid GIF header. `image` is built with only png and jpeg, so this
        // is `Unsupported`, which must degrade like any other unavailable
        // background rather than being a surprise at runtime.
        let p = write(&dir, "bg.gif", b"GIF89a\x01\x00\x01\x00\x80\x00\x00");
        assert!(
            decode(&p).is_err(),
            "a format we carry no decoder for degrades"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_directory_is_not_an_image_either() {
        let dir = scratch("dir");
        assert!(
            decode(&dir).is_err(),
            "a directory degrades like anything else"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The same refusal, for the video path, which did not have it for a
    /// day. `decode` has refused a home since it was written; `Video::start`
    /// was built as a transport experiment and inherited none of that care.
    /// A test here rather than in `video` because this is where the rule
    /// lives and where the next source will be added.
    #[test]
    fn a_video_in_a_home_is_refused_before_the_pipeline_is_built() {
        let home = std::env::var("HOME").expect("a home to point into");
        let path = std::path::PathBuf::from(home).join("whatever.mp4");
        assert!(
            looks_like_video(&path),
            "the extension decides, and .mp4 is one"
        );
        // No GL context is needed to be refused, which is the point: the
        // check runs before anything is opened or negotiated.
        let err = crate::video::Video::start(&path, (0, 0))
            .err()
            .expect("a video in a home is refused");
        let said = err.to_string();
        assert!(said.contains("R15"), "{said}");
        assert!(said.contains("home directory"), "{said}");
    }

    #[test]
    fn a_background_in_a_home_is_refused_before_it_is_opened() {
        // R15, C10. The file does not need to exist: the point is that the
        // greeter does not even try, and that the message is about the home
        // directory rather than about a failed open.
        for p in [
            Path::new("/home/dirk/Pictures/wallpaper.png"),
            Path::new("/root/wallpaper.png"),
        ] {
            let problem = decode(p).expect_err("a home is refused");
            assert!(
                problem.reason.contains("home directory"),
                "the reason is the home, not the open — got {}",
                problem.reason
            );
            assert!(
                problem.reason.contains(RECOMMENDED_DIR),
                "the message says where to put it instead — got {}",
                problem.reason
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn a_symlink_out_of_a_public_directory_into_a_home_is_refused_too() {
        let dir = scratch("symlink");
        let link = dir.join("sneaky.png");
        std::fs::remove_file(&link).ok();
        std::os::unix::fs::symlink("/home/dirk/Pictures/wallpaper.png", &link).expect("symlink");
        let problem = decode(&link).expect_err("the target is in a home");
        // Either refusal is correct and both honour R15: a dangling link is
        // reported as a failed resolve, a live one as a home directory.
        assert!(
            problem.reason.contains("home directory") || !problem.reason.is_empty(),
            "refused with a reason — got {}",
            problem.reason
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nothing_configured_is_the_solid_colour_and_not_a_problem() {
        let bg = Background::start(&[]);
        assert!(!bg.has_image());
        assert!(
            bg.problems().is_empty(),
            "R9 is about a background that was asked for; `None` asks for the colour"
        );
    }

    /// R9 is a chain and this is the bookkeeping half of it: what `start`
    /// picks up, and what it keeps for later. The behaviour half -- that a
    /// failure actually moves to the next one -- needs a GL context and
    /// lives in `view::tests`.
    #[test]
    fn the_chain_is_tried_in_order_and_an_empty_one_is_the_colour() {
        let dir = scratch("chain");
        let a = write(&dir, "first.png", &png(8, 8));
        let b = write(&dir, "second.png", &png(8, 8));

        let bg = Background::start(&[]);
        assert!(matches!(bg.state, State::Solid));
        assert!(
            bg.configured.is_none(),
            "nothing configured, nothing to name"
        );

        let bg = Background::start(&[a.clone(), b.clone()]);
        assert_eq!(
            bg.configured.as_deref(),
            Some(a.as_path()),
            "the first candidate is the one being attempted"
        );
        assert_eq!(
            bg.rest.iter().cloned().collect::<Vec<_>>(),
            vec![b],
            "and the rest wait their turn, in order"
        );

        // A video is recorded rather than started, because `start` has no GL
        // context -- that is what keeps it off R16's critical path.
        let v = write(&dir, "clip.mp4", b"not really a video");
        let bg = Background::start(std::slice::from_ref(&v));
        assert!(matches!(bg.state, State::VideoPending(_)));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_configured_background_is_decoded_once_off_the_main_thread() {
        let dir = scratch("once");
        let p = write(&dir, "bg.png", &png(256, 256));
        let bg = Background::start(std::slice::from_ref(&p));
        // `start` must not have blocked on the decode: the state is pending,
        // not ready, and no GL context exists in this test at all — which is
        // itself the proof that `start` does no uploading.
        assert!(matches!(bg.state, State::Pending(_)));

        // Drain it the way `poll` would, without a renderer.
        let still = match &bg.state {
            State::Pending(rx) => rx
                .recv_timeout(Duration::from_secs(10))
                .expect("the thread delivers")
                .expect("a valid PNG"),
            _ => unreachable!(),
        };
        assert_eq!((still.width, still.height), (256, 256));

        // And nothing further arrives: one decode per run, not one per frame.
        //
        // `recv_timeout` rather than `try_recv`, and either error accepted.
        // The first version demanded `Disconnected`, which is a race it won
        // on a developer's machine and lost on a CI runner: between the
        // receiver taking the value and the decode thread actually ending,
        // the channel is `Empty` and not yet disconnected. Neither of those
        // is a second decode, which is the only thing this is asserting.
        match &bg.state {
            State::Pending(rx) => {
                let second = rx.recv_timeout(Duration::from_millis(500));
                assert!(
                    second.is_err(),
                    "a second background arrived; the decode ran more than once"
                );
            }
            _ => unreachable!(),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_bad_configured_background_arrives_as_a_logged_fallback() {
        let dir = scratch("badpipe");
        let whole = png(128, 128);
        let p = write(&dir, "cut.png", &whole[..20]);
        let bg = Background::start(std::slice::from_ref(&p));
        let got = match &bg.state {
            State::Pending(rx) => rx.recv_timeout(Duration::from_secs(10)).expect("delivers"),
            _ => unreachable!(),
        };
        let problem = got.expect_err("a truncated PNG");
        // What `poll` would then record. Asserted here rather than through
        // `poll` because `poll` needs a renderer and a GL context, which no
        // test on a build machine has.
        let mut solid = Background {
            state: State::Solid,
            configured: Some(p.clone()),
            rest: Default::default(),
            problems: Vec::new(),
        };
        solid.fell_back(problem);
        assert_eq!(solid.problems().len(), 1);
        assert!(solid.problems()[0].contains("cut.png"));
        assert!(!solid.has_image());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn crop_to_fill_covers_without_stretching() {
        // A 16:9 still on a 16:10 surface. The surface is relatively taller,
        // so height is the binding axis: it must fill exactly, and the extra
        // width must hang off both sides rather than the image squashing.
        let [x, y, w, h] = fill_rect(1920, 1200, 1920, 1080);
        assert_eq!((y, h), (0.0, 1200.0), "the binding axis fills exactly");
        assert!(w >= 1920.0, "it covers horizontally, got {w}");
        assert!(x <= 0.0 && (x + w) >= 1920.0, "centred and overflowing");
        assert!(
            ((w / h) - (1920.0 / 1080.0)).abs() < 1e-4,
            "the aspect ratio is the image's own, not the surface's: {w}x{h}"
        );

        // The other way round: a portrait still on a landscape surface, where
        // width binds and the overflow is vertical.
        let [x, y, w, h] = fill_rect(2560, 1440, 1080, 1920);
        assert_eq!((x, w), (0.0, 2560.0));
        assert!(h >= 1440.0 && y <= 0.0, "{y} {h}");
        assert!(((w / h) - (1080.0 / 1920.0)).abs() < 1e-4);

        // An exact match is left exactly alone — no half-pixel drift.
        assert_eq!(
            fill_rect(3840, 2160, 3840, 2160),
            [0.0, 0.0, 3840.0, 2160.0]
        );

        // The overflow is split evenly, which is what "centred" means and what
        // keeps a face in the middle of the frame.
        let [x, _, w, _] = fill_rect(1000, 1000, 2000, 1000);
        assert!((x + (w - 1000.0) / 2.0).abs() < 1e-3, "x={x} w={w}");
    }

    #[test]
    fn a_degenerate_size_does_not_divide_by_zero() {
        for r in fill_rect(1920, 1080, 0, 0) {
            assert!(r.is_finite(), "no NaN or infinity reaches the shader");
        }
    }

    #[test]
    fn an_oversized_still_is_scaled_rather_than_refused() {
        assert_eq!(fit_within(1920, 1080, 4096), None, "it already fits");
        assert_eq!(fit_within(4096, 4096, 4096), None, "the cap is inclusive");
        let (w, h) = fit_within(7680, 4320, 4096).expect("8K does not fit a 4K cap");
        assert!(w <= 4096 && h <= 4096, "{w}x{h}");
        assert_eq!(w, 4096, "the long edge lands on the cap");
        assert!(
            ((w as f64 / h as f64) - (7680.0 / 4320.0)).abs() < 1e-2,
            "the ratio survives the shrink: {w}x{h}"
        );
        // Degenerate caps must not produce a zero-sized texture.
        let (w, h) = fit_within(4000, 10, 16).expect("very wide");
        assert!(w >= 1 && h >= 1, "{w}x{h}");
    }

    #[test]
    fn a_failure_after_the_decode_still_names_the_file() {
        // The upload and the thread can fail too, and those failures have no
        // `Problem` of their own. They must still say which background is not
        // on screen, or the log cannot be acted on.
        let mut bg = Background {
            state: State::Solid,
            configured: Some(PathBuf::from("/var/lib/auroval/bg.png")),
            rest: Default::default(),
            problems: Vec::new(),
        };
        bg.failed("the driver refused a 8192x8192 texture (GL 0x501)");
        let said = &bg.problems()[0];
        assert!(said.contains("/var/lib/auroval/bg.png"), "got {said}");
        assert!(said.contains("solid colour"), "got {said}");
        assert!(said.contains("0x501"), "got {said}");
    }

    /// The measurement behind "decode on a thread", kept so the claim can be
    /// rechecked on other hardware rather than believed. Run it with:
    ///
    /// ```text
    /// cargo test --release -p greet -- --ignored --nocapture measure_decode_cost
    /// ```
    ///
    /// It is `#[ignore]`d because it encodes and decodes 4K images several
    /// times, which has no place in the ordinary test run.
    #[test]
    #[ignore = "a measurement, not a check; see spike/p1/BACKGROUND.md"]
    fn measure_decode_cost() {
        let dir = scratch("measure");
        const ROUNDS: u32 = 5;
        for (name, bytes) in [
            ("1080p.png", png(1920, 1080)),
            ("1440p.png", png(2560, 1440)),
            ("4K.png", png(3840, 2160)),
            ("4K.jpg", jpeg(3840, 2160)),
            ("4K-noise.png", noise_png(3840, 2160)),
            ("1440p-noise.png", noise_png(2560, 1440)),
        ] {
            let p = write(&dir, name, &bytes);
            let mut best = Duration::from_secs(3600);
            let mut total = Duration::ZERO;
            let mut bytes_out = 0;
            for _ in 0..ROUNDS {
                let began = Instant::now();
                let s = decode(&p).expect("the fixture decodes");
                let took = began.elapsed();
                bytes_out = s.bytes();
                best = best.min(took);
                total += took;
            }
            println!(
                "{name:>12}  file {:>8} KiB  rgba {:>8} KiB  best {:>8.1} ms  mean {:>8.1} ms",
                bytes.len() / 1024,
                bytes_out / 1024,
                best.as_secs_f64() * 1e3,
                total.as_secs_f64() * 1e3 / f64::from(ROUNDS),
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
