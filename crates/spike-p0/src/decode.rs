//! The measurement that retires C6's unverified half.
//!
//! The question is narrow: in a process with no user session, no
//! `XDG_RUNTIME_DIR` of the logging-in user, and running as greetd's own
//! unprivileged user, can a 1440p60 video be decoded *in hardware*, at rate,
//! for as long as a login screen is on screen?
//!
//! Three things have to be true for a "yes", and the report states each
//! separately so that a partial result cannot be read as a pass:
//!
//! 1. A hardware decoder element is actually instantiated — not merely present
//!    in the registry. We record the factory names GStreamer chose.
//! 2. Frames arrive in GPU memory, which the sample caps prove via a memory
//!    feature such as `memory:CUDAMemory`, `memory:VAMemory` or
//!    `memory:DMABuf`. Plain `video/x-raw` means the frames were copied down to
//!    system memory, which is a different and much more expensive answer.
//! 3. The rate holds with CPU use consistent with the GPU doing the work.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Path_ {
    /// Let GStreamer rank-select. This is what the real greeter will do.
    Auto,
    /// Force CUDA NVDEC (`nvh264dec` / `nvh265dec`).
    Nvdec,
    /// Force VA-API (`vah264dec` / `vah265dec`), needs `gst-plugin-va`.
    Vaapi,
    /// Force libav software decode — the comparison baseline.
    Software,
}

impl Path_ {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "auto" => Self::Auto,
            "nvdec" => Self::Nvdec,
            "vaapi" | "va" => Self::Vaapi,
            "sw" | "software" => Self::Software,
            _ => return None,
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Nvdec => "nvdec",
            Self::Vaapi => "vaapi",
            Self::Software => "software",
        }
    }
}

pub struct Settings {
    pub file: String,
    pub path: Path_,
    /// `true` runs the clock, so the result answers "does it hold 60 fps".
    /// `false` decodes as fast as it can, so the result answers "how much
    /// headroom is there".
    pub realtime: bool,
    pub duration: Duration,
    /// Codec of the file, used only to name the forced decoder element.
    pub hevc: bool,
}

#[derive(Debug)]
pub struct Measurement {
    pub path: Path_,
    pub realtime: bool,
    pub frames: u64,
    pub wall: Duration,
    pub cpu_seconds: f64,
    /// Elements GStreamer actually built, in order of creation.
    pub elements: Vec<String>,
    /// Caps string of the first sample — proves where the frames live.
    pub caps: Option<String>,
    /// Loop boundaries crossed, i.e. how many times the file restarted. R8's
    /// seam is a separate question; this only proves looping does not stall.
    pub loops: u64,
    /// Reported by the sink: buffers that arrived too late to show.
    pub late: u64,
    pub error: Option<String>,
}

impl Measurement {
    pub fn fps(&self) -> f64 {
        self.frames as f64 / self.wall.as_secs_f64()
    }
    /// CPU seconds burned per second of wall clock, as a percentage of one
    /// core. Hardware decode should sit far below software decode here; that
    /// gap is the strongest evidence the GPU did the work.
    pub fn cpu_percent(&self) -> f64 {
        100.0 * self.cpu_seconds / self.wall.as_secs_f64()
    }
    /// Whether the frames stayed in GPU memory.
    pub fn gpu_memory(&self) -> bool {
        self.caps.as_deref().is_some_and(|c| {
            c.contains("memory:CUDAMemory")
                || c.contains("memory:VAMemory")
                || c.contains("memory:DMABuf")
                || c.contains("memory:NVMM")
                || c.contains("memory:GLMemory")
        })
    }
    /// Whether a hardware decoder element was instantiated.
    pub fn hardware_decoder(&self) -> Option<&str> {
        self.elements
            .iter()
            .find(|e| {
                let e = e.as_str();
                (e.starts_with("nv") || e.starts_with("va")) && e.contains("dec")
            })
            .map(|s| s.as_str())
    }
}

fn pipeline_description(s: &Settings) -> String {
    let parse = if s.hevc { "h265parse" } else { "h264parse" };
    // `sync` on the sink is what makes the realtime/headroom distinction.
    // `max-buffers=2 drop=false` keeps backpressure honest: if the sink cannot
    // keep up, the decoder is made to wait rather than frames being silently
    // discarded, so the frame count means what it says.
    let sink = "appsink name=sink max-buffers=2 drop=false";
    match s.path {
        // decodebin3 picks by rank, which is exactly what the real greeter
        // would get. Nothing is forced, so this is the load-bearing run.
        Path_::Auto => format!(
            "filesrc location=\"{}\" ! parsebin ! decodebin3 ! {sink}",
            s.file
        ),
        Path_::Nvdec => {
            let dec = if s.hevc { "nvh265dec" } else { "nvh264dec" };
            format!(
                "filesrc location=\"{}\" ! qtdemux ! {parse} ! {dec} ! {sink}",
                s.file
            )
        }
        Path_::Vaapi => {
            let dec = if s.hevc { "vah265dec" } else { "vah264dec" };
            format!(
                "filesrc location=\"{}\" ! qtdemux ! {parse} ! {dec} ! {sink}",
                s.file
            )
        }
        Path_::Software => {
            let dec = if s.hevc { "avdec_h265" } else { "avdec_h264" };
            format!(
                "filesrc location=\"{}\" ! qtdemux ! {parse} ! {dec} ! {sink}",
                s.file
            )
        }
    }
}

pub fn run(s: &Settings) -> Measurement {
    let desc = pipeline_description(s);
    eprintln!("[spike] pipeline: {desc}");

    let mut m = Measurement {
        path: s.path,
        realtime: s.realtime,
        frames: 0,
        wall: Duration::ZERO,
        cpu_seconds: 0.0,
        elements: Vec::new(),
        caps: None,
        loops: 0,
        late: 0,
        error: None,
    };

    let element = match gst::parse::launch(&desc) {
        Ok(e) => e,
        Err(e) => {
            m.error = Some(format!("pipeline would not build: {e}"));
            return m;
        }
    };
    let pipeline = match element.downcast::<gst::Pipeline>() {
        Ok(p) => p,
        Err(_) => {
            m.error = Some("parse::launch did not yield a Pipeline".into());
            return m;
        }
    };

    // Record every element GStreamer builds, at any depth. In `Auto` this is
    // how we learn which decoder rank-selection actually chose, which is the
    // thing a registry listing cannot tell us.
    let elements = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let elements = Arc::clone(&elements);
        pipeline.connect_deep_element_added(move |_, _, el| {
            if let Some(f) = el.factory() {
                elements.lock().unwrap().push(f.name().to_string());
            }
        });
    }
    for el in pipeline.iterate_elements().into_iter().flatten() {
        if let Some(f) = el.factory() {
            elements.lock().unwrap().push(f.name().to_string());
        }
    }

    let sink = match pipeline.by_name("sink") {
        Some(s) => s
            .downcast::<gst_app::AppSink>()
            .expect("sink is an appsink"),
        None => {
            m.error = Some("no appsink named `sink`".into());
            return m;
        }
    };
    sink.set_property("sync", s.realtime);

    let frames = Arc::new(AtomicU64::new(0));
    let caps = Arc::new(Mutex::new(None::<String>));
    {
        let frames = Arc::clone(&frames);
        let caps = Arc::clone(&caps);
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let Ok(sample) = sink.pull_sample() else {
                        return Err(gst::FlowError::Eos);
                    };
                    if frames.fetch_add(1, Ordering::Relaxed) == 0 {
                        if let Some(c) = sample.caps() {
                            *caps.lock().unwrap() = Some(c.to_string());
                        }
                    }
                    // The frame is dropped here without being mapped. That is
                    // deliberate: mapping would pull GPU memory down to the CPU
                    // and turn a decode measurement into a readback
                    // measurement. Presentation is measured separately.
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }

    if let Err(e) = pipeline.set_state(gst::State::Playing) {
        m.error = Some(format!("could not reach Playing: {e}"));
        let _ = pipeline.set_state(gst::State::Null);
        m.elements = std::mem::take(&mut *elements.lock().unwrap());
        return m;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let loops = Arc::new(AtomicU64::new(0));
    let cpu_before = crate::sysprobe::cpu_seconds();
    let started = Instant::now();

    // Drive the bus until the duration elapses, restarting the file on EOS so
    // that looping is exercised rather than assumed.
    let bus = pipeline.bus().expect("pipeline has a bus");
    while started.elapsed() < s.duration && !stop.load(Ordering::Relaxed) {
        let remaining = s.duration.saturating_sub(started.elapsed());
        let slice = remaining.min(Duration::from_millis(200));
        let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(slice.as_millis() as u64))
        else {
            continue;
        };
        match msg.view() {
            gst::MessageView::Eos(_) => {
                loops.fetch_add(1, Ordering::Relaxed);
                // FLUSH so the restart is immediate; KEY_UNIT because a loop
                // boundary must land on a keyframe or the first frames after
                // the seam are garbage. R8's seam quality is a separate test.
                if pipeline
                    .seek_simple(
                        gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                        gst::ClockTime::ZERO,
                    )
                    .is_err()
                {
                    m.error = Some("loop seek failed".into());
                    break;
                }
            }
            gst::MessageView::Error(e) => {
                m.error = Some(format!(
                    "{} ({})",
                    e.error(),
                    e.debug().unwrap_or_else(|| "no debug info".into())
                ));
                break;
            }
            gst::MessageView::Qos(q) => {
                // The sink reporting QoS means frames arrived too late to show.
                // In realtime mode this is the signal R7 cares about. `stats`
                // carries the sink's own running totals, so taking the maximum
                // rather than summing avoids double-counting repeated reports.
                let (_processed, dropped) = q.stats();
                if let gst::GenericFormattedValue::Buffers(Some(d)) = dropped {
                    m.late = m.late.max(*d.as_ref());
                }
            }
            _ => {}
        }
    }

    m.wall = started.elapsed();
    m.cpu_seconds = crate::sysprobe::cpu_seconds() - cpu_before;
    let _ = pipeline.set_state(gst::State::Null);

    m.frames = frames.load(Ordering::Relaxed);
    m.loops = loops.load(Ordering::Relaxed);
    m.caps = caps.lock().unwrap().clone();
    let mut els = std::mem::take(&mut *elements.lock().unwrap());
    els.dedup();
    m.elements = els;
    m
}
