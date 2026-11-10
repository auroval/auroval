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
// R9's pool of candidates, and the one integer it remembers between launches.
// This did NOT move into `auroval-ui` with the renderer, and the reason is a
// file mode rather than a preference: the index lives beside R4's login
// memory in `/var/lib/auroval`, which is `drwxr-x--- greeter greeter`, so a
// locker running as the logged-in user cannot read it, let alone advance it.
// `crates/lock` therefore takes the first candidate and does not rotate. See
// N1 in docs/prd/lock.md.
// Moved to the shared crate on 2026-11-10 so the locker can rotate too: it
// was private to the greeter, which is why N7 recorded the locker as taking
// candidate 0 for ever. Re-exported under its old path rather than touching
// every call site, because the move is the change and a rename alongside it
// would hide which is which in the diff.
use auroval_ui::rotation;
mod shell;

// Everything this binary draws with now lives in `auroval-ui`, so that the
// locker can draw the same screen without linking greetd. See that crate's
// doc comment for where the line was drawn and why.
use auroval_ui::{background, clock, config, render, sessions, store, ui, video, view};

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

/// Load the configuration with R15 applied to both paths it decides.
///
/// Every mode goes through here, because an audit on 2026-11-08 found that
/// only one of them was guarded and the guard it had ran after the read it
/// was supposed to prevent.
///
/// `strict` separates the two halves of C10, which are not the same rule.
/// **The state file is never allowed in a home, in any mode**: it belongs to
/// the greeter, nobody wants their last-login record in `~`, and refusing it
/// costs nothing. **The config path is only refused in `greet`**, because
/// `AUROVAL_CONFIG` exists precisely so a theme can be tried without root
/// (see `CONFIG` above), and refusing a developer their own file would
/// remove the reason the override is there. The other modes say what the
/// deployed greeter would have done, so the difference is never a surprise.
fn load_config(strict: bool) -> Config {
    let path = config_path();
    match store::refuse_a_home(&path, "the configuration") {
        Ok(()) => {}
        Err(e) if strict => {
            eprintln!("auroval-greet: {e}");
            std::process::exit(1);
        }
        Err(e) => eprintln!(
            "auroval-greet: {e}
  allowed in this mode; `greet` would refuse it"
        ),
    }

    let cfg = Config::load(&path);
    if let Err(e) = store::refuse_a_home(&cfg.state_path, "the state file") {
        eprintln!("auroval-greet: {e}");
        std::process::exit(1);
    }
    cfg
}

const USAGE: &str = "usage: auroval-greet <greet|preview|probe>

  greet    drive greetd and log in. Needs GREETD_SOCK.
  preview  the same screen with no greetd behind it, so the look can be
           worked on without a login screen to log into.
  probe    print what the greeter can see, and exit.

AUROVAL_CONFIG overrides /etc/auroval/greet.lua.
AUROVAL_EXIT_AFTER_FIRST_FRAME makes greet and preview exit as soon as
  they have drawn once and printed the R16 line. For the measurement
  harness; a login screen that does this is no login screen.
AUROVAL_LOG_KEYS logs one line per keyboard event to stderr: press or
  repeat, the keysym, and how many characters the field holds. Never the
  character itself. For the R6 round, which needs a real seat.
AUROVAL_LOG_DRAWS logs one line a second to stderr: how many turns of
  the loop drew a frame and how many did not. An idle login screen draws
  nothing, and that is a claim worth a counter — see
  spike/p1ab/draw-rate.nu, which reads exactly these lines.
AUROVAL_FRAME_DUMP=<dir> writes a PNG whenever what is on the screen
  changes, and a stderr line naming each one. For answering what the
  screen actually showed, after a console sitting. Refuses a directory
  inside a home, and slows the loop by the cost of a PNG per change.
  snapshot <out.png> [stage]
           render one frame off-screen and write it, with no compositor and
           no greetd. stage is user | password | prompt | error | starting.
  videoprobe <file> [seconds]
           decode a video into this renderer's own GL textures and say what
           was negotiated. R7's question, asked without a compositor.";

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("probe") => probe(),
        Some("snapshot") => snapshot(),
        Some("videoprobe") => videoprobe(),
        Some(mode @ ("greet" | "preview")) => greet(mode == "preview"),
        // Belt and braces for the one caller that matters. The startup
        // script names the mode, but a greeter that exits 2 on a missing
        // argument fails in the context where nobody can read stderr, so a
        // bare invocation with greetd behind it does the obvious thing.
        None if std::env::var_os("GREETD_SOCK").is_some() => greet(false),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// Say what the greeter can see, in the context it is run in. The P0 spike
/// had the same mode and it earned its keep: nearly everything surprising
/// about a greeter is about what it can and cannot reach.
/// One candidate's R9 chain, with each entry's presence stated. Whether the
/// file is there is the whole question a fallback chain exists to answer, and
/// a chain whose first entry is missing still works — silently, one rung
/// down. Printing it is how that stays visible.
fn print_chain(index: Option<usize>, chain: &[std::path::PathBuf]) {
    for (j, path) in chain.iter().enumerate() {
        let label = match (index, j) {
            (Some(i), 0) => format!("[{i}]"),
            _ => "   ".to_string(),
        };
        let present = if path.is_file() { "ok     " } else { "missing" };
        println!("  {label} {present} {}", path.display());
    }
}

fn probe() {
    let cfg = load_config(false);
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

    // Which background this launch would get. Reported rather than chosen:
    // see `rotation::last_shown`. A greeter that picks between candidates and
    // says nothing about it in its own diagnostic leaves the one thing that
    // varies per launch invisible, which is how an untagged colour range went
    // unnoticed for a week.
    let state_path = rotation::state_beside(&cfg.state_path);
    match cfg.background.len() {
        0 => println!("background   none — the solid colour"),
        1 => {
            println!("background   1 candidate, nothing to rotate");
            print_chain(None, &cfg.background[0]);
        }
        n => {
            let last = rotation::last_shown(&state_path);
            let next = rotation::next_after(last, n);
            println!(
                "background   {n} candidates, last shown {}, next {}",
                last.map(|i| i.to_string()).unwrap_or_else(|| "—".into()),
                next.map(|i| i.to_string())
                    .unwrap_or_else(|| "— (from the clock)".into()),
            );
            for (i, chain) in cfg.background.iter().enumerate() {
                print_chain(Some(i), chain);
            }
            println!("  state file   {}", state_path.display());
            println!(
                "  writable     {}",
                match rotation::can_write(&state_path) {
                    Ok(()) => "yes".to_string(),
                    Err(e) => format!("no — {e}"),
                }
            );
        }
    }

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
    // The same line the greeter prints at startup, so `probe` run as the
    // greeter answers the same question without a running greetd.
    println!(
        "{}",
        clock::describe(clock::locale_from_env()).replace("locale ", "locale       ")
    );
}

/// Render one frame off-screen, so the layout can be looked at and compared
/// without a compositor, without greetd, and in CI. This is the seam paying
/// off: the view is a pure function of `UiState`, so it can be posed.
/// This process's own CPU time so far, user plus system, in seconds.
///
/// From `/proc/self/stat` rather than from a child `ps`: the question is what
/// this process costs, and spawning something to ask would add its own.
fn own_cpu_seconds() -> f64 {
    let Ok(stat) = std::fs::read_to_string("/proc/self/stat") else {
        return 0.0;
    };
    // The comm field can contain spaces and parentheses, so the fields are
    // counted from after the last ')' rather than from the start.
    let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else {
        return 0.0;
    };
    let f: Vec<&str> = rest.split_whitespace().collect();
    // After the ')' the fields are state, ppid, ... so utime and stime are
    // the 12th and 13th here (fields 14 and 15 of the whole line).
    let ticks = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let hz = 100.0; // USER_HZ, fixed at 100 on Linux for this purpose.
    (ticks(11) + ticks(12)) / hz
}

/// R7's first question, asked without a compositor: does a decoded frame
/// reach *this renderer's* textures, and by which path?
///
/// `docs/decisions/q5-renderer.md` settled that `nvh264dec` negotiates
/// `memory:GLMemory` against a shared context, measured with `glimagesink`.
/// That is GStreamer talking to itself. This asks the same question with the
/// greeter's own EGL context on the other end, which is the arrangement the
/// greeter will actually use, and prints what was negotiated rather than
/// whether it played — a software decoder plays too, at fourteen times the
/// CPU.
fn videoprobe() {
    let path = match std::env::args().nth(2) {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    let seconds: u64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // Surfaceless, so this runs over ssh, in a container, or on a desktop,
    // and measures the decode rather than a compositor.
    let (gl, _target) = match unsafe { render::Gl::headless(640, 360) } {
        Ok(v) => v,
        Err(e) => {
            eprintln!("auroval-greet: no off-screen GL context: {e}");
            std::process::exit(1);
        }
    };
    println!("file         {}", path.display());
    println!("watching     {seconds} s");

    let video = match video::Video::start(&path, gl.egl_handles()) {
        Ok(v) => v,
        Err(e) => {
            println!("result       FAILED — {e}");
            std::process::exit(1);
        }
    };

    // CPU is read from this process rather than from `top`, because R7's
    // budget is about this process: "no more than 5% of one core", against
    // P0's measured 2.5% hardware and 36% software. The difference between
    // those two is the whole requirement, and it is not subtle enough to
    // need a careful instrument -- but it does need to be *this* process.
    let cpu_before = own_cpu_seconds();
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_secs(seconds);
    let mut first: Option<std::time::Duration> = None;
    let mut seen_textures: Vec<u32> = Vec::new();
    while std::time::Instant::now() < deadline {
        if let Some(f) = video.frame() {
            if first.is_none() {
                first = Some(started.elapsed());
            }
            if !seen_textures.contains(&f.texture) && seen_textures.len() < 16 {
                seen_textures.push(f.texture);
            }
        }
        // 16 ms, because that is what the greeter's own loop does. A
        // tighter poll measures this probe rather than the decoder.
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    let wall = started.elapsed().as_secs_f64();
    let cpu = own_cpu_seconds() - cpu_before;
    let frames = video.delivered();

    let n = video.negotiated();
    println!();
    println!(
        "caps         {}",
        if n.caps.is_empty() {
            "—".into()
        } else {
            n.caps
        }
    );
    println!(
        "memory       {}",
        if n.gl_memory {
            "GLMemory — the frame is already a texture in this context"
        } else {
            "NOT GLMemory — the frame was copied through system memory"
        }
    );
    match video.frame() {
        Some(f) => println!("size         {}x{}", f.width, f.height),
        None => println!("size         — (no frame arrived)"),
    }
    println!(
        "first frame  {}",
        match first {
            Some(d) => format!("{} ms", d.as_millis()),
            None => "never".into(),
        }
    );
    println!(
        "textures     {} distinct{}",
        seen_textures.len(),
        if seen_textures.len() > 1 {
            " (a pool, as expected)"
        } else {
            ""
        }
    );
    println!(
        "rate         {frames} frames in {wall:.1} s = {:.1}/s",
        frames as f64 / wall
    );
    println!(
        "cpu          {:.1}% of one core ({:.2} s of cpu in {wall:.1} s wall)",
        100.0 * cpu / wall,
        cpu
    );
    let said = video.drain();
    for p in said.warnings {
        println!("problem      {p}");
    }
    if let Some(e) = said.fatal {
        println!("error        {e}");
    }
    println!();
    println!(
        "result       {}",
        match (n.gl_memory, frames) {
            (true, f) if f > 0 => "the zero-copy path works from this renderer's context",
            (false, f) if f > 0 =>
                "frames arrive, but through system memory — R7's budget is at risk",
            _ => "NO FRAMES — see `problem` above, and R9 is what the greeter does about it",
        }
    );
}

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

    let cfg = load_config(false);
    let mut state = ui::UiState::new(
        sessions::discover(),
        &store::LastLogin {
            user: Some("dirk".into()),
            session: None,
        },
    );
    let (c, d) = clock::lines(chrono::Local::now(), clock::locale_from_env());
    state.clock = c;
    state.date = d;
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
        // The pose the console sitting had no word for: a secret prompt that
        // is not the password, with nothing typed into it yet.
        "prompt" => {
            state.stage = ui::Stage::Answering;
            state.prompt = "One-time password (OATH) for `dirk`:".into();
            state.secret = true;
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
    // One candidate out of the pool, the same way the greeter picks one, so a
    // snapshot photographs what a login would actually show. The state file
    // is the greeter's, so a snapshot advances the rotation too -- which is
    // correct: it is a launch.
    let chosen = rotation::choose(&cfg.background, &rotation::state_beside(&cfg.state_path));
    for p in &chosen.problems {
        eprintln!("auroval-greet: {p}");
    }
    let mut bg = background::Background::start(chosen.chain);
    let mut ready = None;
    if !chosen.chain.is_empty() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !bg.settled(&gl) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // A video background is a different thing to photograph than a
        // still: every run starts the clip at zero, so two snapshots of the
        // same file are the same picture and prove nothing about motion.
        // This waits a moment first, which is the difference between
        // "it drew a frame" and "it is playing".
        if let Some(ms) = std::env::var_os("AUROVAL_SNAPSHOT_AFTER_MS")
            .and_then(|v| v.to_string_lossy().parse::<u64>().ok())
        {
            let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
            while std::time::Instant::now() < until {
                let _ = bg.ready(&gl);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
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

/// Start the screen. `preview` skips greetd entirely, which is how the look
/// is worked on without a login screen to log into.
fn greet(preview: bool) {
    if !preview && std::env::var_os("GREETD_SOCK").is_none() {
        eprintln!("auroval-greet: GREETD_SOCK is unset — this has to be started by greetd.");
        eprintln!("       `auroval-greet preview` runs the same screen without it.");
        std::process::exit(2);
    }
    // R13: the loader reports its own problems as it finds them, so they are
    // not reported a second time here. They stay on `cfg` so the screen can
    // show them too.
    let cfg = load_config(!preview);
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
