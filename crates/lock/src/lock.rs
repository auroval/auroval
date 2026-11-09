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
pub fn run(cfg: Config) -> Result<(), String> {
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
        lock.tick_clock();
        lock.draw_all();
        lock.report();
        if lock.deadline.is_some_and(|d| Instant::now() >= d) {
            eprintln!("auroval-lock: AUROVAL_LOCK_SECONDS elapsed; releasing the lock");
            lock.release();
        }
    }

    // A correct client "that wishes to exit directly after unlocking the
    // session must use the wl_display.sync request to ensure the server
    // receives and processes the unlock_and_destroy request. Otherwise there
    // is no guarantee that the server has unlocked the session." Skipping
    // this is how an unlock that was requested still leaves a locked screen.
    let _ = conn.roundtrip();
    lock.summarise();
    Ok(())
}

impl Lock {
    /// Release the lock, once, and stop the loop.
    ///
    /// This is the rehearsal harness's unconditional unlock and it is the one
    /// thing in this crate that a real locker must not have. It is called
    /// from Esc, from the `AUROVAL_LOCK_SECONDS` deadline, and from the
    /// signal handler.
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
                    Some(false) => "NOT GLMemory — copied through system memory; R7 is at risk",
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
            "auroval-lock: R7 ceiling   50% of one core — {}",
            if 100.0 * cpu / wall.max(0.001) <= 50.0 {
                "within budget"
            } else {
                "OVER BUDGET"
            }
        );
        eprintln!("auroval-lock: unlock       NONE — nothing here can authenticate anybody");
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
    /// The rehearsal's escape hatch, and nothing else.
    ///
    /// Every keysym other than Escape is ignored *without being stored*. A
    /// real locker accumulates a secret here; this one must be checkable by
    /// reading it, so there is no buffer to accumulate into.
    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _kbd: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if event.keysym == Keysym::Escape {
            eprintln!("auroval-lock: Esc — releasing the lock. A real locker would not do this.");
            self.release();
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

    /// Held Esc releases too, which costs nothing: `release` is idempotent.
    /// A real locker's repeat handling belongs with its field, and there is
    /// no field here to repeat into.
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
