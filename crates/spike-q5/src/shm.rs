//! The `wl_shm` path: what the P0 spike did, kept honest for comparison.
//!
//! Every frame is downloaded from the GPU by GStreamer, converted to BGRx on
//! the CPU, copied into a shared-memory buffer, and composited by the
//! compositor. R7's budget --- set from C6 --- forbids exactly this, so the
//! point of measuring it is to say by how much.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gstreamer::prelude::*;
use gstreamer_app::AppSink;
use smithay_client_toolkit::shm::slot::SlotPool;
use wayland_client::protocol::{wl_shm, wl_surface};
use wayland_client::Connection;

use crate::text::{label, Label};
use crate::win::{self, Renderer, Surface};
use crate::Report;

pub struct ShmRenderer {
    sink: AppSink,
    pipeline: gstreamer::Pipeline,
    decoded: Arc<AtomicU64>,
    label: Label,
    clip: String,
    seconds: f64,
}

impl Renderer for ShmRenderer {
    fn mode(&self) -> &'static str {
        "shm  (readback, BGRx, wl_shm)"
    }
    fn decoded(&self) -> u64 {
        self.decoded.load(Ordering::Relaxed)
    }

    fn configure(&mut self, _: &Connection, _: &wl_surface::WlSurface, w: u32, h: u32) {
        // Scale on the GPU to the exact surface size: the greeter would, and
        // rescaling on the CPU here would measure the wrong thing.
        let desc = format!(
            "filesrc location={} ! decodebin3 ! videoconvertscale ! \
             video/x-raw,format=BGRx,width={w},height={h} ! \
             appsink name=sink max-buffers=2 drop=true sync=true",
            self.clip
        );
        let pipeline = gstreamer::parse::launch(&desc)
            .expect("shm pipeline parses")
            .downcast::<gstreamer::Pipeline>()
            .expect("is a pipeline");
        let sink = pipeline
            .by_name("sink")
            .expect("sink exists")
            .downcast::<AppSink>()
            .expect("is an appsink");
        let _ = self.seconds;
        self.pipeline = pipeline;
        self.sink = sink;
        self.pipeline
            .set_state(gstreamer::State::Playing)
            .expect("plays");
    }

    fn present(&mut self, s: &mut Surface<'_>) -> bool {
        let sample = match self.sink.try_pull_sample(gstreamer::ClockTime::ZERO) {
            Some(s) => s,
            None => {
                // Ask for another callback anyway, or the surface goes quiet.
                s.surface.frame(s.qh, s.surface.clone());
                s.surface.commit();
                return false;
            }
        };
        let n = self.decoded.fetch_add(1, Ordering::Relaxed);
        if n == 0 {
            if let Some(caps) = sample.caps() {
                println!("    negotiated      {caps}");
            }
        }
        let buffer = match sample.buffer() {
            Some(b) => b,
            None => return false,
        };
        let map = match buffer.map_readable() {
            Ok(m) => m,
            Err(_) => return false,
        };

        let (w, h) = (s.width, s.height);
        let stride = w as i32 * 4;
        if s.pool.is_none() {
            *s.pool = Some(SlotPool::new((stride * h as i32) as usize, s.shm).expect("shm pool"));
        }
        let pool = s.pool.as_mut().unwrap();
        let (wl_buffer, canvas) = pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Xrgb8888)
            .expect("shm buffer");

        let n = canvas.len().min(map.size());
        canvas[..n].copy_from_slice(&map.as_slice()[..n]);
        if std::env::var_os("AUROVAL_NO_TEXT").is_none() {
            blit(canvas, w as usize, h as usize, &self.label);
        }

        s.surface.damage_buffer(0, 0, w as i32, h as i32);
        s.surface.frame(s.qh, s.surface.clone());
        wl_buffer.attach_to(s.surface).expect("attach");
        s.surface.commit();
        true
    }
}

/// White text with a soft dark drop, bottom-left. BGRx in, BGRx out.
fn blit(canvas: &mut [u8], w: usize, h: usize, l: &Label) {
    let x0 = 64usize;
    let y0 = h.saturating_sub(l.h + 64);
    for row in 0..l.h {
        for col in 0..l.w {
            let a = l.alpha[row * l.w + col] as u32;
            if a == 0 {
                continue;
            }
            let (x, y) = (x0 + col, y0 + row);
            if x >= w || y >= h {
                continue;
            }
            let i = (y * w + x) * 4;
            for c in 0..3 {
                let src = 255u32;
                let dst = canvas[i + c] as u32;
                canvas[i + c] = ((src * a + dst * (255 - a)) / 255) as u8;
            }
        }
    }
}

pub fn run(clip: &str, seconds: f64) -> Result<Report, Box<dyn std::error::Error>> {
    let (w, h) = crate::probe_size(clip)?;
    let clip = clip.to_string();
    win::run_with(
        move || {
            Box::new(ShmRenderer {
                sink: AppSink::builder().build(),
                pipeline: gstreamer::Pipeline::new(),
                decoded: Arc::new(AtomicU64::new(0)),
                label: label("auroval — 00:00  Saturday, 7 November", 44.0),
                clip: clip.clone(),
                seconds,
            })
        },
        w,
        h,
        seconds,
    )
}
