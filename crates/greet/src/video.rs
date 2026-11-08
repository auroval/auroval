//! R7: a looping video background, decoded in hardware, into this renderer's
//! own OpenGL textures.
//!
//! The hard part of this is already settled and it is worth restating,
//! because the shape of this module follows from it. From
//! `docs/decisions/q5-renderer.md`:
//!
//! > `nvh264dec` advertises `memory:CUDAMemory`, `memory:GLMemory` and plain
//! > `video/x-raw` on its source pad […] with no capsfilter forcing anything,
//! > `nvh264dec ! glimagesink` negotiates `memory:GLMemory` and plays.
//!
//! So NVDEC frames can land directly in an OpenGL texture with **no CUDA
//! interop written here**. GStreamer does it, given a GL context to share.
//! What this module is, therefore, is plumbing: hand GStreamer the greeter's
//! EGL display and context, let the elements negotiate, and take the texture
//! id off each sample.
//!
//! **The trap, also from Q5, and it is the first thing to suspect if this
//! stops working:** forcing the memory type with an explicit
//! `video/x-raw(memory:GLMemory)` capsfilter *fails* with `not-negotiated`,
//! because the GL context has to be established through `GstContext` before
//! the caps can be fixed. Letting the elements negotiate works; helping them
//! does not. Nothing here sets a caps filter on the decoder's output, and
//! that omission is deliberate.
//!
//! R9 governs what happens when any of this is unavailable: the caller falls
//! back to a still and then to a solid colour, and every failure here is a
//! `Problem` carrying the path and a sentence, never a panic and never a
//! silent nothing.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_gl as gst_gl;
use gstreamer_gl::prelude::*;
use gstreamer_gl_egl as gst_gl_egl;

use crate::background::Problem;

/// What the renderer needs in order to draw a frame: a texture that belongs
/// to a context shared with its own, and the size it was decoded at.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub texture: u32,
    pub width: u32,
    pub height: u32,
}

/// What the pipeline negotiated, for a receipt rather than for the renderer.
///
/// R7's fit criterion is about hardware decode, and "it played" is not
/// evidence of that: a software decoder plays too, at 36% of a core instead
/// of 2.5%. The caps are the evidence, and they are recorded so a run can
/// say which path it got rather than which path it hoped for.
#[derive(Clone, Debug, Default)]
pub struct Negotiated {
    /// The caps string of the first sample, memory feature included.
    pub caps: String,
    /// Whether those caps carry `memory:GLMemory`, which is the whole point.
    pub gl_memory: bool,
}

/// A frame, and the mapping that keeps it alive.
///
/// The texture belongs to GStreamer's pool and is recycled. Holding only the
/// id would be a use-after-free with extra steps: the buffer returns to the
/// pool the moment the mapping is dropped, and the next decoded frame
/// overwrites it — so a renderer drawing "the last frame" would be drawing
/// whatever is being decoded right now, torn. Keeping the mapping is what
/// makes the id mean something, and dropping the previous one is what lets
/// the pool keep turning.
struct Held {
    frame: Frame,
    _mapped: gst_gl::GLVideoFrame<gst_gl::gl_video_frame::Readable>,
}

pub struct Video {
    pipeline: gst::Pipeline,
    latest: Arc<Mutex<Option<Held>>>,
    /// Counted where the frames arrive, not where they are looked at.
    ///
    /// The first version of this counted changes in the texture id from the
    /// drawing side, and undercounted badly: the pool holds two textures, so
    /// consecutive frames alternate between two ids and any frame that lands
    /// on the same id as the one before looks like nothing happened. It read
    /// 56.3/s for a clip that was delivering 60. R7 is a claim about dropped
    /// frames, so the count has to come from the sink.
    delivered: Arc<AtomicU64>,
    negotiated: Arc<Mutex<Negotiated>>,
    /// Kept alive for as long as the pipeline is: dropping the wrapped
    /// context while GStreamer still holds frames from it is how you get a
    /// texture id that names nothing.
    _gl_context: gst_gl::GLContext,
    _gl_display: gst_gl::GLDisplay,
}

impl Video {
    /// Start decoding `path` into textures shared with this renderer.
    ///
    /// `egl` is `(display, context)` as `Gl::egl_handles` returns them.
    pub fn start(path: &std::path::Path, egl: (usize, usize)) -> Result<Video, Problem> {
        // R15 and C10, before anything is opened. The still path has refused
        // a home since it was written; this one did not, for a day, because
        // it was built as a transport experiment and inherited none of
        // `decode`'s care. The gap showed itself the first time somebody
        // pointed a real background at it -- theirs was in `~/Pictures`,
        // which is exactly where artwork lives and exactly where the greeter
        // may not look.
        //
        // `refuse_a_home` rather than the lexical predicate, so `..` and a
        // symlink are caught too, and because one place to change is the
        // whole argument for it existing.
        crate::store::refuse_a_home(path, "the video background")
            .map_err(|e| Problem::new(path, e))?;

        gst::init().map_err(|e| Problem::new(path, format!("GStreamer would not start: {e}")))?;

        let (display_handle, context_handle) = egl;
        // SAFETY: the handles come from a live `Gl`, which the caller holds
        // for at least as long as this `Video`.
        let display = unsafe { gst_gl_egl::GLDisplayEGL::with_egl_display(display_handle) }
            .map_err(|e| Problem::new(path, format!("no GL display for GStreamer: {e}")))?;
        let display = display.upcast::<gst_gl::GLDisplay>();

        // GLES2, because that is what `Gl::new` asks EGL for. Telling
        // GStreamer something else here does not change the context; it
        // only makes the two disagree about what they are sharing.
        let wrapped = unsafe {
            gst_gl::GLContext::new_wrapped(
                &display,
                context_handle,
                gst_gl::GLPlatform::EGL,
                gst_gl::GLAPI::GLES2,
            )
        }
        .ok_or_else(|| Problem::new(path, "GStreamer would not wrap the greeter's GL context"))?;

        // `fill_info` is what makes the wrapped context usable rather than
        // merely constructed: it asks the driver what this context actually
        // is. It needs the context current, and the caller's thread is where
        // it is current, which is why this runs here and not on a GStreamer
        // thread.
        wrapped.activate(true).map_err(|e| {
            Problem::new(path, format!("could not activate the shared context: {e}"))
        })?;
        wrapped
            .fill_info()
            .map_err(|e| Problem::new(path, format!("could not read the shared context: {e}")))?;
        wrapped.activate(false).map_err(|e| {
            Problem::new(path, format!("could not release the shared context: {e}"))
        })?;

        let uri = gst::glib::filename_to_uri(path, None)
            .map_err(|e| Problem::new(path, format!("not a usable path: {e}")))?;

        // The sink is built here rather than named in a parse string, and
        // then held. `playbin3` only adds its video-sink to itself on the way
        // to PAUSED, so looking the appsink up by name before that finds
        // nothing — which is exactly what the first version of this did.
        //
        // `glsinkbin` is what inserts whatever upload and convert steps the
        // negotiated decoder needs, and it is the reason this is not a hand
        // written chain: the right chain depends on what the decoder offers,
        // which is the thing Q5 says not to decide in advance. Note what is
        // NOT here — no capsfilter on the decoder's output. Forcing
        // `memory:GLMemory` fails with `not-negotiated`; letting the elements
        // agree is what works.
        let sink = gst_app::AppSink::builder().build();
        // `AUROVAL_VIDEO_UPLOAD_ONLY` swaps `glsinkbin` for a bare
        // `glupload`, which is the experiment Q5 left open: glsinkbin inserts
        // `glcolorconvert` to RGBA so one texture can be sampled, and Q5
        // guessed that sampling NV12 directly with a two-texture shader would
        // be cheaper. This is how that guess gets a number instead of a
        // sentence.
        // AUROVAL_VIDEO_NO_GL is the control: no GL at all, which is the
        // pipeline P0 measured at 2.5%% of a core. It draws nothing, so it is
        // useless as a background -- it exists so the GL cost can be
        // attributed rather than guessed at.
        // AUROVAL_VIDEO_DIRECT: the appsink IS the video sink, with no
        // glsinkbin and no glupload between it and the decoder. `nvh264dec`
        // advertises `video/x-raw(memory:GLMemory)` on its own source pad, so
        // it can do the interop itself; this asks whether its own path is
        // cheaper than glupload's.
        //
        // The caps are set here, which looks like the thing Q5 warns against.
        // It is not quite: Q5's trap is forcing GLMemory *before* the GL
        // context is established, and the bus handler below answers the
        // context query for every element including the decoder. Whether that
        // is enough is exactly what this measures.
        // AUROVAL_VIDEO_SYSMEM: plain system memory, no GL feature at all.
        // The discriminator between two explanations of the 32%: is it the
        // GL interop, or is it leaving CUDA memory by any route?
        let glsink: gst::Element = if std::env::var_os("AUROVAL_VIDEO_SYSMEM").is_some() {
            sink.set_caps(Some(&gst::Caps::builder("video/x-raw").build()));
            sink.clone().upcast()
        } else if std::env::var_os("AUROVAL_VIDEO_DIRECT").is_some() {
            sink.set_caps(Some(
                &gst::Caps::builder("video/x-raw")
                    .features(["memory:GLMemory"])
                    .build(),
            ));
            sink.clone().upcast()
        } else if std::env::var_os("AUROVAL_VIDEO_NO_GL").is_some() {
            gst::ElementFactory::make("fakesink")
                .property("sync", true)
                .build()
                .map_err(|e| Problem::new(path, format!("no fakesink: {e}")))?
        } else if std::env::var_os("AUROVAL_VIDEO_UPLOAD_ONLY").is_some() {
            let bin = gst::Bin::new();
            let upload = gst::ElementFactory::make("glupload")
                .build()
                .map_err(|e| Problem::new(path, format!("no glupload: {e}")))?;
            bin.add_many([&upload, sink.upcast_ref::<gst::Element>()])
                .map_err(|e| Problem::new(path, format!("could not build the bin: {e}")))?;
            upload
                .link(&sink)
                .map_err(|e| Problem::new(path, format!("glupload would not link: {e}")))?;
            let pad = upload
                .static_pad("sink")
                .ok_or_else(|| Problem::new(path, "glupload has no sink pad"))?;
            let ghost = gst::GhostPad::with_target(&pad)
                .map_err(|e| Problem::new(path, format!("no ghost pad: {e}")))?;
            bin.add_pad(&ghost)
                .map_err(|e| Problem::new(path, format!("could not add the ghost pad: {e}")))?;
            bin.upcast()
        } else {
            gst::ElementFactory::make("glsinkbin")
                .property("sink", &sink)
                .build()
                .map_err(|e| Problem::new(path, format!("no glsinkbin: {e}")))?
        };
        let fakesink = gst::ElementFactory::make("fakesink")
            .build()
            .map_err(|e| Problem::new(path, format!("no fakesink: {e}")))?;
        let pipeline = gst::ElementFactory::make("playbin3")
            .property("uri", &uri)
            .property("video-sink", &glsink)
            .property("audio-sink", &fakesink)
            .build()
            .map_err(|e| Problem::new(path, format!("no playbin3: {e}")))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| Problem::new(path, "playbin3 was not a pipeline"))?;

        // One frame deep, newest wins. A login screen that falls behind
        // should drop frames rather than queue them: the thing on screen is
        // supposed to be what is happening now, and R7 is about holding the
        // refresh rate, not about delivering every frame ever decoded.
        sink.set_max_buffers(1);
        sink.set_drop(true);
        sink.set_sync(true);

        let latest: Arc<Mutex<Option<Held>>> = Arc::new(Mutex::new(None));
        let negotiated: Arc<Mutex<Negotiated>> = Arc::new(Mutex::new(Negotiated::default()));
        let delivered = Arc::new(AtomicU64::new(0));

        let l = latest.clone();
        let n = negotiated.clone();
        let d = delivered.clone();
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = match sink.pull_sample() {
                        Ok(s) => s,
                        Err(_) => return Err(gst::FlowError::Eos),
                    };
                    d.fetch_add(1, Ordering::Relaxed);
                    if std::env::var_os("AUROVAL_VIDEO_NO_MAP").is_some() {
                        return Ok(gst::FlowSuccess::Ok);
                    }
                    if let Some(held) = read_gl_frame(&sample, &n) {
                        // The previous mapping drops here, inside the
                        // assignment, which is what returns its buffer.
                        *l.lock().unwrap_or_else(|e| e.into_inner()) = Some(held);
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        // The GL context reaches the elements through the bus: they ask for
        // it by name when they need it, and this answers. Setting it on the
        // pipeline up front does not work for elements created later by
        // `playbin3`, which is all of them.
        let bus_display = display.clone();
        let bus_context = wrapped.clone();
        let bus = pipeline
            .bus()
            .ok_or_else(|| Problem::new(path, "the pipeline has no bus"))?;
        bus.set_sync_handler(move |_, msg| {
            if let gst::MessageView::NeedContext(ctx) = msg.view() {
                let kind = ctx.context_type();
                if let Some(el) = msg.src().and_then(|s| s.downcast_ref::<gst::Element>()) {
                    if kind == "gst.gl.GLDisplay" {
                        let mut c = gst::Context::new(kind, true);
                        c.get_mut()
                            .unwrap()
                            .structure_mut()
                            .set("gst.gl.GLDisplay", &bus_display);
                        el.set_context(&c);
                    } else if kind == "gst.gl.app_context" {
                        let mut c = gst::Context::new(kind, true);
                        c.get_mut()
                            .unwrap()
                            .structure_mut()
                            .set("context", &bus_context);
                        el.set_context(&c);
                    }
                }
            }
            gst::BusSyncReply::Pass
        });

        // R8: loop without a visible seam.
        //
        // `about-to-finish` is playbin3's gapless hand-off: it fires while
        // the current stream is still playing and asks for the next URI, so
        // the new one is prepared and the two are joined without the
        // pipeline ever draining. A seek on EOS would also loop, and would
        // show the seam R8 forbids -- EOS means the last frame is already
        // gone, and the flush that follows empties the sink.
        //
        // Setting the same URI is what makes it a loop rather than a
        // playlist.
        let loop_uri = uri.clone();
        pipeline.connect("about-to-finish", false, move |args| {
            if let Ok(pb) = args[0].get::<gst::Element>() {
                pb.set_property("uri", &loop_uri);
            }
            None
        });

        if let Err(e) = pipeline.set_state(gst::State::Playing) {
            // `e` is `gst::StateChangeError`, which says only that a state
            // change failed -- not which element, and not why. The reason is
            // on the bus, and without it the message for an unreadable file
            // is "Element failed to change its state", which sent somebody
            // looking in the wrong place for twenty minutes. So the bus gets
            // a moment to produce the real cause before this gives up.
            let reason = bus
                .timed_pop_filtered(
                    gst::ClockTime::from_mseconds(500),
                    &[gst::MessageType::Error],
                )
                .and_then(|msg| match msg.view() {
                    // `error()` and not `debug()`: the first is a sentence
                    // for a person -- "Could not open file ... for reading."
                    // -- and the second is a C file and line number inside
                    // GStreamer, which belongs in a bug report about
                    // GStreamer and not in a greeter's log.
                    gst::MessageView::Error(err) => Some(err.error().to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| format!("{e}"));
            let _ = pipeline.set_state(gst::State::Null);
            return Err(Problem::new(
                path,
                format!("the pipeline would not play: {reason}"),
            ));
        }

        Ok(Video {
            pipeline,
            latest,
            delivered,
            negotiated,
            _gl_context: wrapped,
            _gl_display: display,
        })
    }

    /// The newest frame decoded so far, or `None` before the first one.
    pub fn frame(&self) -> Option<Frame> {
        self.latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|h| h.frame)
    }

    /// How many frames the sink has taken since the pipeline started.
    pub fn delivered(&self) -> u64 {
        self.delivered.load(Ordering::Relaxed)
    }

    /// What the pipeline negotiated. For receipts, not for drawing.
    pub fn negotiated(&self) -> Negotiated {
        self.negotiated
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Anything the pipeline has complained about since the last call.
    ///
    /// Errors are drained rather than raised: R9 says the greeter falls back
    /// and logs, and a decoder that dies half way through a login is a
    /// fallback, not a failure to log in.
    pub fn problems(&self) -> Vec<String> {
        let Some(bus) = self.pipeline.bus() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Some(msg) = bus.pop() {
            match msg.view() {
                gst::MessageView::Error(e) => out.push(format!(
                    "{}: {}",
                    e.src().map(|s| s.path_string()).unwrap_or_default(),
                    e.error()
                )),
                gst::MessageView::Warning(w) => out.push(format!(
                    "{}: {}",
                    w.src().map(|s| s.path_string()).unwrap_or_default(),
                    w.error()
                )),
                _ => {}
            }
        }
        out
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

/// Take the texture id off a sample, if it carries one.
///
/// A sample whose memory is not GL memory is not an error here — it is the
/// software path, which plays and costs 36% of a core instead of 2.5%. The
/// caps are recorded either way so the caller can say which it got.
fn read_gl_frame(sample: &gst::Sample, negotiated: &Mutex<Negotiated>) -> Option<Held> {
    let caps = sample.caps()?;
    let info = gstreamer_video::VideoInfo::from_caps(caps).ok()?;

    {
        let mut n = negotiated.lock().unwrap_or_else(|e| e.into_inner());
        if n.caps.is_empty() {
            n.caps = caps.to_string();
            n.gl_memory = caps.to_string().contains("memory:GLMemory");
        }
    }

    let buffer = sample.buffer()?;
    let frame = gstreamer_gl::GLVideoFrame::from_buffer_readable(buffer.copy(), &info).ok()?;
    let texture = frame.texture_id(0).ok()?;
    Some(Held {
        frame: Frame {
            texture,
            width: info.width(),
            height: info.height(),
        },
        _mapped: frame,
    })
}
