//! auroval's lock screen.
//!
//! # It can unlock a session now, and that changes what this file is for
//!
//! The previous version of this comment opened *"This binary cannot unlock a
//! session, and that is the design"*, and the design was to separate the
//! protocol work from the authentication work so that the first could be
//! finished without anything that could lock a person out. That half is
//! merged and measured — `spike/p3/RESULTS.md` has the protocol at version 1,
//! the renderer drawing on a lock surface, and the loop staying in a GL
//! texture there. This is the second half.
//!
//! What authenticates is `src/pam.rs`: four functions and three structs
//! transcribed from `pam_conv(3)` and `/usr/include/security/_pam_types.h`,
//! for the reason `crates/greetd-proto` exists. No PAM crate. The service is
//! `auroval-lock`, written by `pc-setup`, which is `auth include login` and
//! has no `account` stanza — so nothing here calls `pam_acct_mgmt`, and
//! `pam.rs` says why.
//!
//! # The escape hatch is gone, and that is L9
//!
//! <kbd>Esc</kbd> did release the lock, unconditionally, and so did the
//! `AUROVAL_LOCK_SECONDS` timer. L9 required both gone in the same commit
//! that introduced PAM, not after it, and they are:
//!
//! * **Escape does nothing.** The key is matched and dropped, so the absence
//!   is visible in the code rather than being an omission.
//! * **The deadline exits without unlocking.** It still bounds a run, so
//!   `spike/p3/lock-measure.nu` works unchanged, but the session stays
//!   locked. The operator's job changes: a nested compositor has to be
//!   closed rather than politely releasing itself.
//!
//! So there is exactly one path out of a held lock, and it runs through a
//! successful `pam_authenticate`. The compiler helps: the binding cannot be
//! committed without a caller, because `clippy -D warnings` refuses dead
//! code, which is why these two things are one commit and not two.
//!
//! # What holds the faillock trap shut
//!
//! `/etc/security/faillock.conf` is commented out, so three failures lock the
//! account for ten minutes — the mechanism `AGENTS.md` warns about for
//! `sudo`. The stack is left at its defaults deliberately, because the trap
//! is sprung by a locker that retries rather than by the policy. L10 is what
//! forbids retrying, and it is enforced by a state rather than by care:
//! while PAM is thinking every key is dropped, so a held Return, a burst of
//! synthetic events and a keystroke during the two-second failure delay all
//! produce one attempt. `may_submit` in `lock.rs` is that decision, as a free
//! function, so it has tests.
//!
//! PAM runs on a worker thread. `pam_unix(8)` documents a delay-on-failure
//! *"of the order of two seconds"* and the stack does not pass `nodelay`, so
//! on the draw loop a wrong password would freeze the screen mid-animation.
//!
//! # Two modes, and the split is still the safety property
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
//! **No signal is handled, and that is the fail-closed half.** SIGTERM,
//! SIGINT and SIGKILL all leave the session locked: this process installs no
//! handler, and sctk's own `Drop` for the lock object sends `destroy` rather
//! than `unlock_and_destroy`, with a comment reading *"choosing not to unlock
//! here results in us failing secure."*
//!
//! The honest summary: run `rehearse` in a nested compositor, which is what
//! `spike/p3/nested.conf` is for. On the session you are sitting in it will
//! lock your screen, and now the only way back is the right password —
//! there is no key and no timer. If PAM is misconfigured there is no way back
//! at all except killing the compositor from another VT, which ends the
//! session and everything unsaved in it.

mod lock;
mod pam;

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
  * THERE IS NO ESCAPE KEY. Esc used to release the lock and no longer
    does, per L9: the only way out of a held lock is a correct password.
  * AUROVAL_LOCK_SECONDS still bounds a run, but it now exits WITHOUT
    unlocking, so the session stays locked and a nested compositor has to
    be closed,
  * NO SIGNAL IS HANDLED. SIGTERM, SIGINT and SIGKILL all leave the
    session locked. That is R14's property, it is already true of this
    skeleton, and it is the one thing here worth testing on purpose --
    with a second TTY open before you start.

Run it in a nested compositor unless you mean it.

AUROVAL_CONFIG overrides /etc/auroval/greet.lua.
AUROVAL_LOCK_SECONDS=<n> makes rehearse exit after n seconds WITHOUT
  unlocking. For the measurement harness, which should not depend on anyone
  being at the keyboard. The session is left locked on purpose -- see L9.
AUROVAL_LOCK_PAM_SERVICE=<name> overrides the PAM service, default
  `auroval-lock`. For trying a stack of your own without editing
  /etc/pam.d; it cannot make authentication weaker than that file is.
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
            // The pool, and which rung of which candidate this binary would
            // actually show. `greet probe` prints the same pool with the
            // rotation state beside it; this one deliberately cannot, because
            // the state is unreadable to the user this process runs as. What
            // it prints instead is the truth for a locker: candidate 0.
            match cfg.background.len() {
                0 => println!("background   none configured — the solid colour is all R9 has left"),
                n => {
                    println!(
                        "background   {n} candidate{}, this binary shows the first and never rotates",
                        if n == 1 { "" } else { "s" }
                    );
                    for (i, chain) in cfg.background.iter().enumerate() {
                        println!(
                            "  [{i}]{}       {}",
                            if i == 0 { " shown" } else { "      " },
                            chain
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join(" -> ")
                        );
                    }
                }
            }
            // What `probe` can say about the unlock path without touching
            // it: which service it would ask and for whom. It does not call
            // pam_start -- a probe that authenticated would be a probe that
            // could spend one of faillock's three.
            match pam::own_username() {
                Ok(u) => println!(
                    "unlock       PAM service `{}`, user `{}` (not contacted)",
                    std::env::var("AUROVAL_LOCK_PAM_SERVICE")
                        .unwrap_or_else(|_| "auroval-lock".into()),
                    u
                ),
                Err(e) => println!("unlock       NO USER — {e}"),
            }
            println!(
                "             /etc/pam.d/{} {}",
                std::env::var("AUROVAL_LOCK_PAM_SERVICE").unwrap_or_else(|_| "auroval-lock".into()),
                if std::path::Path::new(&format!(
                    "/etc/pam.d/{}",
                    std::env::var("AUROVAL_LOCK_PAM_SERVICE")
                        .unwrap_or_else(|_| "auroval-lock".into())
                ))
                .exists()
                {
                    "exists"
                } else {
                    "IS MISSING — rehearse would lock a screen nothing can unlock"
                }
            );
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
        eprintln!("auroval-lock: this mode locks the screen. The only way out is your password.");
        eprintln!("auroval-lock: `auroval-lock probe` answers the protocol question safely.");
        std::process::exit(2);
    }
    // The refusal that matters most in this whole file.
    //
    // Without its PAM service file, `pam_start` has no stack to run and
    // nothing anybody types can succeed. Locking the screen then is not a
    // risk of a lockout, it is a lockout with extra steps -- and since L9
    // took away the escape key and the timer's unlock, the only way out
    // would be killing the compositor from another VT and losing the
    // session. So this is checked before the lock is asked for, not after.
    let service =
        std::env::var("AUROVAL_LOCK_PAM_SERVICE").unwrap_or_else(|_| "auroval-lock".into());
    let stack = format!("/etc/pam.d/{service}");
    if !std::path::Path::new(&stack).exists() {
        eprintln!("auroval-lock: {stack} does not exist, so nothing could unlock this screen.");
        eprintln!("auroval-lock: refusing to lock. `auroval-lock probe` reports this too.");
        eprintln!(
            "auroval-lock: the file is written by pc-setup -- one line, `auth include login`."
        );
        std::process::exit(2);
    }

    eprintln!("auroval-lock: REHEARSAL. This locks the session.");
    eprintln!("auroval-lock: There is NO escape key. Your password unlocks it; nothing else does.");
    eprintln!("auroval-lock: No signal unlocks it either -- not even SIGKILL (L11).");
    let cfg = load_config();
    if let Err(e) = lock::run(cfg) {
        eprintln!("auroval-lock: {e}");
        std::process::exit(1);
    }
}
