//! Phase 2 of the spike: a Wayland client that plays the video *and* logs in.
//!
//! Phase 1 measures decode in isolation, which is the load-bearing question.
//! This mode exists to prove the whole path end to end in the real context:
//! a Wayland client under `cage`, driving a real greetd auth loop, with
//! hardware-decoded video on screen.
//!
//! Two honest limits, because a spike that overstates itself is worse than no
//! spike. First, frames reach the screen via `wl_shm`, which means a GPU-to-CPU
//! readback and a copy per frame — the opposite of what R7 eventually needs.
//! That is deliberate: Q5 (smithay + wgpu, or a toolkit) is unanswered, and
//! picking a renderer here would prejudge it. The decode measurement is
//! separately instrumented so the readback cost cannot contaminate it. Second,
//! the text rendering is a bitmap blit sufficient to read a prompt, not R11's
//! typography.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use greetd_proto::{AuthMessageType, Connection, ErrorType, Request, Response};

/// What the auth thread wants the UI to show. The UI never decides this: C2
/// forbids assuming anything about the conversation, so the UI renders whatever
/// arrives and nothing more.
#[derive(Debug, Clone)]
pub enum Ui {
    /// A prompt needing an answer. `secret` decides whether to echo.
    Prompt { text: String, secret: bool },
    /// An `info` or `error` message. Per R17 this is content, not a status line.
    Message { text: String, is_error: bool },
    /// Authentication succeeded and the session is starting.
    Starting,
    /// Fatal: nothing more will happen.
    Dead(String),
}

/// What the UI sends back.
pub enum Answer {
    Text(String),
    Cancel,
}

/// Drive the greetd conversation on its own thread.
///
/// R18 forbids `greet` imposing a deadline, so this thread blocks indefinitely
/// on the socket. That is precisely why it is not the UI thread: a device-code
/// flow (C13) can idle for minutes while the background must keep moving.
pub fn spawn_auth(
    username: String,
    session_cmd: Vec<String>,
    to_ui: Sender<Ui>,
    from_ui: Receiver<Answer>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut conn = match Connection::from_env() {
            Ok(c) => c,
            Err(e) => {
                let _ = to_ui.send(Ui::Dead(e.to_string()));
                return;
            }
        };

        let mut reply = match conn.round_trip(&Request::CreateSession {
            username: username.clone(),
        }) {
            Ok(r) => r,
            Err(e) => {
                let _ = to_ui.send(Ui::Dead(e.to_string()));
                return;
            }
        };

        loop {
            match reply {
                Response::AuthMessage {
                    auth_message_type,
                    auth_message,
                } => {
                    let next = if auth_message_type.wants_response() {
                        let _ = to_ui.send(Ui::Prompt {
                            text: auth_message,
                            secret: auth_message_type == AuthMessageType::Secret,
                        });
                        match from_ui.recv() {
                            Ok(Answer::Text(t)) => {
                                Request::PostAuthMessageResponse { response: Some(t) }
                            }
                            Ok(Answer::Cancel) | Err(_) => Request::CancelSession,
                        }
                    } else {
                        // info and error are statements. Show them and answer
                        // with no response at all, which is what the protocol
                        // expects and what makes a device-code prompt work.
                        let _ = to_ui.send(Ui::Message {
                            text: auth_message,
                            is_error: auth_message_type == AuthMessageType::Error,
                        });
                        Request::PostAuthMessageResponse { response: None }
                    };

                    let cancelled = matches!(next, Request::CancelSession);
                    reply = match conn.round_trip(&next) {
                        Ok(r) => r,
                        Err(e) => {
                            let _ = to_ui.send(Ui::Dead(e.to_string()));
                            return;
                        }
                    };
                    if cancelled {
                        let _ = to_ui.send(Ui::Dead("cancelled".into()));
                        return;
                    }
                }
                Response::Success => {
                    let _ = to_ui.send(Ui::Starting);
                    match conn.round_trip(&Request::StartSession {
                        cmd: session_cmd.clone(),
                        env: vec![],
                    }) {
                        Ok(Response::Success) => return,
                        Ok(Response::Error { description, .. }) => {
                            let _ = to_ui.send(Ui::Dead(description));
                            return;
                        }
                        Ok(other) => {
                            let _ = to_ui.send(Ui::Dead(format!("unexpected: {other:?}")));
                            return;
                        }
                        Err(e) => {
                            let _ = to_ui.send(Ui::Dead(e.to_string()));
                            return;
                        }
                    }
                }
                Response::Error {
                    error_type,
                    description,
                } => {
                    match error_type {
                        // R3: an auth failure is not fatal. Cancel the session,
                        // show the message, and start over.
                        ErrorType::AuthError => {
                            let _ = to_ui.send(Ui::Message {
                                text: description,
                                is_error: true,
                            });
                            let _ = conn.round_trip(&Request::CancelSession);
                            reply = match conn.round_trip(&Request::CreateSession {
                                username: username.clone(),
                            }) {
                                Ok(r) => r,
                                Err(e) => {
                                    let _ = to_ui.send(Ui::Dead(e.to_string()));
                                    return;
                                }
                            };
                        }
                        ErrorType::Error => {
                            let _ = to_ui.send(Ui::Dead(description));
                            return;
                        }
                    }
                }
            }
        }
    })
}

/// Frames handed from the GStreamer thread to the UI thread.
pub struct FrameBus {
    pub latest: Mutex<Option<Vec<u8>>>,
    pub size: Mutex<(u32, u32)>,
    pub decoded: std::sync::atomic::AtomicU64,
    pub presented: std::sync::atomic::AtomicU64,
    pub stop: AtomicBool,
    pub first_frame: Mutex<Option<Instant>>,
}

impl FrameBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            latest: Mutex::new(None),
            size: Mutex::new((0, 0)),
            decoded: std::sync::atomic::AtomicU64::new(0),
            presented: std::sync::atomic::AtomicU64::new(0),
            stop: AtomicBool::new(false),
            first_frame: Mutex::new(None),
        })
    }
}

/// A channel pair for the UI side, so `main` need not know the thread shapes.
pub fn ui_channels() -> (Sender<Ui>, Receiver<Ui>, Sender<Answer>, Receiver<Answer>) {
    let (tu, ru) = mpsc::channel();
    let (ta, ra) = mpsc::channel();
    (tu, ru, ta, ra)
}

/// Decode on a worker thread, handing the newest frame to the UI as BGRx.
///
/// Conversion and scaling happen on the GPU (`cudaconvertscale`) and only the
/// finished BGRx frame is pulled down (`cudadownload`), so the readback is one
/// copy of exactly what gets presented. This is still a readback, and R7 will
/// eventually want a dmabuf imported straight into a texture — but that choice
/// belongs to Q5, not to a spike.
pub fn spawn_video(
    file: String,
    width: u32,
    height: u32,
    bus: Arc<FrameBus>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use gstreamer as gst;
    use gstreamer::prelude::*;
    use gstreamer_app as gst_app;

    gst::init().map_err(|e| e.to_string())?;

    let hevc = file.contains("hevc") || file.contains("h265");
    let parse = if hevc { "h265parse" } else { "h264parse" };
    let dec = if hevc { "nvh265dec" } else { "nvh264dec" };
    let desc = format!(
        "filesrc location=\"{file}\" ! qtdemux ! {parse} ! {dec} \
         ! cudaconvertscale ! cudadownload \
         ! video/x-raw,format=BGRx,width={width},height={height} \
         ! appsink name=sink max-buffers=1 drop=true sync=true"
    );
    eprintln!("[spike] video pipeline: {desc}");

    let pipeline = gst::parse::launch(&desc)
        .map_err(|e| format!("pipeline build failed: {e}"))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| "not a pipeline".to_string())?;

    let sink = pipeline
        .by_name("sink")
        .ok_or("no appsink")?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| "sink is not an appsink".to_string())?;

    {
        let bus2 = Arc::clone(&bus);
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    *bus2.latest.lock().unwrap() = Some(map.as_slice().to_vec());
                    *bus2.size.lock().unwrap() = (width, height);
                    if bus2.decoded.fetch_add(1, Ordering::Relaxed) == 0 {
                        *bus2.first_frame.lock().unwrap() = Some(Instant::now());
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    pipeline
        .set_state(gst::State::Playing)
        .map_err(|e| format!("could not play: {e}"))?;

    Ok(std::thread::spawn(move || {
        let gbus = pipeline.bus().expect("bus");
        while !bus.stop.load(Ordering::Relaxed) {
            let Some(msg) = gbus.timed_pop(gst::ClockTime::from_mseconds(200)) else {
                continue;
            };
            match msg.view() {
                gst::MessageView::Eos(_) => {
                    // Seamless looping is R8's problem; here it only has to not
                    // stall. KEY_UNIT so the frames after the seam are valid.
                    let _ = pipeline.seek_simple(
                        gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                        gst::ClockTime::ZERO,
                    );
                }
                gst::MessageView::Error(e) => {
                    eprintln!("[spike] video error: {} ({:?})", e.error(), e.debug());
                    break;
                }
                _ => {}
            }
        }
        let _ = pipeline.set_state(gst::State::Null);
    }))
}

/// Load MesloLGS Nerd Font if present (R11's font), else anything legible.
/// Loaded by path on purpose: fontconfig has no writable cache when HOME is not
/// writable, which is the greeter's situation, and a font that needs a cache is
/// a font that fails in the one context that matters.
pub fn load_font() -> Option<fontdue::Font> {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/TTF/MesloLGSNerdFontMono-Regular.ttf",
        "/usr/share/fonts/TTF/MesloLGSNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/MesloLGSDZNerdFontMono-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    for p in CANDIDATES {
        if let Ok(bytes) = std::fs::read(p) {
            if let Ok(f) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                eprintln!("[spike] font: {p}");
                return Some(f);
            }
        }
    }
    eprintln!("[spike] no font found; text will not render");
    None
}
