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
