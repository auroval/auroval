//! The GL renderer, chosen by Q5 and measured before it was chosen.
//!
//! Three programs and nothing else: a textured quad for the background, the
//! same quad in alpha for glyphs, and a signed-distance rounded rectangle for
//! the input field. Everything is placed in pixels and converted to clip
//! space here, so the layout code never thinks in GL coordinates.
//!
//! The rounded rectangle is a **superellipse**, not a circular arc, because
//! hyprland's `rounding_power = 4` is part of R11: *"the corner curve bends
//! toward a squircle rather than a circular arc; it reads as softer at the
//! same nominal radius."* A circular-arc rounded rect would be the obvious
//! implementation and would quietly fail the requirement.

pub mod atlas;

use glow::HasContext;
use khronos_egl as egl;

use crate::theme::Rgba;
use atlas::Atlas;

type Egl = egl::DynamicInstance<egl::EGL1_4>;

/// Everything the input field needs, as one value. The shape is R11's, and
/// passing it whole keeps the signature readable — nine positional floats is
/// how a radius ends up in the outline's place.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    pub power: f32,
    pub outline: f32,
    pub fill: Rgba,
    pub stroke: Rgba,
}

pub struct Renderer {
    egl: Egl,
    display: egl::Display,
    surface: egl::Surface,
    _context: egl::Context,
    /// Held because dropping it destroys the native window under EGL.
    _window: wayland_egl::WlEglSurface,
    pub gl: glow::Context,
    pub atlas: Atlas,

    quad: glow::Buffer,
    image: glow::Program,
    text: glow::Program,
    rect: glow::Program,
    pub width: u32,
    pub height: u32,
}

const VERT: &str = r#"#version 100
attribute vec2 corner;          // 0..1 over the quad
uniform vec4 box;               // x, y, w, h in pixels, top-left origin
uniform vec2 res;
varying vec2 uv;
varying vec2 local;
void main() {
    local = corner;
    uv = corner;
    vec2 px = box.xy + corner * box.zw;
    vec2 ndc = vec2(px.x / res.x * 2.0 - 1.0, 1.0 - px.y / res.y * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

const FRAG_IMAGE: &str = r#"#version 100
precision mediump float;
uniform sampler2D tex;
uniform vec4 tint;
varying vec2 uv;
void main() { gl_FragColor = texture2D(tex, uv) * tint; }
"#;

const FRAG_TEXT: &str = r#"#version 100
precision mediump float;
uniform sampler2D tex;
uniform vec4 colour;
uniform vec4 uvrect;            // u0, v0, u1, v1
varying vec2 uv;
void main() {
    vec2 t = mix(uvrect.xy, uvrect.zw, uv);
    gl_FragColor = vec4(colour.rgb, colour.a * texture2D(tex, t).a);
}
"#;

// Superellipse distance: the n-norm of the corner offset instead of the
// 2-norm a circular rounded rect would use. n = rounding_power.
const FRAG_RECT: &str = r#"#version 100
precision mediump float;
uniform vec2 half_size;         // in pixels
uniform float radius;
uniform float power;
uniform float outline;
uniform vec4 fill;
uniform vec4 stroke;
varying vec2 local;

float sdf(vec2 p) {
    vec2 q = abs(p) - (half_size - radius);
    vec2 m = max(q, 0.0);
    float corner = pow(pow(m.x, power) + pow(m.y, power), 1.0 / power);
    return corner + min(max(q.x, q.y), 0.0) - radius;
}

void main() {
    vec2 p = (local - 0.5) * 2.0 * half_size;
    float d = sdf(p);
    float aa = fwidth(d);
    float inside = 1.0 - smoothstep(-aa, aa, d);
    float inner  = 1.0 - smoothstep(-aa, aa, d + outline);
    vec4 c = mix(stroke, fill, inner);
    c.a *= inside;
    gl_FragColor = vec4(c.rgb, c.a);
}
"#;

unsafe fn program(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    let p = gl.create_program()?;
    for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
        let s = gl.create_shader(kind)?;
        gl.shader_source(s, src);
        gl.compile_shader(s);
        if !gl.get_shader_compile_status(s) {
            return Err(format!("shader: {}", gl.get_shader_info_log(s)));
        }
        gl.attach_shader(p, s);
        gl.delete_shader(s);
    }
    gl.link_program(p);
    if !gl.get_program_link_status(p) {
        return Err(format!("link: {}", gl.get_program_info_log(p)));
    }
    Ok(p)
}

impl Renderer {
    /// # Safety
    /// `display_ptr` must be a live `wl_display`, and `surface` a live
    /// `wl_surface` on it.
    pub unsafe fn new(
        display_ptr: *mut std::ffi::c_void,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        width: u32,
        height: u32,
    ) -> Result<Renderer, String> {
        use wayland_client::Proxy;

        let egl = Egl::load_required().map_err(|e| format!("libEGL: {e}"))?;
        let display = egl
            .get_display(display_ptr)
            .ok_or("no EGL display for this wl_display")?;
        egl.initialize(display).map_err(|e| e.to_string())?;
        egl.bind_api(egl::OPENGL_ES_API)
            .map_err(|e| e.to_string())?;

        let config = egl
            .choose_first_config(
                display,
                &[
                    egl::SURFACE_TYPE,
                    egl::WINDOW_BIT,
                    egl::RENDERABLE_TYPE,
                    egl::OPENGL_ES2_BIT,
                    egl::RED_SIZE,
                    8,
                    egl::GREEN_SIZE,
                    8,
                    egl::BLUE_SIZE,
                    8,
                    egl::ALPHA_SIZE,
                    8,
                    egl::NONE,
                ],
            )
            .map_err(|e| e.to_string())?
            .ok_or("no suitable EGL config")?;

        let window = wayland_egl::WlEglSurface::new(surface.id(), width as i32, height as i32)
            .map_err(|e| format!("wl_egl_window: {e}"))?;
        let es = egl
            .create_window_surface(display, config, window.ptr() as egl::NativeWindowType, None)
            .map_err(|e| e.to_string())?;
        let context = egl
            .create_context(
                display,
                config,
                None,
                &[egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE],
            )
            .map_err(|e| e.to_string())?;
        egl.make_current(display, Some(es), Some(es), Some(context))
            .map_err(|e| e.to_string())?;

        let gl = glow::Context::from_loader_function(|s| {
            egl.get_proc_address(s)
                .map(|p| p as *const _)
                .unwrap_or(std::ptr::null())
        });
        // The superellipse needs fwidth, which GLES2 only has through this.
        let _ = gl.get_parameter_string(glow::EXTENSIONS);

        let quad = gl.create_buffer()?;
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad));
        let verts: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            std::slice::from_raw_parts(verts.as_ptr() as *const u8, 32),
            glow::STATIC_DRAW,
        );

        let image = program(&gl, VERT, FRAG_IMAGE)?;
        let text = program(&gl, VERT, FRAG_TEXT)?;
        let rect = program(&gl, VERT, FRAG_RECT)?;
        let atlas = Atlas::new(&gl, atlas::FONT_CANDIDATES)?;

        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

        Ok(Renderer {
            egl,
            display,
            surface: es,
            _context: context,
            _window: window,
            gl,
            atlas,
            quad,
            image,
            text,
            rect,
            width,
            height,
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
        self._window.resize(w as i32, h as i32, 0, 0);
        unsafe { self.gl.viewport(0, 0, w as i32, h as i32) };
    }

    pub fn clear(&self, c: Rgba) {
        unsafe {
            self.gl
                .viewport(0, 0, self.width as i32, self.height as i32);
            self.gl.clear_color(c[0], c[1], c[2], c[3]);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    unsafe fn bind_quad(&self, prog: glow::Program) {
        self.gl.use_program(Some(prog));
        self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.quad));
        self.gl.enable_vertex_attrib_array(0);
        self.gl
            .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);
        self.uniform2(prog, "res", self.width as f32, self.height as f32);
    }

    unsafe fn uniform2(&self, p: glow::Program, name: &str, a: f32, b: f32) {
        self.gl
            .uniform_2_f32(self.gl.get_uniform_location(p, name).as_ref(), a, b);
    }
    unsafe fn uniform4(&self, p: glow::Program, name: &str, v: [f32; 4]) {
        self.gl.uniform_4_f32(
            self.gl.get_uniform_location(p, name).as_ref(),
            v[0],
            v[1],
            v[2],
            v[3],
        );
    }
    unsafe fn uniform1(&self, p: glow::Program, name: &str, v: f32) {
        self.gl
            .uniform_1_f32(self.gl.get_uniform_location(p, name).as_ref(), v);
    }

    /// Draw a texture over a pixel rectangle.
    pub fn draw_image(&self, tex: glow::Texture, x: f32, y: f32, w: f32, h: f32, tint: Rgba) {
        unsafe {
            self.bind_quad(self.image);
            self.uniform4(self.image, "box", [x, y, w, h]);
            self.uniform4(self.image, "tint", tint);
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            self.gl
                .uniform_1_i32(self.gl.get_uniform_location(self.image, "tex").as_ref(), 0);
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }

    /// The input field: a superellipse with an outline, per R11.
    pub fn draw_field(&self, f: &Field) {
        let Field {
            x,
            y,
            w,
            h,
            radius,
            power,
            outline,
            fill,
            stroke,
        } = *f;
        unsafe {
            self.bind_quad(self.rect);
            self.uniform4(self.rect, "box", [x, y, w, h]);
            self.uniform2(self.rect, "half_size", w * 0.5, h * 0.5);
            self.uniform1(self.rect, "radius", radius);
            self.uniform1(self.rect, "power", power);
            self.uniform1(self.rect, "outline", outline);
            self.uniform4(self.rect, "fill", fill);
            self.uniform4(self.rect, "stroke", stroke);
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }

    /// Draw a line of text with its left edge at `x` and its baseline at `y`.
    pub fn draw_text(&mut self, s: &str, px: f32, x: f32, baseline: f32, colour: Rgba) {
        let tex = self.atlas.texture();
        let mut pen = x;
        for ch in s.chars() {
            let Some(g) = self.atlas.glyph(&self.gl, ch, px) else {
                continue;
            };
            if g.w > 0.0 && g.h > 0.0 {
                unsafe {
                    self.bind_quad(self.text);
                    self.uniform4(self.text, "box", [pen + g.left, baseline + g.top, g.w, g.h]);
                    self.uniform4(self.text, "colour", colour);
                    self.uniform4(self.text, "uvrect", g.uv);
                    self.gl.active_texture(glow::TEXTURE0);
                    self.gl.bind_texture(glow::TEXTURE_2D, Some(tex));
                    self.gl
                        .uniform_1_i32(self.gl.get_uniform_location(self.text, "tex").as_ref(), 0);
                    self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                }
            }
            pen += g.advance;
        }
    }

    /// Centre a line horizontally on `cx`, placing its visual middle on `cy`.
    pub fn draw_centred(&mut self, s: &str, px: f32, cx: f32, cy: f32, colour: Rgba) {
        let w = self.atlas.width(s, px);
        let ascent = self.atlas.ascent(px);
        self.draw_text(s, px, cx - w * 0.5, cy + ascent * 0.5, colour);
    }

    pub fn present(&self) {
        let _ = self.egl.swap_buffers(self.display, self.surface);
    }
}
