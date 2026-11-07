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

const USAGE: &str = "usage: auroval-greet <greet|preview|probe>

  greet    drive greetd and log in. Needs GREETD_SOCK.
  preview  the same screen with no greetd behind it, so the look can be
           worked on without a login screen to log into.
  probe    print what the greeter can see, and exit.
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
                eprintln!("greet: GREETD_SOCK is unset — this has to be started by greetd.");
                eprintln!("       `auroval-greet preview` runs the same screen without it.");
                std::process::exit(2);
            }
            let cfg = Config::load(std::path::Path::new("/etc/auroval/greet.lua"));
            // R13: problems are reported, never swallowed, and never fatal.
            for p in &cfg.problems {
                eprintln!("greet: {p}");
            }
            let found = sessions::discover();
            if found.is_empty() {
                eprintln!("greet: no sessions found in {:?}", sessions::WAYLAND_DIRS);
            }
            let last = Store::at(&cfg.state_path).load();
            let ui = ui::UiState::new(found, &last);
            if let Err(e) = shell::run(cfg, ui, preview) {
                eprintln!("greet: {e}");
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
    let cfg = Config::load(std::path::Path::new("/etc/auroval/greet.lua"));
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

    let cfg = Config::load(std::path::Path::new("/etc/auroval/greet.lua"));
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
            eprintln!("greet: no off-screen context: {e}");
            std::process::exit(1);
        }
    };
    let style = view::Style {
        geometry: cfg.geometry,
        palette: cfg.palette,
    };
    let _ = gl.make_current(&target);
    view::draw(&mut gl, &state, &style, None);
    let pixels = gl.read_pixels();
    match image::RgbaImage::from_raw(w, h, pixels) {
        Some(img) => {
            if let Err(e) = img.save(&out) {
                eprintln!("greet: could not write {out}: {e}");
                std::process::exit(1);
            }
            println!("snapshot {pose} -> {out}");
        }
        None => {
            eprintln!("greet: read-back was the wrong size");
            std::process::exit(1);
        }
    }
}
