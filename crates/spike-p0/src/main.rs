//! auroval P0 spike — throwaway code answering one question.
//!
//! PRD C6 records that hardware decode is available on this machine, and that
//! whether it works *in the greeter's context* is unverified. This binary
//! measures that, and drives a real greetd login so the context is the genuine
//! one rather than a simulation of it.
//!
//! Modes:
//!   probe              print this process's context (uid, session env, devices)
//!   decode <file>      measure decode; --path auto|nvdec|vaapi|sw, --headroom
//!   greet  <file>      Wayland client: video background + real greetd login
//!
//! Nothing here is meant to survive into `greet` except `greetd-proto`.

mod decode;
mod greet;
mod greet_ui;
mod sysprobe;

use gstreamer::glib::translate::IntoGlib;
use gstreamer::prelude::PluginFeatureExtManual;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("probe");

    match mode {
        "probe" => {
            println!("{}", sysprobe::report());
            println!("{}", gst_report());
        }
        "decode" => decode_main(&args[1..]),
        "greet" => greet_main(&args[1..]),
        other => {
            eprintln!("unknown mode `{other}`; try probe | decode | greet");
            std::process::exit(2);
        }
    }
}

/// What GStreamer thinks it can do, as opposed to what it can actually do.
/// The distinction matters: `gst-plugin-va` installs a plugin that registers
/// zero features on this machine, so a registry listing is not evidence.
fn gst_report() -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    if let Err(e) = gstreamer::init() {
        let _ = writeln!(s, "## GStreamer\n\n- **init failed**: {e}");
        return s;
    }
    let _ = writeln!(s, "## GStreamer\n");
    let _ = writeln!(s, "- version: {}", gstreamer::version_string());
    // `find` consults the registry; a present factory is still not proof it
    // can be instantiated, which is why `decode` instantiates for real.
    for name in [
        "nvh264dec",
        "nvh265dec",
        "vah264dec",
        "vah265dec",
        "avdec_h264",
        "avdec_h265",
    ] {
        match gstreamer::ElementFactory::find(name) {
            Some(f) => {
                let rank = f.rank();
                let buildable = f.create().build().is_ok();
                let _ = writeln!(
                    s,
                    "- `{name}`: present, rank {}, instantiates: {}",
                    rank.into_glib(),
                    if buildable { "yes" } else { "**no**" }
                );
            }
            None => {
                let _ = writeln!(s, "- `{name}`: **absent**");
            }
        }
    }
    s
}

fn decode_main(args: &[String]) {
    let mut file = None;
    let mut path = decode::Path_::Auto;
    let mut realtime = true;
    let mut secs = 60u64;

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--path" => {
                let v = it.next().map(String::as_str).unwrap_or("");
                match decode::Path_::parse(v) {
                    Some(p) => path = p,
                    None => {
                        eprintln!("--path must be auto|nvdec|vaapi|sw, got `{v}`");
                        std::process::exit(2);
                    }
                }
            }
            // Decode as fast as possible instead of at 60 fps, to measure
            // headroom rather than whether the rate holds.
            "--headroom" => realtime = false,
            "--seconds" => {
                secs = it.next().and_then(|v| v.parse().ok()).unwrap_or(60);
            }
            f if !f.starts_with("--") => file = Some(f.to_string()),
            other => {
                eprintln!("unknown flag `{other}`");
                std::process::exit(2);
            }
        }
    }

    let Some(file) = file else {
        eprintln!("usage: spike-p0 decode <file.mp4> [--path auto|nvdec|vaapi|sw] [--headroom] [--seconds N]");
        std::process::exit(2);
    };

    if let Err(e) = gstreamer::init() {
        eprintln!("GStreamer init failed: {e}");
        std::process::exit(1);
    }

    let hevc = file.contains("hevc") || file.contains("h265");
    let m = decode::run(&decode::Settings {
        file: file.clone(),
        path,
        realtime,
        duration: Duration::from_secs(secs),
        hevc,
    });

    // One line of machine-readable output per run, so several runs can be
    // compared without re-reading prose.
    println!("--- result ---");
    println!("path          {}", m.path.label());
    println!(
        "mode          {}",
        if m.realtime {
            "realtime (sync=true)"
        } else {
            "headroom (sync=false)"
        }
    );
    println!("file          {file}");
    println!("frames        {}", m.frames);
    println!("wall          {:.2}s", m.wall.as_secs_f64());
    println!("fps           {:.2}", m.fps());
    println!(
        "cpu           {:.2}s ({:.1}% of one core)",
        m.cpu_seconds,
        m.cpu_percent()
    );
    println!("loops         {}", m.loops);
    println!("late buffers  {}", m.late);
    println!("elements      {}", m.elements.join(" -> "));
    println!(
        "hw decoder    {}",
        m.hardware_decoder().unwrap_or("NONE (software)")
    );
    println!(
        "gpu memory    {}",
        if m.gpu_memory() {
            "yes"
        } else {
            "no (frames copied to system memory)"
        }
    );
    println!("caps          {}", m.caps.as_deref().unwrap_or("<none>"));
    match &m.error {
        Some(e) => {
            println!("error         {e}");
            std::process::exit(1);
        }
        None => println!("error         none"),
    }
}

/// Phase 2: the whole path at once — Wayland surface under `cage`, hardware
/// decode, and a real greetd login.
fn greet_main(args: &[String]) {
    let mut video = None;
    let mut username = None;
    let mut session = vec!["Hyprland".to_string()];
    let mut run_for: Option<std::time::Duration> = None;

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--user" => username = it.next().cloned(),
            "--session" => {
                if let Some(s) = it.next() {
                    session = s.split_whitespace().map(str::to_string).collect();
                }
            }
            // Unattended measurement: stop after N seconds and report.
            "--seconds" => {
                run_for = it
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(std::time::Duration::from_secs);
            }
            f if !f.starts_with("--") => video = Some(f.to_string()),
            other => {
                eprintln!("unknown flag `{other}`");
                std::process::exit(2);
            }
        }
    }

    let Some(username) = username else {
        eprintln!("usage: spike-p0 greet <video.mp4> --user <name> [--session CMD] [--seconds N]");
        std::process::exit(2);
    };

    let bus = greet::FrameBus::new();
    let (to_ui, ui_rx, ans_tx, ans_rx) = greet::ui_channels();

    if video.is_none() {
        eprintln!("[spike] no video given; solid colour only");
    }

    let _auth = greet::spawn_auth(username, session, to_ui, ans_rx);

    // The video pipeline is built inside the UI, on the first `configure`, so
    // that it decodes at exactly the surface size the compositor granted.
    if let Err(e) = greet_ui::run(bus, ui_rx, ans_tx, video, run_for) {
        eprintln!("[spike] wayland: {e}");
        std::process::exit(1);
    }
}
