//! A glyph atlas, because GL has no text.
//!
//! The spike blitted glyphs into a CPU buffer, which `wl_shm` made possible
//! and the GL path does not. So glyphs are rasterised once, packed into one
//! single-channel texture, and drawn as quads.
//!
//! **Fonts are loaded by path, never through fontconfig.** C15 measured the
//! greeter reporting *"No writable cache directories"* because its home is `/`
//! and not writable; a font that needs a cache is a font that fails in the one
//! context that matters. R11 names MesloLGS Nerd Font, so that is what is
//! looked for first, with a fallback list and a log line naming what was
//! actually loaded — the wrong font is a visible failure, not a silent one.

use std::collections::HashMap;

use glow::HasContext;

pub const FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/TTF/MesloLGSNerdFont-Regular.ttf",
    "/usr/share/fonts/TTF/MesloLGSNerdFontMono-Regular.ttf",
    "/usr/share/fonts/TTF/MesloLGSDZNerdFont-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    // Debian and Ubuntu lay fonts out differently, and the render tests have
    // to be able to find *a* font wherever the workspace is built: a CI runner
    // is the one machine that will never have MesloLGS installed. These two
    // are last on purpose -- on the machine this greeter actually runs on,
    // they are never reached.
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
];

const ATLAS: u32 = 2048;

#[derive(Clone, Copy)]
pub struct Glyph {
    /// u0, v0, u1, v1 in the atlas.
    pub uv: [f32; 4],
    pub w: f32,
    pub h: f32,
    /// Offset from the pen position to the glyph's top-left corner.
    pub left: f32,
    pub top: f32,
    pub advance: f32,
}

pub struct Atlas {
    font: fontdue::Font,
    pub source: String,
    tex: glow::Texture,
    cache: HashMap<(char, u32), Glyph>,
    pen_x: u32,
    pen_y: u32,
    row_h: u32,
    full: bool,
}

impl Atlas {
    /// Load the first font that parses. The path used is kept in `source` so
    /// the caller can say which one it got.
    pub fn new(gl: &glow::Context, candidates: &[&str]) -> Result<Atlas, String> {
        let (font, source) = candidates
            .iter()
            .find_map(|p| {
                let bytes = std::fs::read(p).ok()?;
                let f = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
                Some((f, (*p).to_string()))
            })
            .ok_or_else(|| format!("no usable font; tried {candidates:?}"))?;

        let tex = unsafe {
            let t = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(t));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::ALPHA as i32,
                ATLAS as i32,
                ATLAS as i32,
                0,
                glow::ALPHA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            for (k, v) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
            }
            // Glyph bitmaps are one tightly packed byte per pixel.
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            t
        };

        Ok(Atlas {
            font,
            source,
            tex,
            cache: HashMap::new(),
            pen_x: 1,
            pen_y: 1,
            row_h: 0,
            full: false,
        })
    }

    pub fn texture(&self) -> glow::Texture {
        self.tex
    }

    /// Rasterise and pack one glyph, or return the cached one. `None` when the
    /// atlas is full: the caller skips that glyph rather than failing, because
    /// a login screen missing one character still logs you in.
    pub fn glyph(&mut self, gl: &glow::Context, ch: char, px: f32) -> Option<Glyph> {
        let key = (ch, px.to_bits());
        if let Some(g) = self.cache.get(&key) {
            return Some(*g);
        }
        if self.full {
            return None;
        }
        let (m, bitmap) = self.font.rasterize(ch, px);
        let (w, h) = (m.width as u32, m.height as u32);

        if w > 0 && h > 0 {
            if self.pen_x + w + 1 >= ATLAS {
                self.pen_x = 1;
                self.pen_y += self.row_h + 1;
                self.row_h = 0;
            }
            if self.pen_y + h + 1 >= ATLAS {
                self.full = true;
                return None;
            }
            unsafe {
                gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    self.pen_x as i32,
                    self.pen_y as i32,
                    w as i32,
                    h as i32,
                    glow::ALPHA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&bitmap)),
                );
            }
        }

        let a = ATLAS as f32;
        let g = Glyph {
            uv: [
                self.pen_x as f32 / a,
                self.pen_y as f32 / a,
                (self.pen_x + w) as f32 / a,
                (self.pen_y + h) as f32 / a,
            ],
            w: w as f32,
            h: h as f32,
            left: m.xmin as f32,
            // fontdue measures upward from the baseline; everything else here
            // measures downward from the top.
            top: -(m.height as f32 + m.ymin as f32),
            advance: m.advance_width,
        };
        if w > 0 && h > 0 {
            self.pen_x += w + 1;
            self.row_h = self.row_h.max(h);
        }
        self.cache.insert(key, g);
        Some(g)
    }

    /// Width of a string at a size, for centring. Taken from the same metrics
    /// the drawing uses, so the two cannot disagree.
    pub fn width(&self, text: &str, px: f32) -> f32 {
        text.chars()
            .map(|c| self.font.metrics(c, px).advance_width)
            .sum()
    }

    /// Baseline offset from the vertical centre of a line box, so a line can
    /// be placed by its visual centre rather than by its baseline.
    ///
    /// It is `(ascent - descent) / 2`, and the descent is the half that was
    /// missing. Centring on the ascent alone drops the line by half a descent
    /// -- 18 px on the 147 px clock, which is what put this greeter's clock
    /// below hyprlock's when the two were measured side by side on
    /// 2026-11-08. Pango centres the full logical line box, so matching it
    /// means counting both ends.
    ///
    /// fontdue reports the descent as a negative number, so this adds.
    pub fn baseline_from_centre(&self, px: f32) -> f32 {
        self.font
            .horizontal_line_metrics(px)
            .map(|m| (m.ascent + m.descent) * 0.5)
            .unwrap_or(px * 0.3)
    }
}
