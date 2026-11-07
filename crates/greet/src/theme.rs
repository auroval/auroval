//! R11: hyprlock's geometry and typography, and the palette that colours it.
//!
//! The numbers are not invented. They are read off the live `hyprlock.conf`
//! this project is trying to match, and the whole point of the requirement is
//! that they are identical rather than similar:
//!
//! ```text
//! label  $TIME   font_size 110  position 0, 220   MesloLGS Nerd Font
//! label  date    font_size  22  position 0, 130
//! label  $USER   font_size  16  position 0,  40
//! input-field    size 340, 56   rounding 12  outline_thickness 2  position 0, -40
//! ```
//!
//! hyprlock places from the centre with y upward; this places from the centre
//! with y downward, because that is how a framebuffer is addressed. The signs
//! flip and nothing else does.

/// Where each element sits, relative to the centre of the output, in logical
/// pixels at scale 1. Sizes scale; the layout does not move.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    pub clock_px: f32,
    pub clock_dy: f32,
    pub date_px: f32,
    pub date_dy: f32,
    pub user_px: f32,
    pub user_dy: f32,
    pub field_w: f32,
    pub field_h: f32,
    pub field_dy: f32,
    pub field_rounding: f32,
    /// hyprland's `rounding_power = 4`: the corner bends toward a squircle
    /// rather than a circular arc, which reads softer at the same radius.
    pub rounding_power: f32,
    pub outline: f32,
    pub dots_size: f32,
    pub dots_spacing: f32,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            clock_px: 110.0,
            clock_dy: -220.0,
            date_px: 22.0,
            date_dy: -130.0,
            user_px: 16.0,
            user_dy: -40.0,
            field_w: 340.0,
            field_h: 56.0,
            field_dy: 40.0,
            field_rounding: 12.0,
            rounding_power: 4.0,
            outline: 2.0,
            dots_size: 0.26,
            dots_spacing: 0.3,
        }
    }
}

/// Straight RGBA, 0..1, already in linear-ish sRGB values as the shader wants
/// them. Stored as four floats rather than a hex string so no conversion
/// happens per frame.
pub type Rgba = [f32; 4];

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub on_surface: Rgba,
    pub outline: Rgba,
    pub primary: Rgba,
    pub surface: Rgba,
    pub error: Rgba,
    pub check: Rgba,
    pub shadow: Rgba,
}

impl Default for Palette {
    /// The live `hyprlock.conf`'s own colours, so that P1 can be held up next
    /// to hyprlock and compared. **This is a placeholder**: R11 says colour
    /// comes from the active theme's palette, and theme packages do not exist
    /// yet. When they do, this becomes the fallback for "no theme installed"
    /// rather than the normal path.
    fn default() -> Self {
        Palette {
            on_surface: rgb(0xcd, 0xd6, 0xf4),
            outline: rgb(0xa6, 0xad, 0xc8),
            primary: rgb(0x89, 0xb4, 0xfa),
            surface: rgba(0x1e, 0x1e, 0x2e, 0xcc),
            error: rgb(0xf3, 0x8b, 0xa8),
            check: rgb(0xcb, 0xa6, 0xf7),
            shadow: rgba(0x11, 0x11, 0x1b, 0xaa),
        }
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    rgba(r, g, b, 0xff)
}

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

impl Palette {
    // Written here because a palette is a theme concern, used by the Lua
    // loader in #33. Until that lands nothing in the binary calls it.
    #[allow(dead_code)]
    /// Read a palette table: one `role = rrggbb` per line, bare six-digit hex
    /// with no leading `#`, which is the shape nightshade's §4 commits to so
    /// that callers can append their own alpha byte.
    ///
    /// Unknown roles are ignored and missing ones keep their default, because
    /// a half-written palette must not be the reason nobody can log in (R13's
    /// rule, applied to the one file that is not the config).
    pub fn parse(text: &str) -> Palette {
        let mut p = Palette::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let Some(c) = hex(v.trim()) else { continue };
            match k.trim() {
                "on_surface" => p.on_surface = c,
                "outline" => p.outline = c,
                "primary" => p.primary = c,
                // Keep the field's own translucency; a theme supplies a hue,
                // not a decision about whether you can see the wallpaper.
                "surface" => p.surface = [c[0], c[1], c[2], p.surface[3]],
                "error" => p.error = c,
                "tertiary" => p.check = c,
                _ => {}
            }
        }
        p
    }

    #[allow(dead_code)]
    pub fn load(path: &std::path::Path) -> Palette {
        std::fs::read_to_string(path)
            .map(|t| Palette::parse(&t))
            .unwrap_or_default()
    }
}

fn hex(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 && s.len() != 8 {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some(rgba(
        b(0)?,
        b(2)?,
        b(4)?,
        if s.len() == 8 { b(6)? } else { 0xff },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_matches_the_live_hyprlock_config() {
        let g = Geometry::default();
        assert_eq!((g.clock_px, g.clock_dy), (110.0, -220.0));
        assert_eq!((g.date_px, g.date_dy), (22.0, -130.0));
        assert_eq!((g.user_px, g.user_dy), (16.0, -40.0));
        assert_eq!((g.field_w, g.field_h, g.field_dy), (340.0, 56.0, 40.0));
        assert_eq!((g.field_rounding, g.rounding_power), (12.0, 4.0));
    }

    #[test]
    fn bare_hex_without_a_hash() {
        let p = Palette::parse("primary = 00bdce\non_surface=d6ecf0\n");
        assert_eq!(p.primary, rgb(0x00, 0xbd, 0xce));
        assert_eq!(p.on_surface, rgb(0xd6, 0xec, 0xf0));
    }

    #[test]
    fn a_theme_supplies_a_hue_not_the_fields_transparency() {
        let p = Palette::parse("surface=142224\n");
        assert_eq!(p.surface[3], Palette::default().surface[3]);
    }

    #[test]
    fn rubbish_lines_leave_the_defaults_alone() {
        let p = Palette::parse("primary = not-a-colour\nnonsense\n# comment\n");
        assert_eq!(p.primary, Palette::default().primary);
    }
}
