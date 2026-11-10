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
//! `auroval-lock`, written by `pc-setup`, which is `auth include system-auth`
//! and has no `account` stanza — so nothing here calls `pam_acct_mgmt`, and
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
//! `spike/p3/nested.lua` is for. On the session you are sitting in it will
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

const USAGE: &str = "usage: auroval-lock <probe|lock|rehearse>

  probe     say whether ext-session-lock-v1 is available, at what version,
            and on how many outputs. Never locks anything. Safe to run on
            the desktop you are sitting in.
  lock      LOCK THIS SESSION. What an idle manager or a keybind points at.
            No environment gate -- and for that reason it refuses to start
            while any of the test hooks below are set.
  rehearse  the same lock, gated behind AUROVAL_LOCK_REHEARSAL=1 and
            honouring every test hook. For a nested compositor and for the
            measurement harness.

THERE IS NO BARE INVOCATION THAT LOCKS. `auroval-lock` with no argument
prints this and exits 2. The greeter may fall through to its real mode on a
bare run because the worst case there is a login prompt nobody asked for;
here the worst case is a locked screen, so something has to say the word.

THIS BINARY UNLOCKS A SESSION, as of 2026-11-09. The paragraph here used to
say the opposite -- `no PAM code in it and no authentication of any kind` --
which was true of the skeleton and false from the commit that added
crates/lock/src/pam.rs, while the text below went on to document
an override of the PAM service two paragraphs later. What you type is handed
to the stack in /etc/pam.d/auroval-lock, once per deliberate submission (L10).
That stack is a compiled-in constant: the override was removed on 2026-11-09
because it could point authentication at `pam_permit`.

exit codes: 0 locked and released, 1 a failure of this program,
2 a refusal to start, 3 the compositor declined the lock (L8).

`lock` AND `rehearse` BOTH LOCK YOUR SCREEN. ext-session-lock-v1 fails closed: if this
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

`lock` REFUSES TO START while any of these are set, because each makes the
screen worse in a way nobody at the keyboard can see, and the environment
here means a line in a file the session's own user writes:
  AUROVAL_LOCK_SECONDS, AUROVAL_LOCK_BREAK_GL, AUROVAL_FRAME_DUMP.
`rehearse` honours all three. That is what two modes are for: the one an idle
manager points at is not the one you measure with.

AUROVAL_CONFIG overrides /etc/auroval/greet.lua.
AUROVAL_LOCK_SECONDS=<n> makes rehearse exit after n seconds WITHOUT
  unlocking. For the measurement harness, which should not depend on anyone
  being at the keyboard. The session is left locked on purpose -- see L9.
AUROVAL_LOG_DRAWS logs one line a second: frames presented, turns of the
  loop that drew nothing, and this process's CPU as a share of one core.
  This is R7's 50%-of-one-core ceiling, asked on a lock surface.";

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("probe") => probe(),
        Some("rehearse") => rehearse(),
        Some("lock") => lock_mode(),
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
            // The pool, and which candidate the next lock would show.
            //
            // `last_shown` rather than `choose`, which is the same rule
            // `greet probe` follows and for the same reason: a diagnostic
            // that advances the thing it reports on is one you cannot run
            // twice, and running it would change what the next real lock
            // shows.
            //
            // This used to print *"this binary shows the first and never
            // rotates"*, which was true and is the kind of true that rots:
            // it stopped being so on 2026-11-10 and nothing but reading this
            // line would have said.
            let state = lock::rotation_state_for_probe();
            match cfg.background.len() {
                0 => println!("background   none configured — the solid colour is all R9 has left"),
                n => {
                    let last = auroval_ui::rotation::last_shown(&state);
                    let next = auroval_ui::rotation::next_after(last, n);
                    println!(
                        "background   {n} candidate{}, rotating per lock; state {}",
                        if n == 1 { "" } else { "s" },
                        state.display()
                    );
                    match last {
                        Some(i) => println!("  last shown   [{i}]"),
                        None => {
                            println!("  last shown   — (no state yet; the clock picks a start)")
                        }
                    }
                    for (i, chain) in cfg.background.iter().enumerate() {
                        println!(
                            "  [{i}]{}       {}",
                            if Some(i) == next { " next " } else { "      " },
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
                    pam::SERVICE,
                    u
                ),
                Err(e) => println!("unlock       NO USER — {e}"),
            }
            println!(
                "             /etc/pam.d/{} {}",
                pam::SERVICE,
                if std::path::Path::new(&format!("/etc/pam.d/{}", pam::SERVICE)).exists() {
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
/// The environment variables a locker that is really locking must refuse.
///
/// # Why a refusal and not a default
///
/// Each of these makes the locker worse in a way nobody at the keyboard can
/// see, and each is set by the environment — which on this machine means a
/// line in `~/.config/hypr/hyprland.conf`, a file the session's own user
/// writes. On 2026-11-09 an adversarial review found exactly that shape in
/// `AUROVAL_LOCK_PAM_SERVICE`: a variable that looked like a convenience and
/// was a complete authentication bypass. The answer there was to delete the
/// variable. These three cannot be deleted — two of them are how L7 and the
/// measurement harness exist at all — so the answer is that the mode which
/// really locks will not start while they are set.
///
/// `rehearse` still honours all three. That is the whole point of having two
/// modes: the one you point an idle manager at is not the one you measure
/// with.
const REFUSED_WHEN_LOCKING: [(&str, &str); 3] = [
    (
        "AUROVAL_LOCK_SECONDS",
        "exits WITHOUT unlocking, which in a real session is a locked screen \
         with no process holding it",
    ),
    (
        "AUROVAL_LOCK_BREAK_GL",
        "destroys the GL context on purpose; it is L7's test hook",
    ),
    (
        "AUROVAL_FRAME_DUMP",
        "writes PNGs of the screen, and this screen is a password field",
    ),
];

/// Which of them are set. Takes the lookup so it can be tested without
/// touching the real environment, which a test in a threaded binary must not
/// do.
///
/// L4's row says a test is what keeps `AUROVAL_FRAME_DUMP` away from a lock
/// surface, and an adversarial review found that what actually kept it away
/// was an absence: the locker never constructs a `Dump`, so nothing writes a
/// PNG and nothing checks. An absence is not a guard — it is a guard until
/// somebody adds a feature. This is the check that sentence was describing.
fn refused_hooks_present(present: impl Fn(&str) -> bool) -> Vec<&'static str> {
    REFUSED_WHEN_LOCKING
        .iter()
        .filter(|(name, _)| present(name))
        .map(|(name, _)| *name)
        .collect()
}

fn why_refused(name: &str) -> &'static str {
    REFUSED_WHEN_LOCKING
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, why)| *why)
        .unwrap_or("is a test hook")
}

/// Which of the two locking modes this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `rehearse`: for a nested compositor and the harness. Gated behind
    /// `AUROVAL_LOCK_REHEARSAL=1` and honours every test hook.
    Rehearsal,
    /// `lock`: what an idle manager or a keybind points at. No gate, and the
    /// test hooks are refused.
    Production,
}

/// `lock` — the mode that exists to be pointed at.
///
/// Deliberately a named mode rather than a bare invocation. The greeter can
/// fall through to its real mode with no arguments because the worst case
/// there is a login prompt nobody asked for; here the worst case is a locked
/// screen, so `auroval-lock` with no arguments still prints its usage and
/// exits 2. Something has to say the word.
fn lock_mode() {
    let set: Vec<&str> = refused_hooks_present(|n| std::env::var_os(n).is_some());
    if !set.is_empty() {
        eprintln!("auroval-lock: refusing to lock: the environment carries test hooks.");
        for name in &set {
            eprintln!("auroval-lock:   {name} {}", why_refused(name));
        }
        eprintln!("auroval-lock: unset them, or use `rehearse`, which honours them on purpose.");
        std::process::exit(2);
    }
    lock_the_session(Mode::Production);
}

fn rehearse() {
    if std::env::var_os("AUROVAL_LOCK_REHEARSAL").as_deref() != Some(std::ffi::OsStr::new("1")) {
        eprintln!("auroval-lock: refusing to lock without AUROVAL_LOCK_REHEARSAL=1.");
        eprintln!("auroval-lock: this mode locks the screen. The only way out is your password.");
        eprintln!("auroval-lock: `auroval-lock probe` answers the protocol question safely.");
        std::process::exit(2);
    }
    lock_the_session(Mode::Rehearsal);
}

/// Everything both locking modes do, which is almost all of it.
fn lock_the_session(mode: Mode) {
    // The refusal that matters most in this whole file.
    //
    // Without its PAM service file, `pam_start` has no stack to run and
    // nothing anybody types can succeed. Locking the screen then is not a
    // risk of a lockout, it is a lockout with extra steps -- and since L9
    // took away the escape key and the timer's unlock, the only way out
    // would be killing the compositor from another VT and losing the
    // session. So this is checked before the lock is asked for, not after.
    let service = pam::SERVICE;
    let stack = format!("/etc/pam.d/{service}");
    if !std::path::Path::new(&stack).exists() {
        eprintln!("auroval-lock: {stack} does not exist, so nothing could unlock this screen.");
        eprintln!("auroval-lock: refusing to lock. `auroval-lock probe` reports this too.");
        eprintln!(
            "auroval-lock: the file is written by pc-setup -- one line, `auth include system-auth`."
        );
        std::process::exit(2);
    }

    // Before anything can be typed, and before the renderer exists to crash.
    if let Err(e) = pam::refuse_core_dumps() {
        eprintln!("auroval-lock: could not disable core dumps ({e}).");
        eprintln!("auroval-lock:   a crash while typing could write the secret to disk.");
    }

    match mode {
        Mode::Rehearsal => {
            eprintln!("auroval-lock: REHEARSAL. This locks the session.");
            eprintln!(
                "auroval-lock: There is NO escape key. Your password unlocks it; nothing else does."
            );
            eprintln!("auroval-lock: No signal unlocks it either -- not even SIGKILL (L11).");
        }
        // Shorter, and said once. This one runs every time the screen locks,
        // so a paragraph of warnings would be noise in the journal rather
        // than a warning to anybody.
        Mode::Production => {
            eprintln!("auroval-lock: locking. Your password unlocks it; nothing else does.");
        }
    }
    let cfg = load_config();
    match lock::run(cfg) {
        Err(e) => {
            eprintln!("auroval-lock: {e}");
            std::process::exit(1);
        }
        // L8 asks that a withdrawn or refused lock be reported *and* exit
        // non-zero. It reported and exited 0 until `spike/p3/failure-round.nu`
        // asked. Its own code rather than 1, because the two mean different
        // things to anything that starts this: 1 is a locker that broke, 3 is
        // a compositor that would not have it.
        Ok(lock::Outcome::Declined) => std::process::exit(3),
        Ok(lock::Outcome::Locked) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mode that really locks will not start with a test hook set.
    ///
    /// This is the shape that bit this crate once already. On 2026-11-09 an
    /// adversarial review found `AUROVAL_LOCK_PAM_SERVICE`, a variable that
    /// read like a convenience and was a complete authentication bypass: one
    /// `env =` line in a user-writable `hyprland.conf` and the next lock
    /// opened on a keystroke. That one was deleted. These three cannot be --
    /// two of them are how L7 and the harness exist -- so the guard is that
    /// `lock` refuses while they are set, and `rehearse` still honours them.
    #[test]
    fn locking_for_real_refuses_the_test_hooks() {
        for (name, _) in REFUSED_WHEN_LOCKING {
            let found = refused_hooks_present(|n| n == name);
            assert_eq!(found, vec![name], "{name} did not stop a real lock");
        }
    }

    /// A clean environment is not a refusal.
    #[test]
    fn a_clean_environment_locks() {
        assert!(refused_hooks_present(|_| false).is_empty());
    }

    /// All of them at once are all reported, not just the first.
    ///
    /// Somebody who has one of these set probably has several, and a message
    /// naming one of three sends them round the loop twice.
    #[test]
    fn every_hook_that_is_set_is_named() {
        let found = refused_hooks_present(|_| true);
        assert_eq!(found.len(), REFUSED_WHEN_LOCKING.len());
    }

    /// Each refusal says why, because "refusing" without a reason is a
    /// message people route around rather than read.
    #[test]
    fn each_refusal_explains_itself() {
        for (name, why) in REFUSED_WHEN_LOCKING {
            assert_eq!(why_refused(name), why);
            assert!(why.len() > 20, "{name}'s reason is too short to be one");
        }
    }

    /// `AUROVAL_FRAME_DUMP` is on the list, and the reason is L4.
    ///
    /// L4's row says a test keeps the frame dump away from a lock surface.
    /// What actually kept it away was an absence -- the locker never builds a
    /// `Dump` -- and an absence holds until somebody adds a feature. Naming
    /// it here is the check that sentence was always describing.
    #[test]
    fn the_frame_dump_is_refused_rather_than_merely_absent() {
        assert!(
            REFUSED_WHEN_LOCKING
                .iter()
                .any(|(n, _)| *n == "AUROVAL_FRAME_DUMP"),
            "L4 rests on this being refused, not on nobody having wired it up"
        );
    }
}
