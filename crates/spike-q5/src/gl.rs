//! The GL path: GStreamer's own GL context, shared with ours, no readback.
//!
//! The shape of it, and why each step is needed:
//!
//!   1. auroval creates the EGL display, config, surface and context itself,
//!      because the Wayland surface is auroval's.
//!   2. It wraps that EGL display and context as a `GstGLDisplayEGL` and a
//!      wrapped `GstGLContext`, and answers the pipeline's `need-context`
//!      messages with them. GStreamer then creates its own context *sharing*
//!      with ours, and `glupload` can hand over textures we can sample.
//!   3. The caps are left to negotiate. Forcing
//!      `video/x-raw(memory:GLMemory)` with a capsfilter fails with
//!      `not-negotiated`, because the context has to be established through
//!      `GstContext` before the caps can be fixed --- letting the elements
//!      negotiate works, helping them does not. That trap is recorded in the
//!      Q5 brief and it is real.
//!   4. Each frame carries a `GstGLSyncMeta`; waiting on it in our context is
//!      what makes the texture safe to sample without a full pipeline stall.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use glow::HasContext;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;
use gstreamer_gl::prelude::*;
use gstreamer_gl::{GLContext, GLSyncMeta, GLVideoFrameExt};
use khronos_egl as egl;
use wayland_client::protocol::wl_surface;
use wayland_client::{Connection, Proxy};

use crate::text::{label, Label};
use crate::win::{self, Renderer, Surface};
use crate::Report;

type Egl = egl::DynamicInstance<egl::EGL1_4>;

pub struct GlRenderer {
    clip: String,
    decoded: Arc<AtomicU64>,
    label: Label,

    egl: Option<Egl>,
    display: Option<egl::Display>,
    surface: Option<egl::Surface>,
    context: Option<egl::Context>,
    _window: Option<wayland_egl::WlEglSurface>,
    gl: Option<glow::Context>,

    gst_context: Option<GLContext>,
    pipeline: Option<gstreamer::Pipeline>,
    sink: Option<AppSink>,

    video_prog: Option<glow::Program>,
    text_prog: Option<glow::Program>,
    text_tex: Option<glow::Texture>,
    vao: Option<glow::VertexArray>,
}

const VERT: &str = r#"#version 100
attribute vec2 pos;
varying vec2 uv;
void main() {
    uv = vec2(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    gl_Position = vec4(pos, 0.0, 1.0);
}
"#;

const FRAG_VIDEO: &str = r#"#version 100
precision mediump float;
uniform sampler2D tex;
varying vec2 uv;
void main() { gl_FragColor = texture2D(tex, uv); }
"#;

// The label is coverage only, so the shader supplies the colour. Drawn as a
// separate quad placed by a uniform rather than baked into the video texture,
// because that is how a greeter would do it.
const VERT_TEXT: &str = r#"#version 100
attribute vec2 pos;
uniform vec4 rect;      // x, y, w, h in clip space
varying vec2 uv;
void main() {
    uv = vec2(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
    vec2 p = rect.xy + (pos * 0.5 + 0.5) * rect.zw;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;

const FRAG_TEXT: &str = r#"#version 100
precision mediump float;
uniform sampler2D tex;
varying vec2 uv;
void main() { gl_FragColor = vec4(1.0, 1.0, 1.0, texture2D(tex, uv).a); }
"#;

unsafe fn program(gl: &glow::Context, vs: &str, fs: &str) -> glow::Program {
    let p = gl.create_program().expect("program");
    for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
        let s = gl.create_shader(kind).expect("shader");
        gl.shader_source(s, src);
        gl.compile_shader(s);
        if !gl.get_shader_compile_status(s) {
            panic!("shader: {}", gl.get_shader_info_log(s));
        }
        gl.attach_shader(p, s);
        gl.delete_shader(s);
    }
    gl.link_program(p);
    if !gl.get_program_link_status(p) {
        panic!("link: {}", gl.get_program_info_log(p));
    }
    p
}

impl Renderer for GlRenderer {
    fn mode(&self) -> &'static str {
        "gl   (memory:GLMemory, no readback)"
    }
    fn decoded(&self) -> u64 {
        self.decoded.load(Ordering::Relaxed)
    }

    fn configure(&mut self, conn: &Connection, surface: &wl_surface::WlSurface, w: u32, h: u32) {
        unsafe { self.setup(conn, surface, w, h) }
    }

    fn present(&mut self, s: &mut Surface<'_>) -> bool {
        let sink = match self.sink.as_ref() {
            Some(s) => s,
            None => return false,
        };
        let sample = match sink.try_pull_sample(gstreamer::ClockTime::ZERO) {
            Some(x) => x,
            None => {
                s.surface.frame(s.qh, s.surface.clone());
                s.surface.commit();
                return false;
            }
        };
        let n = self.decoded.fetch_add(1, Ordering::Relaxed);
        if n == 0 {
            // Say out loud what was negotiated. A GL path that quietly fell
            // back to system memory would still run, and the measurement
            // would then be of something else entirely.
            if let Some(caps) = sample.caps() {
                println!("    negotiated      {caps}");
            }
            if let Some(p) = self.pipeline.as_ref() {
                let mut names: Vec<String> = Vec::new();
                let mut it = p.iterate_recurse();
                while let Ok(Some(e)) = it.next() {
                    names.push(e.name().to_string());
                }
                println!("    elements        {}", names.join(" "));
            }
        }

        let info = sample
            .caps()
            .and_then(|c| gstreamer_video::VideoInfo::from_caps(c).ok());
        let (buffer, info) = match (sample.buffer_owned(), info) {
            (Some(b), Some(i)) => (b, i),
            _ => return false,
        };

        let ctx = self.gst_context.as_ref().expect("gst context");
        if let Some(meta) = buffer.meta::<GLSyncMeta>() {
            meta.wait(ctx);
        }
        let frame = match gstreamer_gl::GLVideoFrame::from_buffer_readable(buffer, &info) {
            Ok(f) => f,
            Err(_) => return false,
        };
        let tex = match frame.texture_id(0) {
            Ok(id) => id,
            Err(_) => return false,
        };

        unsafe { self.draw(tex, s.width, s.height) };
        s.surface.frame(s.qh, s.surface.clone());
        // eglSwapBuffers attaches and commits the surface itself.
        let (egl, display, es) = (
            self.egl.as_ref().unwrap(),
            self.display.unwrap(),
            self.surface.unwrap(),
        );
        egl.swap_buffers(display, es).expect("swap");
        true
    }
}

impl GlRenderer {
    unsafe fn setup(&mut self, conn: &Connection, surface: &wl_surface::WlSurface, w: u32, h: u32) {
        let egl = Egl::load_required().expect("libEGL");
        let display = egl
            .get_display(win::display_ptr(conn))
            .expect("egl display from wl_display");
        egl.initialize(display).expect("egl init");
        egl.bind_api(egl::OPENGL_ES_API).expect("bind gles");

        let attrs = [
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
        ];
        let config = egl
            .choose_first_config(display, &attrs)
            .expect("choose config")
            .expect("a config exists");

        let wl_window = wayland_egl::WlEglSurface::new(surface.id(), w as i32, h as i32)
            .expect("wl_egl_window");
        let es = egl
            .create_window_surface(
                display,
                config,
                wl_window.ptr() as egl::NativeWindowType,
                None,
            )
            .expect("egl window surface");
        let ctx_attrs = [egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE];
        let context = egl
            .create_context(display, config, None, &ctx_attrs)
            .expect("egl context");
        egl.make_current(display, Some(es), Some(es), Some(context))
            .expect("make current");
        // Present as fast as the compositor allows; the frame callback is what
        // paces this, not the swap interval.
        let _ = egl.swap_interval(display, 0);

        let gl = glow::Context::from_loader_function(|s| {
            egl.get_proc_address(s)
                .map(|p| p as *const _)
                .unwrap_or(std::ptr::null())
        });

        let video_prog = program(&gl, VERT, FRAG_VIDEO);
        let text_prog = program(&gl, VERT_TEXT, FRAG_TEXT);

        // One full-screen triangle strip, reused by both programs.
        let vao = gl.create_vertex_array().ok();
        if let Some(v) = vao {
            gl.bind_vertex_array(Some(v));
        }
        let vbo = gl.create_buffer().expect("vbo");
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let quad: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            std::slice::from_raw_parts(quad.as_ptr() as *const u8, 32),
            glow::STATIC_DRAW,
        );
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 0, 0);

        // The label goes up once, as an alpha texture.
        let text_tex = gl.create_texture().expect("texture");
        gl.bind_texture(glow::TEXTURE_2D, Some(text_tex));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::ALPHA as i32,
            self.label.w as i32,
            self.label.h as i32,
            0,
            glow::ALPHA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&self.label.alpha)),
        );

        // --- GStreamer, sharing this context -------------------------------
        let gst_display = {
            let raw = gstreamer_gl_egl::ffi::gst_gl_display_egl_new_with_egl_display(
                display.as_ptr() as *mut _,
            );
            let d: gstreamer_gl::GLDisplay =
                glib::translate::from_glib_full(raw as *mut gstreamer_gl::ffi::GstGLDisplay);
            d
        };
        let gst_context = GLContext::new_wrapped(
            &gst_display,
            context.as_ptr() as usize,
            gstreamer_gl::GLPlatform::EGL,
            gstreamer_gl::GLAPI::GLES2,
        )
        .expect("wrapped gl context");
        gst_context.activate(true).expect("activate");
        gst_context.fill_info().expect("fill_info");

        let desc = format!(
            "filesrc location={} ! decodebin3 ! glupload ! glcolorconvert ! \
             appsink name=sink max-buffers=2 drop=true sync=true \
             caps=video/x-raw(memory:GLMemory),format=RGBA",
            self.clip
        );
        let pipeline = gstreamer::parse::launch(&desc)
            .expect("gl pipeline parses")
            .downcast::<gstreamer::Pipeline>()
            .expect("is a pipeline");
        let sink = pipeline
            .by_name("sink")
            .expect("sink exists")
            .downcast::<AppSink>()
            .expect("is an appsink");

        // Answer need-context synchronously, before the element gives up.
        let d = gst_display.clone();
        let c = gst_context.clone();
        pipeline
            .bus()
            .expect("bus")
            .set_sync_handler(move |_, msg| {
                if let gstreamer::MessageView::NeedContext(nc) = msg.view() {
                    let t = nc.context_type();
                    if let Some(el) = msg
                        .src()
                        .and_then(|s| s.clone().downcast::<gstreamer::Element>().ok())
                    {
                        if t == "gst.gl.GLDisplay" {
                            let ctx = gstreamer::Context::new(t, true);
                            ctx.set_gl_display(&d);
                            el.set_context(&ctx);
                        } else if t == "gst.gl.app_context" {
                            let mut ctx = gstreamer::Context::new(t, true);
                            {
                                let ctx = ctx.get_mut().expect("fresh context is writable");
                                ctx.structure_mut().set("context", &c);
                            }
                            el.set_context(&ctx);
                        }
                    }
                }
                gstreamer::BusSyncReply::Pass
            });
        pipeline
            .set_state(gstreamer::State::Playing)
            .expect("plays");

        self.egl = Some(egl);
        self.display = Some(display);
        self.surface = Some(es);
        self.context = Some(context);
        self._window = Some(wl_window);
        self.gl = Some(gl);
        self.video_prog = Some(video_prog);
        self.text_prog = Some(text_prog);
        self.text_tex = Some(text_tex);
        self.vao = vao;
        self.gst_context = Some(gst_context);
        self.pipeline = Some(pipeline);
        self.sink = Some(sink);
    }

    unsafe fn draw(&self, tex: u32, w: u32, h: u32) {
        let gl = self.gl.as_ref().unwrap();
        gl.viewport(0, 0, w as i32, h as i32);
        gl.disable(glow::BLEND);
        gl.use_program(self.video_prog);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(
            glow::TEXTURE_2D,
            Some(glow::NativeTexture(
                std::num::NonZeroU32::new(tex).expect("texture id"),
            )),
        );
        gl.uniform_1_i32(
            gl.get_uniform_location(self.video_prog.unwrap(), "tex")
                .as_ref(),
            0,
        );
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

        if std::env::var_os("AUROVAL_NO_TEXT").is_some() {
            return;
        }
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        gl.use_program(self.text_prog);
        gl.bind_texture(glow::TEXTURE_2D, self.text_tex);
        let p = self.text_prog.unwrap();
        gl.uniform_1_i32(gl.get_uniform_location(p, "tex").as_ref(), 0);
        // Bottom-left, 64 px in, in clip space.
        let (lw, lh) = (self.label.w as f32, self.label.h as f32);
        let rect = [
            -1.0 + 128.0 / w as f32,
            -1.0 + 128.0 / h as f32,
            2.0 * lw / w as f32,
            2.0 * lh / h as f32,
        ];
        gl.uniform_4_f32(
            gl.get_uniform_location(p, "rect").as_ref(),
            rect[0],
            rect[1],
            rect[2],
            rect[3],
        );
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
    }
}

pub fn run(clip: &str, seconds: f64) -> Result<Report, Box<dyn std::error::Error>> {
    let (w, h) = crate::probe_size(clip)?;
    let clip = clip.to_string();
    win::run_with(
        move || {
            Box::new(GlRenderer {
                clip: clip.clone(),
                decoded: Arc::new(AtomicU64::new(0)),
                label: label("auroval — 00:00  Saturday, 7 November", 44.0),
                egl: None,
                display: None,
                surface: None,
                context: None,
                _window: None,
                gl: None,
                gst_context: None,
                pipeline: None,
                sink: None,
                video_prog: None,
                text_prog: None,
                text_tex: None,
                vao: None,
            })
        },
        w,
        h,
        seconds,
    )
}
