//! Q5 spike: what does a real GL renderer actually cost, against `wl_shm`?
//!
//! The Q5 brief narrowed the question to one measurement and then refused to
//! decide without it: "a minimal GL renderer in auroval, drawing the video
//! texture and one text label, measured at realtime 1440p against the spike's
//! `wl_shm` path." This is that renderer, and nothing more.
//!
//! What makes the comparison fair, and why each part is here:
//!
//!   * **Both modes present the same clip to the same surface size, in
//!     realtime.** The earlier measurement ran as fast as possible, which
//!     answers "how much work is there" rather than "how much of a core does
//!     the greeter use", and the second is the question R7's budget asks.
//!   * **Both present on a Wayland frame callback**, pulling the newest sample
//!     available. The decode path differs; the presentation cadence does not.
//!   * **Both draw a text label**, because R11's typography is auroval's
//!     problem on the GL path and free on the shm path, and leaving it out
//!     would flatter GL.
//!
//! The number that matters is CPU seconds per wall second, read from
//! /proc/self/stat so it includes every thread GStreamer starts.

mod gl;
mod shm;
mod text;
mod win;

use std::time::Duration;

/// The clip's own resolution. The surface is made this size so that neither
/// path is quietly rescaling and paying for it.
pub fn probe_size(clip: &str) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
            clip,
        ])
        .output()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let mut it = s.trim().split('x');
    let w: u32 = it.next().ok_or("no width")?.trim().parse()?;
    let h: u32 = it.next().ok_or("no height")?.trim().parse()?;
    Ok((w, h))
}

/// CPU time this process and all its threads have used, in seconds.
pub fn cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // The comm field can contain spaces and parentheses, so split after it.
    let rest = match stat.rfind(") ") {
        Some(i) => &stat[i + 2..],
        None => return 0.0,
    };
    let f: Vec<&str> = rest.split_whitespace().collect();
    // After the comm and state fields, utime is field 11 and stime 12 here.
    let utime: f64 = f.get(11).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let stime: f64 = f.get(12).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let hz = 100.0; // USER_HZ; constant on every Linux this will ever run on
    (utime + stime) / hz
}

pub struct Report {
    pub mode: &'static str,
    pub width: u32,
    pub height: u32,
    pub wall: Duration,
    pub cpu: f64,
    pub presented: u64,
    pub decoded: u64,
    pub first_frame_ms: f64,
}

impl Report {
    pub fn print(&self) {
        let secs = self.wall.as_secs_f64();
        println!("\n  {} — {}x{}", self.mode, self.width, self.height);
        println!("    wall            {secs:.2} s");
        println!(
            "    cpu             {:.2} s  ({:.1}% of one core)",
            self.cpu,
            100.0 * self.cpu / secs.max(1e-9)
        );
        println!(
            "    decoded         {} frames  ({:.1} fps)",
            self.decoded,
            self.decoded as f64 / secs.max(1e-9)
        );
        println!(
            "    presented       {} frames  ({:.1} fps)",
            self.presented,
            self.presented as f64 / secs.max(1e-9)
        );
        println!(
            "    cpu per frame   {:.2} ms",
            1000.0 * self.cpu / self.decoded.max(1) as f64
        );
        println!(
            "    first frame     {:.0} ms after start",
            self.first_frame_ms
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: spike-q5 <gl|shm> <clip> [seconds]";
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let clip = match args.get(2) {
        Some(c) => c.clone(),
        None => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    let seconds: f64 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(20.0);

    gstreamer::init().expect("gstreamer init");

    let report = match mode {
        "gl" => gl::run(&clip, seconds),
        "shm" => shm::run(&clip, seconds),
        _ => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    match report {
        Ok(r) => r.print(),
        Err(e) => {
            eprintln!("  {mode} failed: {e}");
            std::process::exit(1);
        }
    }
}
