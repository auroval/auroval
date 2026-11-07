//! One text label, rasterised once.
//!
//! Both paths pay for the same label, because R11's typography is auroval's
//! problem on the GL path and nearly free on the shm path, and leaving it out
//! of the comparison would flatter GL. This is not a glyph atlas with shaping
//! and caching --- that is P1 work --- it is the cheapest thing that still
//! puts real glyph coverage on screen every frame.

pub struct Label {
    pub w: usize,
    pub h: usize,
    /// Coverage, one byte per pixel.
    pub alpha: Vec<u8>,
}

/// R11 names MesloLGS; the spike takes the same list spike-p0 uses so the
/// glyph work being measured is the glyph work the greeter would do.
const CANDIDATES: &[&str] = &[
    "/usr/share/fonts/TTF/MesloLGSNerdFontMono-Regular.ttf",
    "/usr/share/fonts/TTF/MesloLGSNerdFont-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
];

fn load_font() -> fontdue::Font {
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(f) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                return f;
            }
        }
    }
    panic!("no usable font found; tried {CANDIDATES:?}");
}

pub fn label(text: &str, px: f32) -> Label {
    let font = load_font();
    let mut pen = 0usize;
    let mut glyphs = Vec::new();
    let mut top = 0i32;
    let mut bottom = 0i32;
    for ch in text.chars() {
        let (m, bitmap) = font.rasterize(ch, px);
        top = top.max(m.height as i32 + m.ymin);
        bottom = bottom.min(m.ymin);
        glyphs.push((pen, m, bitmap));
        pen += m.advance_width.ceil() as usize;
    }
    let h = (top - bottom).max(1) as usize;
    let w = pen.max(1);
    let mut alpha = vec![0u8; w * h];
    for (x0, m, bitmap) in glyphs {
        let gy = (top - m.height as i32 - m.ymin).max(0) as usize;
        for row in 0..m.height {
            for col in 0..m.width {
                let x = x0 + col + m.xmin.max(0) as usize;
                let y = gy + row;
                if x < w && y < h {
                    alpha[y * w + x] = alpha[y * w + x].max(bitmap[row * m.width + col]);
                }
            }
        }
    }
    Label { w, h, alpha }
}
