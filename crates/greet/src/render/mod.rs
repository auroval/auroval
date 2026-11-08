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

/// One EGL surface, for one output. R10 wants one surface per output at that
/// output's scale, so the thing that is per-output is separated from the thing
/// that is not: the context, the programs and the glyph atlas are shared, and
/// only the surface and its size are not. Building this the other way round —
/// one renderer per output — would duplicate the atlas on every monitor.
pub struct Target {
    /// `None` for an off-screen target.
    surface: Option<egl::Surface>,
    /// Held because dropping it destroys the native window under EGL.
    window: Option<wayland_egl::WlEglSurface>,
    pub width: u32,
    pub height: u32,
    pub scale: i32,
}

impl Target {
    /// A target that is not an output: `present` on it is a no-op, because
    /// the pixels are read back rather than shown.
    fn offscreen(width: u32, height: u32) -> Target {
        Target {
            surface: None,
            window: None,
            width,
            height,
            scale: 1,
        }
    }

    pub fn resize(&mut self, w: u32, h: u32, scale: i32) {
        self.width = w;
        self.height = h;
        self.scale = scale;
        if let Some(win) = self.window.as_ref() {
            win.resize(w as i32, h as i32, 0, 0);
        }
    }
}

/// The GL context and everything that can be shared across outputs.
pub struct Gl {
    egl: Egl,
    display: egl::Display,
    config: egl::Config,
    context: egl::Context,
    pub gl: glow::Context,
    pub atlas: Atlas,

    quad: glow::Buffer,
    image: glow::Program,
    text: glow::Program,
    rect: glow::Program,
    /// The size currently being drawn into: set by `make_current`, so draw
    /// calls need not carry it.
    width: u32,
    height: u32,
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
// fwidth is an extension in GLES2, not core. Without this the shader fails to
// compile on a conformant driver with "requires #version 300 or later", which
// is a confusing way of saying "ask for the extension".
#extension GL_OES_standard_derivatives : enable
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

impl Gl {
    /// Create the shared context. `display_ptr` must be a live `wl_display`.
    ///
    /// # Safety
    /// The pointer must outlive this value.
    pub unsafe fn new(display_ptr: *mut std::ffi::c_void) -> Result<Gl, String> {
        Gl::with_surface_type(display_ptr, egl::WINDOW_BIT)
    }

    /// The body of both constructors. `surface_type` is the one thing they
    /// disagree about, and it is not a detail: EGL matches `EGL_SURFACE_TYPE`
    /// by "config has all of these bits", so asking for `WINDOW_BIT` on a
    /// display that will never have a window excludes every config there is.
    /// Mesa's surfaceless platform offers pbuffer configs and no window
    /// configs at all, so the Wayland path asks for one and the headless path
    /// asks for the other.
    ///
    /// # Safety
    /// As `new`.
    unsafe fn with_surface_type(
        display_ptr: *mut std::ffi::c_void,
        surface_type: egl::Int,
    ) -> Result<Gl, String> {
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
                    surface_type,
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

        let context = egl
            .create_context(
                display,
                config,
                None,
                &[egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE],
            )
            .map_err(|e| e.to_string())?;

        // A context cannot be made current without a surface on GLES2, and
        // the programs have to be built before the first output exists. A
        // 1x1 pbuffer is the usual way out and costs nothing.
        let scratch = egl
            .create_pbuffer_surface(display, config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE])
            .map_err(|e| format!("scratch pbuffer: {e}"))?;
        egl.make_current(display, Some(scratch), Some(scratch), Some(context))
            .map_err(|e| e.to_string())?;

        let gl = glow::Context::from_loader_function(|s| {
            egl.get_proc_address(s)
                .map(|p| p as *const _)
                .unwrap_or(std::ptr::null())
        });

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

        Ok(Gl {
            egl,
            display,
            config,
            context,
            gl,
            atlas,
            quad,
            image,
            text,
            rect,
            width: 1,
            height: 1,
        })
    }

    /// A context with no Wayland behind it, drawing into an off-screen
    /// buffer. This is what makes the view testable: a known `UiState`
    /// renders to a PNG that can be compared, with no compositor, no greetd
    /// and no login screen to log into. It is also the only way to look at
    /// the layout while the shell is being written.
    ///
    /// # Safety
    /// Creates an EGL context; must be called once, from one thread.
    pub unsafe fn headless(width: u32, height: u32) -> Result<(Gl, Target), String> {
        let mut gl = Gl::with_surface_type(egl::DEFAULT_DISPLAY, egl::PBUFFER_BIT)?;
        let fbo = gl.gl.create_framebuffer()?;
        let tex = gl.gl.create_texture()?;
        gl.gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        gl.gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(tex),
            0,
        );
        if gl.gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
            return Err("off-screen framebuffer is incomplete".into());
        }
        gl.width = width;
        gl.height = height;
        gl.gl.viewport(0, 0, width as i32, height as i32);
        Ok((gl, Target::offscreen(width, height)))
    }

    /// Read the current buffer back as RGBA, top row first.
    pub fn read_pixels(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut buf = vec![0u8; w * h * 4];
        unsafe {
            self.gl.read_pixels(
                0,
                0,
                w as i32,
                h as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut buf)),
            );
        }
        // GL's origin is bottom-left; every image format here is top-left.
        let mut out = vec![0u8; buf.len()];
        for y in 0..h {
            let src = (h - 1 - y) * w * 4;
            out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&buf[src..src + w * 4]);
        }
        out
    }

    /// Give one output its own EGL surface.
    ///
    /// # Safety
    /// `surface` must be a live `wl_surface` on the same display.
    pub unsafe fn target(
        &self,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        width: u32,
        height: u32,
        scale: i32,
    ) -> Result<Target, String> {
        use wayland_client::Proxy;
        let window = wayland_egl::WlEglSurface::new(surface.id(), width as i32, height as i32)
            .map_err(|e| format!("wl_egl_window: {e}"))?;
        let es = self
            .egl
            .create_window_surface(
                self.display,
                self.config,
                window.ptr() as egl::NativeWindowType,
                None,
            )
            .map_err(|e| e.to_string())?;
        Ok(Target {
            surface: Some(es),
            window: Some(window),
            width,
            height,
            scale,
        })
    }

    /// Draw into this output from now on.
    pub fn make_current(&mut self, t: &Target) -> Result<(), String> {
        if let Some(es) = t.surface {
            self.egl
                .make_current(self.display, Some(es), Some(es), Some(self.context))
                .map_err(|e| e.to_string())?;
        }
        self.width = t.width;
        self.height = t.height;
        unsafe { self.gl.viewport(0, 0, t.width as i32, t.height as i32) };
        Ok(())
    }

    pub fn present(&self, t: &Target) {
        if let Some(es) = t.surface {
            let _ = self.egl.swap_buffers(self.display, es);
        }
    }

    pub fn size(&self) -> (f32, f32) {
        (self.width as f32, self.height as f32)
    }

    pub fn clear(&self, c: Rgba) {
        unsafe {
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
    /// How wide a string would be drawn, so a caller can decide where it
    /// fits before it draws it.
    pub fn width_of(&self, s: &str, px: f32) -> f32 {
        self.atlas.width(s, px)
    }

    pub fn draw_centred(&mut self, s: &str, px: f32, cx: f32, cy: f32, colour: Rgba) {
        let w = self.atlas.width(s, px);
        let baseline = self.atlas.baseline_from_centre(px);
        self.draw_text(s, px, cx - w * 0.5, cy + baseline, colour);
    }
}
