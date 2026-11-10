//! The `ext-session-lock-v1` surface, and the loop that draws on it.
//!
//! This is the half of a locker that is not authentication, and it is
//! deliberately the only half written. What it has to get right is smaller
//! than the greeter's equivalent and differently shaped:
//!
//! **The greeter does not use this protocol, or anything like it.**
//! `crates/greet/src/shell.rs` runs on **xdg_shell** — an ordinary
//! fullscreen `xdg_toplevel` — because it runs under `cage`, which gives it
//! the whole output and owns the keyboard on its behalf (C4b). So nothing of
//! that module's surface handling transfers, which is exactly why it stayed
//! in `greet` when the renderer moved out. What does transfer is everything
//! below the surface: `Gl::target` takes a `wl_surface` and has never cared
//! where it came from.
//!
//! **Three protocol rules this file exists to obey.** From
//! `ext-session-lock-v1.xml`, installed locally:
//!
//! 1. *"attaching/committing a buffer before the first
//!    ext_session_lock_surface_v1.configure event"* is a protocol error. So
//!    nothing is drawn until `configure` arrives, which is why `Screen` has a
//!    `size` that starts as `None` rather than a guess at the output's size.
//! 2. The configured *"width and height are in surface-local coordinates and
//!    are exact requirements. Failing to match these surface dimensions in
//!    the next commit after acking a configure is a protocol error."* The
//!    compositor's numbers win over the output's; `OutputState` is used for
//!    the scale factor and for nothing else.
//! 3. *"The client is expected to create lock surfaces for all outputs
//!    currently present and any new outputs as they are advertised."* A
//!    missed output is a monitor showing the unlocked desktop, so
//!    `new_output` is load-bearing here in a way it is not in a greeter.
//!
//! **The `locked` event is the one worth understanding**, because it is the
//! security argument in a single sentence: it *"must not be sent until a new
//! 'locked' frame has been presented on all outputs and no security sensitive
//! normal/unlocked content is possibly visible."* The compositor, not this
//! client, decides when the desktop stopped being visible. A layer-shell
//! overlay has no such event and no such guarantee — it is a window that
//! happens to be on top, and the gap between asking for it and getting it is
//! a gap in which the desktop is on screen.
//!
//! **What this module does not do.** It does not read a password, hold one,
//! or have anywhere to send one. `UiState` is constructed in the
//! `AskingUser` stage with an empty session list and left there; keys are
//! read only so that <kbd>Esc</kbd> can release the rehearsal, and every
//! other keysym is dropped on the floor without being recorded. See
//! `main.rs` for why.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopSignal};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_session_lock,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        Capability, SeatHandler, SeatState,
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

use crate::pam;
use auroval_ui::background::Background;
use auroval_ui::config::Config;
use auroval_ui::render::{Gl, Target};
use auroval_ui::ui::UiState;
use auroval_ui::view::{self, Style};

/// What `probe` found, without locking anything.
pub struct Probe {
    /// The manager global's bound version, or `None` when the compositor does
    /// not advertise it at all.
    pub manager: Option<u32>,
    pub outputs: Vec<String>,
    pub font: String,
}

/// Bind the globals, report, and lock nothing.
///
/// `SessionLockState::new` binds the manager if it is there and records a
/// `GlobalProxy` error if it is not — binding the manager is not locking, so
/// this is safe on a live desktop. `lock()` is the request that locks, and it
/// is not called from here.
pub fn probe() -> Result<Probe, String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("no Wayland display: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init::<Lock>(&conn).map_err(|e| format!("registry: {e}"))?;
    let qh = queue.handle();

    // The manager's version comes off the registry listing rather than from
    // the bound proxy, because an absent global has no proxy to ask and the
    // distinction between "absent" and "present at a version we cannot use"
    // is the answer this mode exists to give.
    let manager = globals
        .contents()
        .clone_list()
        .into_iter()
        .find(|g| g.interface == "ext_session_lock_manager_v1")
        .map(|g| g.version);

    let output_state = OutputState::new(&globals, &qh);
    let mut probing = Lock::probing(
        RegistryState::new(&globals),
        output_state,
        SeatState::new(&globals, &qh),
    );
    // One round trip, so the outputs the registry announced have their
    // geometry before they are listed.
    queue
        .roundtrip(&mut probing)
        .map_err(|e| format!("roundtrip: {e}"))?;

    let outputs = probing
        .output_state
        .outputs()
        .map(|o| {
            let info = probing.output_state.info(&o);
            match info {
                Some(i) => {
                    let (w, h) = i
                        .modes
                        .iter()
                        .find(|m| m.current)
                        .map(|m| m.dimensions)
                        .unwrap_or((0, 0));
                    format!(
                        "{:<12} {}x{} scale {}  {}",
                        i.name.unwrap_or_else(|| "?".into()),
                        w,
                        h,
                        i.scale_factor,
                        i.description.unwrap_or_default()
                    )
                }
                None => "? (no output info)".to_string(),
            }
        })
        .collect();

    // The font is asked for here rather than in `rehearse` because R11 names
    // MesloLGS and a silent substitution is the failure mode: a lock screen
    // in the wrong face is still a lock screen, which is how it would ship.
    // A headless context is enough to answer it and needs no compositor.
    let font = match unsafe { Gl::headless(16, 16) } {
        Ok((gl, _)) => gl.atlas.source.clone(),
        Err(e) => format!("— (no off-screen GL context: {e})"),
    };

    Ok(Probe {
        manager,
        outputs,
        font,
    })
}

/// One output's lock surface and the EGL target on it.
struct Screen {
    /// Held for as long as the lock is up: dropping it destroys the lock
    /// surface, which on an output that still exists means a monitor the
    /// compositor has nothing to show on.
    _surface: SessionLockSurface,
    wl_surface: wl_surface::WlSurface,
    target: Option<Target>,
    scale: i32,
    /// The compositor's exact requirement, or `None` before the first
    /// `configure`. Nothing may be committed while this is `None` (rule 1 in
    /// this module's doc comment).
    size: Option<(u32, u32)>,
    /// `view::signature` of the last frame drawn here. `None` forces a frame.
    last: Option<String>,
}
/// What the closing summary says about authentication, from what happened.
///
/// A free function so the four cases can be asserted without a compositor, a
/// PAM stack or a password. The case that matters is `(true, n)`: the line it
/// replaced said no unlock was possible, and printed that two lines after the
/// run had unlocked.
/// What the closing summary says about R7's ceiling.
///
/// A free function, and conditional on a frame, because the run that found
/// this printed `cpu 130.3% of one core` and `OVER BUDGET` for a run that
/// was declined the lock and never presented anything at all: 0.03 s of
/// startup over a wall clock that had barely started. A budget verdict on a
/// run with no frames in it is noise that reads like a finding.
fn r7_verdict(presented: bool, pct: f64) -> String {
    if !presented {
        return "no verdict — nothing was presented".into();
    }
    if pct <= 50.0 {
        "50% of one core — within budget".into()
    } else {
        "50% of one core — OVER BUDGET".into()
    }
}

fn unlock_summary(accepted: bool, denials: usize) -> String {
    match (accepted, denials) {
        (true, 0) => "authenticated and unlocked".into(),
        (true, n) => format!("authenticated and unlocked after {n} denied"),
        (false, 0) => "never attempted — this run ended without a submission".into(),
        (false, n) => format!("still locked — {n} denied, none accepted"),
    }
}

/// Whether a Return should become a `pam_authenticate` call.
///
/// A free function because L10 is a requirement with a test attached, and a
/// `Lock` cannot be built without a Wayland display. The two reasons to
/// refuse are the whole of L10:
///
/// * an attempt is already in flight — a held Return, a burst of synthetic
///   events, or a second press during the two-second failure delay
/// * the field is empty — not an attempt anybody made, and it would still
///   spend one of `pam_faillock`'s three
fn may_submit(auth: &Auth, secret_empty: bool) -> bool {
    matches!(auth, Auth::Typing) && !secret_empty
}

/// What the field is doing. Two states, and the second one is L10.
///
/// L10: *"Never submit an authentication attempt the user did not ask for,
/// and never retry one automatically."* `InFlight` is how that is kept: while
/// a `pam_authenticate` is running nothing can start another, so a held
/// Return, a burst of synthetic events and a failure followed by a redraw all
/// produce exactly one call. The counter in `pam_faillock` is at three and
/// `/etc/security/faillock.conf` is commented out, so an automatic retry
/// empties it in under a second and shuts the owner out for ten minutes.
#[derive(Debug, PartialEq, Eq)]
enum Auth {
    /// Accepting keystrokes.
    Typing,
    /// A worker thread is inside `pam_authenticate`. No submission is
    /// accepted and no key is stored until it answers.
    InFlight,
}

pub struct Lock {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    compositor: Option<CompositorState>,
    session_lock_state: Option<SessionLockState>,
    qh: Option<QueueHandle<Lock>>,
    signal: Option<LoopSignal>,
    keyboard: Option<wl_keyboard::WlKeyboard>,

    /// `Some` once the compositor has sent `locked`. Until then there is
    /// nothing to draw on and no lock to release.
    session_lock: Option<SessionLock>,
    /// The lock object between `lock()` and the `locked` event.
    ///
    /// It is held rather than dropped because dropping it sends `destroy`,
    /// which withdraws the request — and it is kept out of `session_lock`
    /// because `unlock_and_destroy` before the `locked` event is a protocol
    /// error. The two fields are the two legal states, which is cheaper to
    /// read than one field and a boolean.
    pending: Option<SessionLock>,
    screens: HashMap<u32, Screen>,
    gl: Option<Gl>,
    background: Option<Background>,
    style: Style,
    ui: UiState,
    locale: chrono::Locale,

    /// Set when the lock has been released and the loop should stop. Kept
    /// separate from `session_lock` being `None` so that a `finished` event —
    /// the compositor refusing or withdrawing the lock — ends the loop
    /// without the code having to pretend it ever held one.
    done: bool,
    /// True once `unlock` has been called, so it is not called twice: the
    /// protocol makes a second `unlock_and_destroy` an error.
    released: bool,
    /// True once `finished` arrived: the compositor declined the lock. Read
    /// by `run` to end with `Outcome::Declined`, so the exit code says what
    /// the two printed lines already said.
    declined: bool,

    /// How many submissions PAM denied, and whether one was ever accepted.
    ///
    /// These exist because the closing summary used to end with a line
    /// claiming `unlock NONE — nothing here can authenticate anybody`, which
    /// was true of the skeleton and became a lie the moment PAM landed: the
    /// first real rehearsal printed it two lines after `authenticated;
    /// unlocking`. A report that contradicts the run it is reporting on is
    /// worse than no report, so the summary now reads these rather than a
    /// constant.
    denials: usize,
    accepted: bool,

    /// The PAM service name. `auroval-lock`, written by pc-setup, which is
    /// `auth include system-auth` and deliberately has no `account` stanza --
    /// see `pam.rs` for why nothing here calls `pam_acct_mgmt`.
    service: String,
    /// The one user this locker will authenticate, from `getpwuid(getuid())`.
    user: String,
    /// What the field is doing. See [`Auth`].
    auth: Auth,
    /// The typed secret, in the only buffer this process owns. It is never
    /// copied into `ui`, which gets a placeholder per keystroke so the dot
    /// count is right and nothing else.
    secret: pam::Secret,
    /// Results from the worker thread. Polled with `try_recv` on the draw
    /// loop's own 16 ms turn rather than through a calloop source, because
    /// the loop already wakes that often and a channel that needs no waking
    /// is one fewer thing to get wrong on this path.
    auth_rx: std::sync::mpsc::Receiver<pam::Outcome>,
    auth_tx: std::sync::mpsc::Sender<pam::Outcome>,

    log_draws: bool,
    presents: u32,
    turns: u32,
    reported: Instant,
    /// `own_cpu_seconds()` as of the last report, so the per-second line can
    /// state the *interval's* cost. The first version of this printed CPU
    /// since process start divided by wall since process start, which is a
    /// cumulative average dressed up as a rate: it read 15% while the
    /// instantaneous cost was over 30%, because the four seconds of waiting
    /// for `locked` were averaged into every later line. A receipt that
    /// understates by half is worse than no receipt.
    cpu_at_report: f64,
    started: Instant,
    first_frame: Option<Instant>,
    deadline: Option<Instant>,
    cpu_at_start: f64,
}

impl Lock {
    fn probing(
        registry_state: RegistryState,
        output_state: OutputState,
        seat_state: SeatState,
    ) -> Lock {
        Lock {
            registry_state,
            output_state,
            seat_state,
            compositor: None,
            session_lock_state: None,
            qh: None,
            signal: None,
            keyboard: None,
            session_lock: None,
            pending: None,
            screens: HashMap::new(),
            gl: None,
            background: None,
            style: Style {
                geometry: Default::default(),
                palette: Default::default(),
            },
            ui: UiState::new(Vec::new(), &auroval_ui::store::LastLogin::default()),
            locale: auroval_ui::clock::locale_from_env(),
            done: false,
            released: false,
            declined: false,
            denials: 0,
            accepted: false,
            // probe locks nothing, so these are inert by construction: it
            // never reaches `submit`, and `release` is unreachable without a
            // `session_lock`. The service and user are still the real ones so
            // that `probe` can report them -- which is the mode's whole job.
            service: String::new(),
            user: String::new(),
            auth: Auth::Typing,
            secret: pam::Secret::new(),
            auth_rx: {
                let (_t, r) = std::sync::mpsc::channel();
                r
            },
            auth_tx: std::sync::mpsc::channel().0,

            log_draws: false,
            presents: 0,
            turns: 0,
            reported: Instant::now(),
            cpu_at_report: 0.0,
            started: Instant::now(),
            first_frame: None,
            deadline: None,
            cpu_at_start: 0.0,
        }
    }
}

/// This process's own CPU time so far, user plus system, in seconds.
///
/// Lifted from `greet`'s `videoprobe` for the same reason it is there: R7's
/// budget is about *this* process, and spawning `ps` to ask would add its
/// own. It is duplicated rather than shared because it is eight lines of
/// `/proc` parsing that belongs to a measurement harness, and putting it in
/// `auroval-ui` would mean the library carried a function neither front end
/// uses in earnest.
fn own_cpu_seconds() -> f64 {
    let Ok(stat) = std::fs::read_to_string("/proc/self/stat") else {
        return 0.0;
    };
    let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else {
        return 0.0;
    };
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (ticks(11) + ticks(12)) / 100.0
}

/// Lock the session and draw on it until something releases the lock.
/// How a run ended, for callers that must exit differently.
///
/// Added because a run that locked nothing exited 0. L8 asks that a
/// `finished` be *"reported, and exits non-zero"*, and the report was there
/// while the exit code said the run had gone fine -- which is the same class
/// of mistake as a summary that denies its own unlock, one layer further
/// out: `spike/p3/failure-round.nu` asserts it now, and the assertion failed
/// the first time it was asked.
pub enum Outcome {
    /// The compositor locked the session and this process held it.
    Locked,
    /// `finished`: the compositor declined or withdrew the lock. Nothing was
    /// ever locked and nothing was drawn.
    Declined,
}

pub fn run(cfg: Config) -> Result<Outcome, String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("no Wayland display: {e}"))?;
    let (globals, queue) =
        registry_queue_init::<Lock>(&conn).map_err(|e| format!("registry: {e}"))?;
    let qh = queue.handle();
    let mut loop_: EventLoop<Lock> = EventLoop::try_new().map_err(|e| e.to_string())?;

    let compositor = CompositorState::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let session_lock_state = SessionLockState::new(&globals, &qh);

    let gl = unsafe { Gl::new(conn.backend().display_ptr() as *mut _) }?;
    eprintln!("auroval-lock: font {}", gl.atlas.source);
    eprintln!(
        "auroval-lock: {}",
        auroval_ui::clock::describe(auroval_ui::clock::locale_from_env())
    );

    // R9's chain, started before the lock is asked for so the first locked
    // frame has something on it. The solid colour shows until the decode
    // lands, exactly as in the greeter.
    //
    // THE FIRST CANDIDATE, NOT A ROTATED ONE. `background` is a pool of
    // chains since #66, and `greet` picks between them with `rotation`, which
    // remembers its place in `/var/lib/auroval/last-background`. That
    // directory is `drwxr-x--- greeter greeter`: a locker runs as the
    // logged-in user and cannot read it, never mind advance it. So the pool's
    // first candidate is what a locker shows, every time, and the two screens
    // agree only when the greeter happens to be on candidate 0. Whether a
    // locker should rotate at all, and against what state, is N1 in the PRD.
    let chain: &[std::path::PathBuf] = cfg.background.first().map_or(&[], |c| c.as_slice());
    let background = Background::start(chain);
    for p in background.problems() {
        eprintln!("auroval-lock: {p}");
    }

    // L3: the one user this locker will ever authenticate, from the uid and
    // nowhere else. Fatal if it cannot be determined -- a locker that does
    // not know whose screen it is holding has no business holding it.
    let user = pam::own_username()?;
    let service = pam::SERVICE.to_string();
    eprintln!("auroval-lock: pam service {service}, user {user}");
    let (auth_tx, auth_rx) = std::sync::mpsc::channel();

    let deadline = std::env::var_os("AUROVAL_LOCK_SECONDS")
        .and_then(|v| v.to_string_lossy().parse::<u64>().ok())
        .map(|n| Instant::now() + Duration::from_secs(n));

    let mut lock = Lock {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        seat_state: SeatState::new(&globals, &qh),
        compositor: Some(compositor),
        session_lock_state: Some(session_lock_state),
        qh: Some(qh.clone()),
        signal: Some(loop_.get_signal()),
        keyboard: None,
        session_lock: None,
        pending: None,
        screens: HashMap::new(),
        gl: Some(gl),
        background: Some(background),
        style: Style {
            geometry: cfg.geometry,
            palette: cfg.palette,
        },
        // An empty session list, which is the shape of the wart recorded in
        // `auroval-ui`'s doc comment: `UiState` carries a session list because
        // the greeter preselects one, and a locker has nothing to put in it.
        ui: UiState::new(Vec::new(), &auroval_ui::store::LastLogin::default()),
        locale: auroval_ui::clock::locale_from_env(),
        done: false,
        released: false,
        declined: false,
        denials: 0,
        accepted: false,
        service: service.clone(),
        user: user.clone(),
        auth: Auth::Typing,
        secret: pam::Secret::new(),
        auth_rx,
        auth_tx,

        log_draws: std::env::var_os("AUROVAL_LOG_DRAWS").is_some(),
        presents: 0,
        turns: 0,
        reported: Instant::now(),
        cpu_at_report: own_cpu_seconds(),
        started: Instant::now(),
        first_frame: None,
        deadline,
        cpu_at_start: own_cpu_seconds(),
    };

    WaylandSource::new(conn.clone(), queue)
        .insert(loop_.handle())
        .map_err(|e| e.to_string())?;

    // The request that locks the screen. Everything above this line is safe;
    // nothing below it is.
    let held = lock
        .session_lock_state
        .as_ref()
        .expect("set above")
        .lock(&qh)
        .map_err(|e| {
            format!("ext_session_lock_manager_v1: {e} — this compositor will not let a client lock")
        })?;
    // Not stored on `lock` yet: the protocol says surfaces created before the
    // `locked` event "won't be displayed", and `SessionLock::unlock` is only
    // legal after it. `locked()` below is what promotes this to held state.
    lock.pending = Some(held);
    eprintln!("auroval-lock: lock requested; waiting for the compositor's `locked`");

    while !lock.done {
        loop_
            .dispatch(Duration::from_millis(16), &mut lock)
            .map_err(|e| e.to_string())?;
        lock.poll_auth();
        lock.tick_clock();
        lock.draw_all();
        lock.report();
        if lock.deadline.is_some_and(|d| Instant::now() >= d) {
            // **Exits without unlocking**, and the difference is L9. This
            // deadline used to call `release`, which was an unconditional
            // unlock on a timer. It now ends the process and leaves the
            // session locked, which is what L11 asks of every other way of
            // dying.
            //
            // The measurement harness still works: `lock-measure.nu` passes
            // this to bound a run and waits for the process to exit, which it
            // still does. What changes is the operator's job -- the nested
            // compositor is left locked and has to be closed. On the session
            // you are sitting in there is no way out but the right password,
            // which is the point.
            eprintln!(
                "auroval-lock: AUROVAL_LOCK_SECONDS elapsed; exiting WITHOUT unlocking. \
                 The session stays locked -- close the nested compositor."
            );
            lock.done = true;
        }
    }

    // A correct client "that wishes to exit directly after unlocking the
    // session must use the wl_display.sync request to ensure the server
    // receives and processes the unlock_and_destroy request. Otherwise there
    // is no guarantee that the server has unlocked the session." Skipping
    // this is how an unlock that was requested still leaves a locked screen.
    let _ = conn.roundtrip();
    lock.summarise();
    if lock.declined {
        return Ok(Outcome::Declined);
    }
    Ok(Outcome::Locked)
}

impl Lock {
    /// Hand the typed secret to PAM, on a thread, once.
    ///
    /// **Off the draw loop, and that is a requirement rather than a
    /// courtesy.** `pam_unix(8)` documents a *"delay-on-failure of the order
    /// of two seconds"* and `/etc/pam.d/system-auth` does not pass
    /// `nodelay`, so a wrong password blocks for seconds. On this thread that
    /// is a lock screen frozen mid-animation at the moment somebody is
    /// wondering whether they mistyped. L7 called it: *"if the PAM
    /// conversation shares a thread with the renderer, a render path that
    /// cannot recover takes the unlock with it."*
    ///
    /// The secret is **moved** into the thread, so it is zeroed by its own
    /// `Drop` when the closure ends, whatever PAM returned.
    fn submit(&mut self) {
        if !may_submit(&self.auth, self.secret.is_empty()) {
            return;
        }
        let mut secret = std::mem::replace(&mut self.secret, pam::Secret::new());
        // `clear_field`, not `typed.clear()`. This locker's stage never
        // leaves `AskingUser`, so `UiState::push` has been filling
        // `username` -- and clearing `typed` cleared nothing. Every denial
        // left the previous attempt's placeholders on screen and the field
        // grew with each try.
        self.ui.clear_field();
        self.ui.notice = None;
        self.auth = Auth::InFlight;

        let (service, user, tx) = (
            self.service.clone(),
            self.user.clone(),
            self.auth_tx.clone(),
        );
        std::thread::spawn(move || {
            let outcome = pam::authenticate(&service, &user, secret.as_str());
            secret.clear();
            // A send that fails means the loop is already gone, which is the
            // ordinary shutdown path and not worth reporting.
            let _ = tx.send(outcome);
        });
    }

    /// Take whatever the worker said, if it has said anything.
    ///
    /// Nothing here retries. A denial puts the field back in the user's hands
    /// and waits, which is the other half of L10.
    fn poll_auth(&mut self) {
        let Ok(outcome) = self.auth_rx.try_recv() else {
            return;
        };
        self.auth = Auth::Typing;
        match outcome {
            pam::Outcome::Success { info } => {
                for i in &info {
                    eprintln!("auroval-lock: pam: {i}");
                }
                eprintln!("auroval-lock: authenticated; unlocking");
                self.accepted = true;
                self.release();
            }
            pam::Outcome::Denied { reason, info } => {
                for i in &info {
                    eprintln!("auroval-lock: pam: {i}");
                }
                eprintln!("auroval-lock: denied: {reason}");
                self.denials += 1;
                self.ui.notice = Some(auroval_ui::ui::Notice {
                    text: reason,
                    kind: auroval_ui::ui::NoticeKind::Error,
                });
            }
            pam::Outcome::Error { reason } => {
                // Not a wrong password. Said differently on purpose: a user
                // who retypes a correct password against a broken stack
                // spends faillock's three attempts on nothing.
                eprintln!("auroval-lock: PAM error (not a denial): {reason}");
                self.ui.notice = Some(auroval_ui::ui::Notice {
                    text: format!("authentication is unavailable: {reason}"),
                    kind: auroval_ui::ui::NoticeKind::Error,
                });
            }
        }
    }

    /// Release the lock, once, and stop the loop.
    ///
    /// **One caller: `poll_auth`'s success branch.** That is L9 — a shipped
    /// build contains no path from a keysym, a timer or a signal to this
    /// function that does not pass through a PAM success. Escape no longer
    /// calls it, the `AUROVAL_LOCK_SECONDS` deadline no longer calls it, and
    /// no signal is handled at all, so `SIGTERM` and `SIGKILL` both leave the
    /// session locked (L11).
    fn release(&mut self) {
        if let Some(l) = &self.session_lock {
            if !self.released {
                l.unlock();
                self.released = true;
            }
        }
        self.done = true;
        if let Some(s) = &self.signal {
            s.stop();
        }
    }

    fn tick_clock(&mut self) {
        let (c, d) = auroval_ui::clock::lines(chrono::Local::now(), self.locale);
        self.ui.clock = c;
        self.ui.date = d;
    }

    fn add_output(&mut self, output: wl_output::WlOutput) {
        let id = output.id().protocol_id();
        if self.screens.contains_key(&id) {
            return;
        }
        let (Some(lock), Some(compositor), Some(qh)) =
            (&self.session_lock, &self.compositor, &self.qh)
        else {
            // Before `locked`, there is no lock object to hang a surface off.
            // `locked()` walks the outputs again, so nothing is lost.
            return;
        };
        let info = self.output_state.info(&output);
        let scale = info.as_ref().map(|i| i.scale_factor).unwrap_or(1);
        let surface = compositor.create_surface(qh);
        let wl_surface = surface.clone();
        let lock_surface = lock.create_lock_surface(surface, &output, qh);
        self.screens.insert(
            id,
            Screen {
                _surface: lock_surface,
                wl_surface,
                target: None,
                scale,
                // Rule 1: nothing may be committed until `configure` says how
                // big. That is what this `None` enforces.
                size: None,
                last: None,
            },
        );
    }

    /// Draw every surface whose state has changed, and no others.
    fn draw_all(&mut self) {
        self.turns += 1;
        let Some(gl) = self.gl.as_mut() else { return };
        let ready = self.background.as_mut().and_then(|b| b.ready(gl));
        let generation = self
            .background
            .as_ref()
            .map(|b| b.generation())
            .unwrap_or(0);

        for screen in self.screens.values_mut() {
            let Some((w, h)) = screen.size else { continue };
            if screen.target.is_none() {
                match unsafe { gl.target(&screen.wl_surface, w, h, screen.scale) } {
                    Ok(t) => screen.target = Some(t),
                    Err(e) => {
                        eprintln!("auroval-lock: no EGL surface for this output: {e}");
                        continue;
                    }
                }
            }
            let Some(target) = screen.target.as_mut() else {
                continue;
            };
            if (target.width, target.height) != (w, h) {
                target.resize(w, h, screen.scale);
                screen.last = None;
            }
            let sig = view::signature(&self.ui, (w, h), generation);
            if screen.last.as_deref() == Some(sig.as_str()) {
                continue;
            }
            if gl.make_current(target).is_err() {
                continue;
            }
            view::draw(gl, &self.ui, &self.style, ready);
            gl.present(target);
            screen.last = Some(sig);
            self.presents += 1;
            if self.first_frame.is_none() {
                self.first_frame = Some(Instant::now());
                eprintln!(
                    "auroval-lock: first locked frame at {} ms",
                    self.started.elapsed().as_millis()
                );
            }
        }
    }

    /// One line a second, when asked. R7's ceiling is 50% of one core for the
    /// whole present path, so the number that matters is this process's CPU
    /// against wall clock — not a frame rate, which a lock screen showing a
    /// video holds trivially by dropping work nobody can see.
    fn report(&mut self) {
        if !self.log_draws || self.reported.elapsed() < Duration::from_secs(1) {
            return;
        }
        let interval = self.reported.elapsed().as_secs_f64();
        let now_cpu = own_cpu_seconds();
        let cpu = now_cpu - self.cpu_at_report;
        eprintln!(
            "auroval-lock: draws {} of {} turns in {:.2} s, cpu {:.1}% of one core \
             (since start {:.1}%)",
            self.presents,
            self.turns,
            interval,
            100.0 * cpu / interval.max(0.001),
            100.0 * (now_cpu - self.cpu_at_start) / self.started.elapsed().as_secs_f64().max(0.001),
        );
        self.presents = 0;
        self.turns = 0;
        self.reported = Instant::now();
        self.cpu_at_report = now_cpu;
    }

    /// What the run cost, printed once at the end so a rehearsal leaves a
    /// receipt whether or not `AUROVAL_LOG_DRAWS` was set.
    fn summarise(&self) {
        let wall = self.started.elapsed().as_secs_f64();
        let cpu = own_cpu_seconds() - self.cpu_at_start;
        eprintln!();
        eprintln!(
            "auroval-lock: surfaces     {} at {}",
            self.screens.len(),
            self.screens
                .values()
                .map(|s| match s.size {
                    Some((w, h)) => format!("{w}x{h} scale {}", s.scale),
                    None => "never configured".into(),
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
        eprintln!(
            "auroval-lock: first frame   {}",
            match self.first_frame {
                Some(t) => format!("{} ms after start", (t - self.started).as_millis()),
                None => "never — nothing was ever presented".into(),
            }
        );
        if let Some(b) = &self.background {
            let n = b.negotiated_gl_memory();
            eprintln!(
                "auroval-lock: memory       {}",
                match n {
                    Some(true) => "GLMemory — the frame was already a texture here",
                    Some(false) if self.first_frame.is_some() =>
                        "NOT GLMemory — copied through system memory; R7 is at risk",
                    Some(false) => "NOT GLMemory — and nothing was presented through it",
                    None => "— (no video in the chain)",
                }
            );
            for p in b.problems() {
                eprintln!("auroval-lock: problem      {p}");
            }
        }
        eprintln!(
            "auroval-lock: cpu          {:.1}% of one core ({:.2} s of cpu in {:.1} s wall)",
            100.0 * cpu / wall.max(0.001),
            cpu,
            wall
        );
        eprintln!(
            "auroval-lock: R7 ceiling   {}",
            r7_verdict(self.first_frame.is_some(), 100.0 * cpu / wall.max(0.001))
        );
        eprintln!(
            "auroval-lock: unlock       {}",
            unlock_summary(self.accepted, self.denials)
        );
    }
}

impl SessionLockHandler for Lock {
    /// The compositor has locked the session. From here the desktop is not
    /// visible, by the protocol's own guarantee, and this client owns the
    /// screen until it says otherwise.
    fn locked(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, session_lock: SessionLock) {
        eprintln!(
            "auroval-lock: locked at {} ms — the compositor has hidden the desktop",
            self.started.elapsed().as_millis()
        );
        // Both fields are clones of one `Arc`, so dropping the pending clone
        // here does not reach `SessionLockInner::drop` and does not send
        // `destroy`. It only moves this object into its second legal state.
        self.pending = None;
        self.session_lock = Some(session_lock);
        // Rule 3: every output that exists now, plus every one announced
        // later by `new_output`. A missed output is a monitor still showing
        // the desktop.
        let outputs: Vec<_> = self.output_state.outputs().collect();
        for o in outputs {
            self.add_output(o);
        }
    }

    /// The compositor will not lock, or has withdrawn the lock. Either way
    /// there is nothing to draw on and nothing to release.
    fn finished(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _lock: SessionLock) {
        eprintln!("auroval-lock: `finished` — the compositor declined or withdrew the lock.");
        eprintln!("auroval-lock:   another client may already hold one. Nothing was locked.");
        self.declined = true;
        // Not `release()`: calling `unlock_and_destroy` without having had
        // `locked` is a protocol error. Dropping the last clone reaches
        // sctk's `Drop`, which sends `destroy` — the correct request here,
        // and the one its own comment describes as failing secure.
        self.pending = None;
        self.session_lock = None;
        self.released = true;
        self.done = true;
        if let Some(s) = &self.signal {
            s.stop();
        }
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _serial: u32,
    ) {
        // sctk has already acked the serial. Rule 2: these numbers are exact
        // requirements and the next commit must match them, so they are
        // recorded rather than compared against the output's mode.
        let (w, h) = configure.new_size;
        let id = surface.wl_surface().id().protocol_id();
        for screen in self.screens.values_mut() {
            if screen.wl_surface.id().protocol_id() == id {
                if screen.size != Some((w, h)) {
                    screen.size = Some((w, h));
                    screen.last = None;
                }
                return;
            }
        }
    }
}

impl KeyboardHandler for Lock {
    /// The field.
    ///
    /// **Escape does nothing, and that absence is L9.** It used to release
    /// the lock, unconditionally and without authenticating anybody, because
    /// a skeleton that leaves the session locked after every run can be
    /// tested once per boot. The rehearsal is nested now -- the compositor
    /// being locked is a window to close -- so the hatch has no excuse left,
    /// and L9 required it gone in the same commit that introduced PAM.
    ///
    /// Nothing is submitted that the user did not ask for (L10): a secret
    /// leaves this function only on Return, and only when no attempt is
    /// already in flight.
    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _kbd: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // While PAM is thinking, every key is dropped -- not buffered for
        // later. Buffering would mean a keystroke typed during the two-second
        // failure delay lands in the *next* attempt, which is an attempt the
        // user did not compose.
        if self.auth == Auth::InFlight {
            return;
        }
        match event.keysym {
            Keysym::Return | Keysym::KP_Enter => self.submit(),
            Keysym::BackSpace => {
                self.secret.pop();
                self.ui.backspace();
            }
            Keysym::Escape => {}
            _ => {
                // `utf8` is what the key produced after the layout and the
                // modifiers, which is the only thing a password field can
                // use. Control characters are not text.
                if let Some(text) = event.utf8 {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        self.secret.push(c);
                        // A placeholder, so `field_display` counts the right
                        // number of dots and `ui` never holds the secret.
                        self.ui.push('\u{2022}');
                    }
                }
            }
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    /// A held key repeats into the field, which is what a field should do --
    /// and a held Return cannot submit twice, because the first press leaves
    /// `Auth::InFlight` and `press_key` drops everything until PAM answers.
    /// That is L10 holding without a special case for repeats.
    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kbd: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(conn, qh, kbd, serial, event);
    }

    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    /// Caps lock is R6's, and R6 is the greeter's. A locker wants it too, but
    /// it is drawn from `UiState` and `UiState` is not being driven here.
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}

impl SeatHandler for Lock {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            // Taken only so Esc can be heard. A locker with no keyboard at
            // all would be unreleasable by anything short of SIGKILL, which
            // is a worse rehearsal than one with a documented escape.
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => eprintln!("auroval-lock: no keyboard: {e} — Esc will not work"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl OutputHandler for Lock {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    /// Rule 3. An output that appears while the session is locked needs a
    /// surface, or it shows whatever the compositor has for it.
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.add_output(output);
    }

    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let id = output.id().protocol_id();
        let scale = self
            .output_state
            .info(&output)
            .map(|i| i.scale_factor)
            .unwrap_or(1);
        if let Some(screen) = self.screens.get_mut(&id) {
            if screen.scale != scale {
                screen.scale = scale;
                screen.last = None;
            }
        }
    }

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.screens.remove(&output.id().protocol_id());
    }
}

impl CompositorHandler for Lock {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        let id = surface.id().protocol_id();
        for screen in self.screens.values_mut() {
            if screen.wl_surface.id().protocol_id() == id {
                screen.scale = new_factor;
                screen.last = None;
            }
        }
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wayland_client::protocol::wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl ProvidesRegistryState for Lock {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Lock);
delegate_output!(Lock);
delegate_seat!(Lock);
delegate_keyboard!(Lock);
delegate_registry!(Lock);
delegate_session_lock!(Lock);

#[cfg(test)]
mod tests {
    use super::*;

    /// L10, as far as it can be reached without a compositor: at most one
    /// attempt per deliberate submission, and none at all from an empty
    /// field.
    #[test]
    fn only_a_deliberate_nonempty_submission_reaches_pam() {
        assert!(
            may_submit(&Auth::Typing, false),
            "a typed secret and Return"
        );
        assert!(
            !may_submit(&Auth::Typing, true),
            "an empty field is not an attempt"
        );
        assert!(
            !may_submit(&Auth::InFlight, false),
            "a second Return while PAM is thinking must not reach it"
        );
        assert!(!may_submit(&Auth::InFlight, true));
    }

    /// The shape of the burst L10 names: a thousand Returns against a field
    /// that is answered once. Only the first can pass, because the first is
    /// what moves the state.
    #[test]
    fn a_burst_of_returns_produces_one_attempt() {
        let mut auth = Auth::Typing;
        let mut attempts = 0;
        for _ in 0..1000 {
            if may_submit(&auth, false) {
                attempts += 1;
                auth = Auth::InFlight;
            }
        }
        assert_eq!(attempts, 1);
    }

    /// The regression `r7_verdict` exists for: a run the compositor had
    /// declined reported `cpu 130.3% of one core` and `OVER BUDGET`, from
    /// 0.03 s of startup over a wall clock that had barely begun. It had
    /// presented no frames at all, so there was no present path to be over
    /// anything.
    #[test]
    fn a_run_with_no_frames_gets_no_budget_verdict() {
        assert_eq!(
            r7_verdict(false, 130.3),
            "no verdict — nothing was presented"
        );
        assert_eq!(r7_verdict(false, 0.1), "no verdict — nothing was presented");
    }

    #[test]
    fn a_run_that_drew_is_judged_against_the_ceiling() {
        assert!(r7_verdict(true, 17.3).ends_with("within budget"));
        assert!(r7_verdict(true, 50.0).ends_with("within budget"));
        assert!(r7_verdict(true, 50.1).ends_with("OVER BUDGET"));
    }

    /// The regression this function exists for: the first real rehearsal, on
    /// 2026-11-09, denied one password and accepted the next, and then printed
    /// `unlock NONE — nothing here can authenticate anybody` as its closing
    /// line. Whatever the summary says, it may not contradict the run.
    #[test]
    fn the_summary_does_not_deny_an_unlock_that_happened() {
        assert_eq!(unlock_summary(true, 0), "authenticated and unlocked");
        assert_eq!(
            unlock_summary(true, 1),
            "authenticated and unlocked after 1 denied"
        );
        for denials in 0..4 {
            assert!(
                !unlock_summary(true, denials).contains("NONE"),
                "a run that unlocked must not be reported as unable to"
            );
        }
    }

    /// And the other side of it: a run that never unlocked must not read as
    /// though it did. The locker is left holding the screen in that case,
    /// which is the state the operator has to be told about.
    #[test]
    fn the_summary_does_not_claim_an_unlock_that_did_not_happen() {
        assert_eq!(
            unlock_summary(false, 0),
            "never attempted — this run ended without a submission"
        );
        assert_eq!(
            unlock_summary(false, 2),
            "still locked — 2 denied, none accepted"
        );
        for denials in 0..4 {
            assert!(!unlock_summary(false, denials).contains("unlocked"));
        }
    }
}
