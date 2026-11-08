//! R11 and R17: hyprlock's layout, drawn from `UiState` and nothing else.
//!
//! This is a pure function of its input by design. The seam exists so the
//! picture can be written and checked without Wayland, greetd or a working
//! greeter — `draw` takes a `Gl` that may be rendering to an output or to an
//! offscreen buffer, and does not know which.
//!
//! The numbers all come from `theme::Geometry`, which was read off the live
//! `hyprlock.conf` rather than invented. hyprlock places from the centre with
//! y upward; here y runs downward, because that is how a framebuffer is
//! addressed, and the signs flip in `Geometry` rather than here.

use crate::render::{Field, Gl};
use crate::theme::{Geometry, Palette};
use crate::ui::{FieldDisplay, NoticeKind, Stage, UiState};

/// Everything the view needs that is not in `UiState`.
pub struct Style {
    pub geometry: Geometry,
    pub palette: Palette,
}

pub fn draw(
    gl: &mut Gl,
    ui: &UiState,
    style: &Style,
    background: Option<(glow::Texture, u32, u32)>,
) {
    let (w, h) = gl.size();
    let (cx, cy) = (w * 0.5, h * 0.5);
    let g = &style.geometry;
    let p = &style.palette;

    // R9's last resort is a solid colour. It is drawn first whatever happens,
    // because it is also what fills the strip a crop-to-fill cannot reach and
    // what shows while a decode is still in flight.
    gl.clear([0.07, 0.07, 0.11, 1.0]);
    if let Some((tex, iw, ih)) = background {
        // Crop to fill, never stretch: an aspect-ratio mismatch must not
        // distort a face. `fill_rect` is the background module's and is
        // tested on its own.
        let [x, y, rw, rh] = crate::background::fill_rect(w as u32, h as u32, iw, ih);
        gl.draw_image(tex, x, y, rw, rh, [1.0, 1.0, 1.0, 1.0]);
    }

    // The clock and date carry a drop shadow in hyprlock (shadow_passes = 2).
    // One offset copy is not two passes of blur, and saying so is better than
    // pretending: it reads as a shadow at these sizes and costs one draw.
    shadowed(
        gl,
        &ui.clock,
        g.clock_px,
        cx,
        cy + g.clock_dy,
        p.on_surface,
        p.shadow,
    );
    shadowed(
        gl,
        &ui.date,
        g.date_px,
        cx,
        cy + g.date_dy,
        p.outline,
        p.shadow,
    );

    let who = if ui.username.is_empty() {
        "—"
    } else {
        &ui.username
    };
    gl.draw_centred(who, g.user_px, cx, cy + g.user_dy, p.primary);

    // The input field. Its outline says what state the login is in, which is
    // hyprlock's own convention: check_color while it works, fail_color when
    // it failed, outer_color otherwise.
    let stroke = match (&ui.notice, ui.stage) {
        (_, Stage::Starting) => p.check,
        (Some(n), _) if n.kind == NoticeKind::Error => p.error,
        _ => p.primary,
    };
    let fx = cx - g.field_w * 0.5;
    let fy = cy + g.field_dy - g.field_h * 0.5;
    gl.draw_field(&Field {
        x: fx,
        y: fy,
        w: g.field_w,
        h: g.field_h,
        radius: g.field_rounding,
        power: g.rounding_power,
        outline: g.outline,
        fill: p.surface,
        stroke,
    });

    let text_px = g.field_h * 0.36;
    match ui.field_display() {
        FieldDisplay::Dots(n) => dots(gl, n, fx, fy, g, p),
        FieldDisplay::Text("") => {
            // hyprlock's placeholder, in the dimmer role so it reads as a
            // hint and not as content.
            let hint = match ui.stage {
                Stage::AskingUser => "Who are you?",
                Stage::Answering => ui.prompt.as_str(),
                Stage::Starting => "starting…",
            };
            gl.draw_centred(hint, text_px, cx, fy + g.field_h * 0.5, p.outline);
        }
        FieldDisplay::Text(t) => {
            gl.draw_centred(t, text_px, cx, fy + g.field_h * 0.5, p.on_surface);
        }
    }

    // R17: `info` and `error` are content, not a status line. They are given
    // the date's size rather than a caption's, and sit below the field where
    // there is room for a sentence.
    if let Some(n) = &ui.notice {
        let colour = if n.kind == NoticeKind::Error {
            p.error
        } else {
            p.on_surface
        };
        gl.draw_centred(&n.text, g.date_px, cx, fy + g.field_h + 48.0, colour);
    }

    if ui.caps_lock {
        // Beside the field rather than above it: the username line already
        // sits 80 px up, and stacking a warning between them read as cramped
        // the first time it was drawn.
        let x = cx + g.field_w * 0.5 + 72.0;
        gl.draw_centred("CAPS LOCK", g.user_px, x, fy + g.field_h * 0.5, p.error);
    }

    // The session, bottom centre: present enough to check, quiet enough to
    // ignore. Tab cycles it.
    if let Some(s) = ui.session() {
        let label = if ui.sessions.len() > 1 {
            format!("{}  ·  tab to change", s.name)
        } else {
            s.name.clone()
        };
        gl.draw_centred(&label, g.user_px, cx, h - 64.0, p.outline);
    }
}

fn shadowed(
    gl: &mut Gl,
    s: &str,
    px: f32,
    cx: f32,
    cy: f32,
    colour: crate::theme::Rgba,
    shadow: crate::theme::Rgba,
) {
    if s.is_empty() {
        return;
    }
    let off = (px * 0.04).max(2.0);
    gl.draw_centred(s, px, cx + off, cy + off, shadow);
    gl.draw_centred(s, px, cx, cy, colour);
}

/// hyprlock's dots: `dots_size` and `dots_spacing` are fractions of the field
/// height, which is why they are 0.26 and 0.3 rather than pixel counts.
fn dots(gl: &mut Gl, n: usize, fx: f32, fy: f32, g: &Geometry, p: &Palette) {
    if n == 0 {
        return;
    }
    let d = g.field_h * g.dots_size;
    let gap = d * (1.0 + g.dots_spacing);
    // A long password must not grow out of its field, so the row is clamped
    // to what fits and the rest is simply not drawn. The alternative — an
    // ever-wider row — leaks the length across the whole screen.
    let max = ((g.field_w - d) / gap).floor().max(1.0) as usize;
    let shown = n.min(max);
    let total = gap * (shown.saturating_sub(1)) as f32;
    let mut x = fx + g.field_w * 0.5 - total * 0.5 - d * 0.5;
    let y = fy + g.field_h * 0.5 - d * 0.5;
    for _ in 0..shown {
        gl.draw_field(&Field {
            x,
            y,
            w: d,
            h: d,
            radius: d * 0.5,
            power: 2.0, // a dot is a circle, whatever the field's corners are
            outline: 0.0,
            fill: p.on_surface,
            stroke: p.on_surface,
        });
        x += gap;
    }
}

/// Headless render tests: a known `UiState` rendered to pixels and measured.
///
/// These are the checks the rest of the suite structurally cannot make. A
/// parse test proves `Geometry` holds hyprlock's numbers; only a rendered
/// frame proves the picture does. The two that matter most are written
/// against the failure rather than the success: the superellipse test fails
/// if `rounding_power` is ignored, and the background test fails if a
/// wrong-aspect image is stretched. Both were confirmed to fail when the
/// behaviour they describe was deliberately broken.
///
/// **They need a working EGL context**, which is why they are not `#[ignore]`d
/// and will not be: a test that has to be asked for does not catch a
/// regression. `Gl::headless` is surfaceless EGL, so Mesa's software
/// rasteriser satisfies it and no GPU is required — but a machine with no EGL
/// at all fails these loudly rather than skipping them silently, which is the
/// right way round. CI took it that way round: the first run of these failed
/// on `eglInitialize`, and the fix was to install Mesa on the runner
/// (`.github/actions/rust-build-deps`), not to let the tests opt out. The
/// `test` job pins `llvmpipe` through `EGL_PLATFORM` and friends, because a
/// pixel assertion wants the same rasteriser every run.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::Background;
    use crate::render::Gl;
    use crate::store::LastLogin;
    use crate::ui::Stage;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// Everything here renders through one off-screen GL context, and a GL
    /// context is not a thing to create from several test threads at once.
    /// `cargo test` runs tests in parallel by default, so they take turns.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// The surface R11 and R16 are both stated at. Rendering these at the
    /// real resolution is the point: hyprlock's numbers are pixel counts, and
    /// a check at some other size would be checking arithmetic rather than
    /// the picture.
    const W: u32 = 2560;
    const H: u32 = 1440;

    /// What the clear colour is in `draw`, as bytes. Everything that is not
    /// this is something the view drew.
    const CLEAR: [u8; 3] = [18, 18, 28];

    struct Frame {
        px: Vec<u8>,
    }

    impl Frame {
        fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
            let i = ((y * W + x) * 4) as usize;
            [self.px[i], self.px[i + 1], self.px[i + 2]]
        }

        fn drawn(&self, x: u32, y: u32) -> bool {
            let c = self.rgb(x, y);
            // A tolerance, not equality: the clear colour arrives through a
            // float framebuffer and need not come back as the exact bytes.
            (0..3).any(|i| (c[i] as i32 - CLEAR[i] as i32).abs() > 3)
        }

        /// The run of drawn pixels containing `(x, y)`, along one axis.
        ///
        /// Deliberately not a bounding box over the whole frame: the clock,
        /// the date and the username are also drawn, and a bounding box would
        /// quietly include them. A run through a point inside the field can
        /// only be the field.
        fn run_x(&self, x: u32, y: u32) -> (u32, u32) {
            assert!(self.drawn(x, y), "({x},{y}) is not drawn at all");
            let mut a = x;
            while a > 0 && self.drawn(a - 1, y) {
                a -= 1;
            }
            let mut b = x;
            while b + 1 < W && self.drawn(b + 1, y) {
                b += 1;
            }
            (a, b - a + 1)
        }

        fn run_y(&self, x: u32, y: u32) -> (u32, u32) {
            assert!(self.drawn(x, y), "({x},{y}) is not drawn at all");
            let mut a = y;
            while a > 0 && self.drawn(x, a - 1) {
                a -= 1;
            }
            let mut b = y;
            while b + 1 < H && self.drawn(x, b + 1) {
                b += 1;
            }
            (a, b - a + 1)
        }
    }

    fn style() -> Style {
        Style {
            geometry: Geometry::default(),
            palette: Palette::default(),
        }
    }

    fn state() -> UiState {
        let mut ui = UiState::new(Vec::new(), &LastLogin::default());
        ui.stage = Stage::Answering;
        ui.prompt = "Password:".into();
        ui.secret = true;
        ui.typed = "hunter2".into();
        ui.clock = "23:26".into();
        ui.date = "Saturday, 7 November".into();
        ui
    }

    /// Render one frame off-screen and read it back.
    fn render(ui: &UiState, st: &Style, background: Option<&Path>) -> Frame {
        render_each(&[(ui, st)], background)
            .pop()
            .expect("one frame")
    }

    /// Render several frames through **one** context and read each back.
    ///
    /// Not a loop over `render`: a pixel diff between two frames from two
    /// different GL contexts is a diff between two rasterisers as much as
    /// between two inputs, and the point of the superellipse test is that
    /// the only difference is the one it changed.
    fn render_each(frames: &[(&UiState, &Style)], background: Option<&Path>) -> Vec<Frame> {
        let (mut gl, target) =
            unsafe { Gl::headless(W, H) }.expect("an off-screen GL context for the render tests");
        gl.make_current(&target).expect("make current");

        let mut bg = Background::start(background);
        let ready = if background.is_some() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !bg.settled(&gl) && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(
                bg.problems().is_empty(),
                "the fixture background did not decode: {:?}",
                bg.problems()
            );
            let r = bg.ready(&gl);
            assert!(r.is_some(), "the fixture background never arrived");
            r
        } else {
            None
        };

        frames
            .iter()
            .map(|(ui, st)| {
                draw(&mut gl, ui, st, ready);
                Frame {
                    px: gl.read_pixels(),
                }
            })
            .collect()
    }

    /// Where the field must land, computed from `Geometry` the way `draw`
    /// does. The test asserts the pixels agree with this, which is the whole
    /// point: the arithmetic and the picture are checked against each other
    /// rather than the arithmetic against itself.
    fn expected_field(g: &Geometry) -> (u32, u32, u32, u32) {
        let (cx, cy) = (W as f32 * 0.5, H as f32 * 0.5);
        (
            (cx - g.field_w * 0.5) as u32,
            (cy + g.field_dy - g.field_h * 0.5) as u32,
            g.field_w as u32,
            g.field_h as u32,
        )
    }

    /// R11, in pixels rather than in `Geometry`. hyprlock's input field is
    /// 340x56 at `position 0, -40`, and this is the check that the rendered
    /// frame actually says so — the parse is covered by
    /// `theme::tests::geometry_matches_the_live_hyprlock_config`, and a parse
    /// that is right while the picture is wrong is exactly what that test
    /// cannot see.
    #[test]
    fn the_input_field_renders_at_hyprlocks_own_size_and_place() {
        let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let st = style();
        let (ex, ey, ew, eh) = expected_field(&st.geometry);
        let f = render(&state(), &st, None);

        // Through the middle of the field, where nothing else is.
        let (x0, w) = f.run_x(ex + 3, ey + eh / 2);
        assert_eq!((x0, w), (ex, ew), "field width and left edge");

        // And down a column three pixels inside the left edge, which is in
        // the outline and the fill but clear of any centred text.
        let (y0, h) = f.run_y(ex + 3, ey + eh / 2);
        assert_eq!((y0, h), (ey, eh), "field height and top edge");
    }

    /// R11's trap, and #36 names it: `rounding_power = 4` is part of the
    /// requirement and a circular rounded rect would quietly satisfy every
    /// other check. So this renders the same frame twice, changing nothing
    /// but the power, and insists the picture changes — and changes only in
    /// the corners, by the radius, at all four of them.
    ///
    /// An empty diff is the failure this exists to catch: it would mean the
    /// shader takes the parameter and ignores it.
    #[test]
    fn the_field_corners_are_a_superellipse_and_not_a_circle() {
        let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let st = style();
        let (ex, ey, ew, eh) = expected_field(&st.geometry);
        let r = st.geometry.field_rounding as u32;

        let mut circular = style();
        circular.geometry.rounding_power = 2.0; // a plain circular rounded rect
        let ui = state();
        let pair = render_each(&[(&ui, &st), (&ui, &circular)], None);
        let (a, b) = (&pair[0], &pair[1]);

        let mut corners = [0u32; 4];
        let mut total = 0u32;
        for y in 0..H {
            for x in 0..W {
                let (p, q) = (a.rgb(x, y), b.rgb(x, y));
                if (0..3).all(|i| (p[i] as i32 - q[i] as i32).abs() <= 8) {
                    continue;
                }
                total += 1;
                let left = x < ex + r;
                let right = x >= ex + ew - r;
                let top = y < ey + r;
                let bottom = y >= ey + eh - r;
                assert!(
                    (left || right) && (top || bottom),
                    "the power changed a pixel at ({x},{y}), which is not within \
                     {r} px of a corner of the {ew}x{eh} field at ({ex},{ey})"
                );
                corners[(right as usize) + 2 * (bottom as usize)] += 1;
            }
        }

        assert!(
            total > 0,
            "rounding_power changed nothing: the shader is ignoring it, and a \
             circular rounded rect would pass every other check in this file"
        );
        assert!(
            corners.iter().all(|&c| c > 0),
            "only some corners changed: {corners:?}"
        );
        let first = corners[0];
        assert!(
            corners.iter().all(|&c| c.abs_diff(first) <= 2),
            "the four corners should differ from each other by at most \
             antialiasing, got {corners:?}"
        );
    }

    /// R9, the half that only pixels can settle: a background whose aspect
    /// ratio does not match the surface must be cropped to fill, never
    /// stretched. `background::tests::crop_to_fill_covers_without_stretching`
    /// covers `fill_rect`'s arithmetic; this covers what reaches the screen.
    ///
    /// The probe is a circle, because a circle that is still a circle cannot
    /// have been stretched. It is deliberately small enough that it survives
    /// the crop whole: a probe larger than the surface measures the surface.
    #[test]
    fn a_background_of_the_wrong_aspect_is_cropped_and_not_stretched() {
        let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (sw, sh) = (800u32, 2000u32);
        let scale = (W as f32 / sw as f32).max(H as f32 / sh as f32); // 3.2
        let radius = 100u32;
        let expect = (2 * radius) as f32 * scale; // 640 px across

        let path = write_circle_png(sw, sh, radius);
        let f = render(&state(), &style(), Some(&path));

        // The probe is pure red and nothing else in the view is.
        let (mut x0, mut x1, mut y0, mut y1) = (W, 0u32, H, 0u32);
        for y in 0..H {
            for x in 0..W {
                let [r, g, b] = f.rgb(x, y);
                if r as i32 > g as i32 + 30 && r as i32 > b as i32 + 30 {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        assert!(x1 >= x0, "the background never reached the frame");

        let (w, h) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        // Stretch-to-fit would give (2560/800)/(1440/2000) = 4.44.
        assert!(
            (w / h - 1.0).abs() < 0.02,
            "the probe came out {w}x{h}, ratio {:.4}; a circle that is no \
             longer round has been stretched",
            w / h
        );
        assert!(
            (w - expect).abs() < 8.0,
            "the probe is {w} px across, expected about {expect} at cover \
             scale {scale}"
        );
    }

    /// A tall, narrow PNG with a red circle in the middle, written outside
    /// any home so that R15 does not refuse it for the wrong reason.
    fn write_circle_png(w: u32, h: u32, radius: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("greet-view-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch directory");
        let path = dir.join(format!("circle-{w}x{h}-r{radius}.png"));

        let mut img = image::RgbaImage::new(w, h);
        let (cx, cy) = (w as i64 / 2, h as i64 / 2);
        let r2 = (radius as i64) * (radius as i64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let (dx, dy) = (x as i64 - cx, y as i64 - cy);
            *p = if dx * dx + dy * dy <= r2 {
                image::Rgba([255, 40, 40, 255])
            } else {
                image::Rgba([20, 20, 30, 255])
            };
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("encodes");
        let mut f = std::fs::File::create(&path).expect("fixture");
        f.write_all(&bytes.into_inner()).expect("fixture");
        path
    }
}
