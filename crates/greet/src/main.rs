//! auroval's login screen.
//!
//! Three modes, and the second one is why the first could be built at all:
//!
//!   * `greet` drives greetd and logs you in. Needs `GREETD_SOCK`.
//!   * `preview` draws the same screen with nothing behind it, so the look
//!     can be worked on without a login screen to log into.
//!   * `probe` prints what the greeter can see and exits, the way the P0
//!     spike's probe did. Nearly everything surprising about a greeter is
//!     about what it can and cannot reach.

mod auth;
mod background;
mod config;
mod render;
mod sessions;
mod shell;
mod store;
mod theme;
mod ui;
mod view;

use config::Config;
use store::Store;

/// The configuration, and how to point somewhere else at it.
///
/// `/etc/auroval/greet.lua` is where it lives on a deployed machine. The
/// override exists because everything else about this binary can be exercised
/// without root — the preview, the snapshot, the probe — and a config path
/// that could only be changed by writing to /etc would have made the config
/// the one thing nobody could test without it.
const CONFIG: &str = "/etc/auroval/greet.lua";

fn config_path() -> std::path::PathBuf {
    std::env::var_os("AUROVAL_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(CONFIG))
}

const USAGE: &str = "usage: auroval-greet <greet|preview|probe>

  greet    drive greetd and log in. Needs GREETD_SOCK.
  preview  the same screen with no greetd behind it, so the look can be
           worked on without a login screen to log into.
  probe    print what the greeter can see, and exit.

AUROVAL_CONFIG overrides /etc/auroval/greet.lua.
  snapshot <out.png> [stage]
           render one frame off-screen and write it, with no compositor and
           no greetd. stage is user | password | error | starting.";

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("probe") => probe(),
        Some("snapshot") => snapshot(),
        Some(mode @ ("greet" | "preview")) => {
            let preview = mode == "preview";
            if !preview && std::env::var_os("GREETD_SOCK").is_none() {
                eprintln!(
                    "auroval-greet: GREETD_SOCK is unset — this has to be started by greetd."
                );
                eprintln!("       `auroval-greet preview` runs the same screen without it.");
                std::process::exit(2);
            }
            // R13: the loader reports its own problems as it finds them, so
            // they are not reported a second time here. They stay on `cfg`
            // so the screen can show them too.
            let cfg = Config::load(&config_path());
            let found = sessions::discover();
            if found.is_empty() {
                eprintln!(
                    "auroval-greet: no sessions found in {:?}",
                    sessions::WAYLAND_DIRS
                );
            }
            let last = Store::at(&cfg.state_path).load();
            let ui = ui::UiState::new(found, &last);
            if let Err(e) = shell::run(cfg, ui, preview) {
                eprintln!("auroval-greet: {e}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// Say what the greeter can see, in the context it is run in. The P0 spike
/// had the same mode and it earned its keep: nearly everything surprising
/// about a greeter is about what it can and cannot reach.
fn probe() {
    let cfg = Config::load(&config_path());
    for p in &cfg.problems {
        println!("problem      {p}");
    }

    let found = sessions::discover();
    println!("sessions     {}", found.len());
    for s in &found {
        println!("  {:<22} {:?}  {}", s.id, s.kind, s.command.join(" "));
    }

    let store = Store::at(&cfg.state_path);
    let last = store.load();
    println!("state file   {}", cfg.state_path.display());
    println!("  last user    {}", last.user.as_deref().unwrap_or("—"));
    println!("  last session {}", last.session.as_deref().unwrap_or("—"));
    println!(
        "  writable     {}",
        match store.save(&last) {
            Ok(()) => "yes".to_string(),
            Err(e) => format!("no — {e}"),
        }
    );

    let state = ui::UiState::new(found, &last);
    println!(
        "preselected  {}",
        state.session().map(|s| s.id.as_str()).unwrap_or("—")
    );

    let g = cfg.geometry;
    println!(
        "geometry     clock {}px  date {}px  user {}px  field {}x{} r{} p{}",
        g.clock_px, g.date_px, g.user_px, g.field_w, g.field_h, g.field_rounding, g.rounding_power
    );
    println!("font         {}", render::atlas::FONT_CANDIDATES[0]);
    println!(
        "fonts found  {}",
        render::atlas::FONT_CANDIDATES
            .iter()
            .filter(|p| std::path::Path::new(p).is_file())
            .count()
    );
}

/// Render one frame off-screen, so the layout can be looked at and compared
/// without a compositor, without greetd, and in CI. This is the seam paying
/// off: the view is a pure function of `UiState`, so it can be posed.
fn snapshot() {
    let out = match std::env::args().nth(2) {
        Some(p) => p,
        None => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    let pose = std::env::args().nth(3).unwrap_or_else(|| "password".into());
    let (w, h) = (2560u32, 1440u32);

    let cfg = Config::load(&config_path());
    let mut state = ui::UiState::new(
        sessions::discover(),
        &store::LastLogin {
            user: Some("dirk".into()),
            session: None,
        },
    );
    let now = chrono::Local::now();
    state.clock = now.format("%H:%M").to_string();
    state.date = now.format("%A, %-d %B").to_string();
    match pose.as_str() {
        "user" => {}
        "error" => {
            state.stage = ui::Stage::Answering;
            state.prompt = "Password:".into();
            state.secret = true;
            state.typed = "hunter2".into();
            state.notice = Some(ui::Notice {
                text: "Authentication failure".into(),
                kind: ui::NoticeKind::Error,
            });
            state.caps_lock = true;
        }
        "starting" => {
            state.stage = ui::Stage::Starting;
        }
        _ => {
            state.stage = ui::Stage::Answering;
            state.prompt = "Password:".into();
            state.secret = true;
            state.typed = "correct horse".into();
        }
    }

    let (mut gl, target) = match unsafe { render::Gl::headless(w, h) } {
        Ok(v) => v,
        Err(e) => {
            eprintln!("auroval-greet: no off-screen context: {e}");
            std::process::exit(1);
        }
    };
    let style = view::Style {
        geometry: cfg.geometry,
        palette: cfg.palette,
    };
    let _ = gl.make_current(&target);

    // The decode runs on its own thread, which is right for a greeter and
    // wrong for a snapshot: one frame rendered immediately would always show
    // the fallback colour and the background would look broken. So this waits
    // for it, briefly and with a stated limit, rather than pretending.
    let mut bg = background::Background::start(cfg.background.as_deref());
    let mut ready = None;
    if cfg.background.is_some() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !bg.settled(&gl) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        ready = bg.ready(&gl);
        if ready.is_none() && bg.problems().is_empty() {
            eprintln!("auroval-greet: background did not arrive within 5 s; drawing the fallback");
        }
    }
    for p in bg.problems() {
        eprintln!("auroval-greet: {p}");
    }
    view::draw(&mut gl, &state, &style, ready);
    let pixels = gl.read_pixels();
    match image::RgbaImage::from_raw(w, h, pixels) {
        Some(img) => {
            if let Err(e) = img.save(&out) {
                eprintln!("auroval-greet: could not write {out}: {e}");
                std::process::exit(1);
            }
            println!("snapshot {pose} -> {out}");
        }
        None => {
            eprintln!("auroval-greet: read-back was the wrong size");
            std::process::exit(1);
        }
    }
}
