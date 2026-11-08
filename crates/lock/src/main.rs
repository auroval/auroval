//! auroval's lock screen — **the protocol skeleton, and nothing else.**
//!
//! # This binary cannot unlock a session, and that is the design
//!
//! There is no PAM code here. No conversation function, no password buffer,
//! no `libpam` in the dependency list, no code path that could authenticate
//! anybody. The field is drawn and your keystrokes go nowhere. `L1` in
//! [`docs/prd/lock.md`](../../../docs/prd/lock.md) is the requirement this
//! binary deliberately does not meet.
//!
//! The reason is narrow and worth stating plainly: the unlock path is the one
//! part of this project where a mistake locks someone out of their own
//! machine, and `ext-session-lock-v1` is explicit that it fails closed —
//! *"If the client dies while the session is locked the session remains
//! locked, possibly permanently depending on compositor policy."* So the
//! protocol work and the authentication work were separated, and this is the
//! first half: it proves the surface comes up, that the shared renderer
//! draws on it, that the video still negotiates `memory:GLMemory` there, and
//! what the present costs against R7's ceiling. Those are answerable without
//! anything that could lock a person out. The other half gets written with
//! the maintainer awake and a second TTY open.
//!
//! # What it is safe to run, and what is not
//!
//! Two modes, and the split between them is the safety property:
//!
//! * **`probe`** binds `ext_session_lock_manager_v1`, says whether it is
//!   there and at what version, lists the outputs, and exits. **It never
//!   calls `lock`**, so it cannot lock a screen. Safe on a live desktop, and
//!   this is the mode that answers L2's first half.
//! * **`rehearse`** locks the session for real. It refuses to start unless
//!   `AUROVAL_LOCK_REHEARSAL=1` is set, because a binary that locks a screen
//!   by being run with no arguments is a trap, and the trap would be sprung
//!   by exactly the person least expecting it.
//!
//! **The escape hatch, and why it exists even though it is wrong.** In
//! `rehearse`, two things release the lock: <kbd>Esc</kbd>, and the
//! `AUROVAL_LOCK_SECONDS` timer. That is an unconditional unlock with no
//! authentication, which in a real locker would be the whole vulnerability —
//! so it is not an unlock path, it is a rehearsal harness, and **it must be
//! deleted in the same commit that introduces PAM**. L9 in the PRD records
//! this as a tripwire rather than trusting anybody to remember it.
//!
//! It is still deliberately kept, rather than exiting without unlocking,
//! because the alternative is worse in a specific way: a skeleton that leaves
//! the session locked after every run can be tested exactly once per boot, so
//! nobody would test it.
//!
//! **No signal is handled, and that is the fail-closed half.** SIGTERM,
//! SIGINT and SIGKILL all leave the session locked: this process installs no
//! handler, and sctk's own `Drop` for the lock object sends `destroy` rather
//! than `unlock_and_destroy`, with a comment reading *"choosing not to unlock
//! here results in us failing secure."* So R14's property is in the skeleton
//! already, and it is the one thing here worth testing on purpose — with a
//! second TTY open before you start.
//!
//! The honest summary: run `rehearse` in a nested compositor. Running it on
//! the session you are sitting in works, and will lock your screen, and the
//! only ways back are a key this binary tells you about, a timer you have to
//! have set in advance, and a second TTY.

mod lock;

use auroval_ui::config::Config;

/// Where the configuration comes from, for now.
///
/// **This reads the greeter's file.** Not because the lock screen should be
/// configured by a file named `greet.lua` forever, but because inventing
/// `/etc/auroval/lock.lua` in a commit that installs nothing would create a
/// path that exists in the code and nowhere on disk — and the two surfaces
/// are supposed to look alike, so sharing one palette and one background
/// chain is the behaviour that matches the requirement today. Whether lock
/// gets its own file, and what it may override, is **N1** in the PRD.
const CONFIG: &str = "/etc/auroval/greet.lua";

const USAGE: &str = "usage: auroval-lock <probe|rehearse>

  probe     say whether ext-session-lock-v1 is available, at what version,
            and on how many outputs. Never locks anything. Safe to run on
            the desktop you are sitting in.
  rehearse  bring up a real ext-session-lock surface, play the background on
            it, draw the clock and the date, and report the present cost.

THIS BINARY CANNOT UNLOCK A SESSION. There is no PAM code in it and no
authentication of any kind -- the input field is drawn, and what you type
into it goes nowhere. See the module doc comment for why, and
docs/prd/lock.md for what is still missing.

`rehearse` LOCKS YOUR SCREEN. ext-session-lock-v1 fails closed: if this
process dies while the session is locked, the session stays locked. So:

  * it refuses to run unless AUROVAL_LOCK_REHEARSAL=1 is set,
  * Esc releases the lock and exits, and so does the AUROVAL_LOCK_SECONDS
    timer -- an unconditional unlock, which is a rehearsal harness and not
    a locker, and which has to be deleted when PAM is written,
  * NO SIGNAL IS HANDLED. SIGTERM, SIGINT and SIGKILL all leave the
    session locked. That is R14's property, it is already true of this
    skeleton, and it is the one thing here worth testing on purpose --
    with a second TTY open before you start.

Run it in a nested compositor unless you mean it.

AUROVAL_CONFIG overrides /etc/auroval/greet.lua.
AUROVAL_LOCK_SECONDS=<n> makes rehearse release the lock and exit after n
  seconds. For the measurement harness, which should not depend on anyone
  being at the keyboard.
AUROVAL_LOG_DRAWS logs one line a second: frames presented, turns of the
  loop that drew nothing, and this process's CPU as a share of one core.
  This is R7's 50%-of-one-core ceiling, asked on a lock surface.";

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("probe") => probe(),
        Some("rehearse") => rehearse(),
        // Deliberately no default. The greeter can fall through to its real
        // mode on a bare invocation because the worst case there is a login
        // prompt nobody asked for; here the worst case is a locked screen.
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn config_path() -> std::path::PathBuf {
    std::env::var_os("AUROVAL_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(CONFIG))
}

/// Load the config, with C10 applied the way the greeter applies it.
///
/// A locker runs as the logged-in user, so C10's original argument — a
/// greeter cannot traverse a home at mode 700 — does not bind it. The rule is
/// kept anyway, and the PRD argues for it under **L12**: the two surfaces
/// share a background chain, and a chain that works for one and not the other
/// is a worse failure than a chain that is refused in both. So the check is
/// reported, not enforced, in this skeleton: refusing outright would make the
/// skeleton untestable from a checkout, and it has nothing to protect yet.
fn load_config() -> Config {
    let path = config_path();
    if let Err(e) = auroval_ui::store::refuse_a_home(&path, "the configuration") {
        eprintln!("auroval-lock: {e}");
        eprintln!("auroval-lock:   allowed in this skeleton; L12 says what a real locker does");
    }
    let cfg = Config::load(&path);
    for p in &cfg.problems {
        eprintln!("auroval-lock: {p}");
    }
    cfg
}

/// L2's first half, asked without locking anything.
///
/// The greeter has a `probe` mode for the same reason and it earned its keep:
/// nearly everything surprising about one of these screens is about what it
/// can and cannot reach. For a locker the first such question is whether the
/// compositor will even offer the protocol — `ext-session-lock-v1` says in
/// its own description that *"The compositor may choose to restrict this
/// protocol to a special client launched by the compositor itself"*, so its
/// presence is policy and not a given.
fn probe() {
    let cfg = load_config();
    match lock::probe() {
        Ok(p) => {
            println!("protocol     ext-session-lock-v1");
            println!(
                "manager      {}",
                match &p.manager {
                    Some(v) => format!("ext_session_lock_manager_v1 version {v}"),
                    None => "ABSENT — this compositor will not let a client lock".into(),
                }
            );
            println!("outputs      {}", p.outputs.len());
            for o in &p.outputs {
                println!("  {o}");
            }
            println!("font         {}", p.font);
            println!(
                "{}",
                auroval_ui::clock::describe(auroval_ui::clock::locale_from_env())
                    .replace("locale ", "locale       ")
            );
            println!("config       {}", config_path().display());
            println!(
                "background   {}",
                if cfg.background.is_empty() {
                    "none configured — the solid colour is all R9 has left".to_string()
                } else {
                    cfg.background
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                }
            );
            println!("unlock       NONE — this binary cannot authenticate anybody");
            println!(
                "result       {}",
                if p.manager.is_some() {
                    "the protocol is available; `rehearse` would get a surface"
                } else {
                    "no manager global — nothing to build a locker on here"
                }
            );
        }
        Err(e) => {
            eprintln!("auroval-lock: {e}");
            std::process::exit(1);
        }
    }
}

/// Lock the session, draw on it, and say what it cost.
fn rehearse() {
    if std::env::var_os("AUROVAL_LOCK_REHEARSAL").as_deref() != Some(std::ffi::OsStr::new("1")) {
        eprintln!("auroval-lock: refusing to lock without AUROVAL_LOCK_REHEARSAL=1.");
        eprintln!("auroval-lock: this mode locks the screen and cannot authenticate anybody.");
        eprintln!("auroval-lock: `auroval-lock probe` answers the protocol question safely.");
        std::process::exit(2);
    }
    eprintln!("auroval-lock: REHEARSAL. This locks the session and has no unlock path.");
    eprintln!("auroval-lock: Esc releases the lock. No signal does -- not even SIGTERM.");
    let cfg = load_config();
    if let Err(e) = lock::run(cfg) {
        eprintln!("auroval-lock: {e}");
        std::process::exit(1);
    }
}
