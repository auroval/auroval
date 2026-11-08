//! The Wayland shell: one surface per output, the keyboard, and the loop.
//!
//! R10 wants one surface per output at that output's scale, and this is built
//! for N from the first line even though the machine it was written on has
//! one monitor. Retrofitting multi-output onto a single-surface design is how
//! you get an abstraction that cannot hold it; the GL context and the glyph
//! atlas are shared, and only the EGL surface and its size are per-output.
//!
//! The loop owns nothing it does not have to. greetd's conversation runs on
//! its own thread (R18 forbids a deadline of ours), and the view is a pure
//! function of `UiState`, so what is left here is Wayland, keys, and deciding
//! when to redraw.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopHandle, LoopSignal};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers, RepeatInfo},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

use crate::auth::{self, Answer};
use crate::background::Background;
use crate::config::Config;
use crate::render::{Gl, Target};
use crate::store::Store;
use crate::ui::{Stage, UiState};
use crate::view::{self, Style};

/// One output: its window, its EGL surface, and whether it needs drawing.
struct Screen {
    window: Window,
    target: Option<Target>,
    scale: i32,
    configured: bool,
}

pub struct Shell {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    compositor: CompositorState,
    xdg: XdgShell,
    qh: QueueHandle<Shell>,
    signal: LoopSignal,
    loop_handle: LoopHandle<'static, Shell>,
    keyboard: Option<wl_keyboard::WlKeyboard>,

    screens: HashMap<u32, Screen>,
    gl: Option<Gl>,
    background: Background,
    style: Style,
    store: Store,

    pub ui: UiState,
    to_auth: Option<Sender<Answer>>,
    from_auth: Option<Receiver<auth::Ui>>,
    session_cmd: Vec<String>,
    /// Resolved once at startup, not per tick: the greeter's environment is
    /// fixed by the time it draws, and a locale lookup every frame would be
    /// work for an answer that cannot change.
    locale: chrono::Locale,
    /// `AUROVAL_LOG_KEYS`. R6 asks for key repeat and a caps-lock warning,
    /// and both of those can only be exercised by a person holding a key on
    /// a real seat -- there is no seat in a unit test and no keyboard in a
    /// snapshot. Without this the verification is somebody saying it felt
    /// right, which is not a receipt. With it, the run leaves a log whose
    /// timestamps can be checked against the repeat rate the compositor
    /// announced.
    log_keys: bool,
    /// `AUROVAL_FRAME_DUMP`. `None` unless it is set; see `dump`.
    dump: Option<crate::dump::Dump>,

    started: Instant,
    pub first_frame: Option<Instant>,
    /// Set when there is no point running any more: the session is starting,
    /// or `preview` was asked for a single frame.
    pub done: bool,
    preview: bool,
}

/// Run the greeter. `preview` skips greetd entirely, which is how the view is
/// worked on without a login screen to log into.
pub fn run(cfg: Config, ui: UiState, preview: bool) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("no Wayland display: {e}"))?;
    let (globals, queue) =
        registry_queue_init::<Shell>(&conn).map_err(|e| format!("registry: {e}"))?;
    let qh = queue.handle();
    let mut loop_: EventLoop<Shell> = EventLoop::try_new().map_err(|e| e.to_string())?;

    let compositor = CompositorState::bind(&globals, &qh).map_err(|e| e.to_string())?;
    let xdg = XdgShell::bind(&globals, &qh).map_err(|e| e.to_string())?;

    let gl = unsafe { Gl::new(conn.backend().display_ptr() as *mut _) }?;
    // R11 names MesloLGS. Saying which font was actually loaded makes the
    // wrong one a visible failure rather than a silent substitution.
    eprintln!("auroval-greet: font {}", gl.atlas.source);
    // R11's typography is half the look and the language is the other half.
    // greetd clears the environment, so what the greeter ends up with is
    // whatever systemd exported, and that is not something to assume.
    let locale = crate::clock::locale_from_env();
    eprintln!("auroval-greet: {}", crate::clock::describe(locale));
    // C10, asserted rather than assumed: the greeter runs as its own user and
    // must work when /home/<user> is mode 700.
    if !crate::store::is_outside_homes(&cfg.state_path) {
        eprintln!(
            "auroval-greet: refusing {} — C10 forbids any greeter path inside a home directory",
            cfg.state_path.display()
        );
        return Err("state path is inside a home directory".into());
    }
    // R9. The decode runs on its own thread; the solid colour shows until it
    // lands, and every fallback says which file and why.
    let background = Background::start(cfg.background.as_deref());
    for p in background.problems() {
        eprintln!("auroval-greet: {p}");
    }
    if cfg.ambient_sound.is_some() {
        // R23 is gated on C16: the greeter cannot open a sound device at all.
        eprintln!("auroval-greet: a theme asked for ambient sound; blocked by C16, ignoring");
    }
    let session_cmd = ui.session().map(|s| s.command.clone()).unwrap_or_default();

    let mut shell = Shell {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        seat_state: SeatState::new(&globals, &qh),
        compositor,
        xdg,
        qh: qh.clone(),
        signal: loop_.get_signal(),
        loop_handle: loop_.handle(),
        keyboard: None,
        screens: HashMap::new(),
        gl: Some(gl),
        background,
        style: Style {
            geometry: cfg.geometry,
            palette: cfg.palette,
        },
        store: Store::at(&cfg.state_path),
        ui,
        to_auth: None,
        from_auth: None,
        session_cmd,
        locale,
        log_keys: std::env::var_os("AUROVAL_LOG_KEYS").is_some(),
        dump: crate::dump::Dump::from_env(),
        started: Instant::now(),
        first_frame: None,
        done: false,
        preview,
    };

    WaylandSource::new(conn.clone(), queue)
        .insert(loop_.handle())
        .map_err(|e| e.to_string())?;

    // Every output that already exists gets a surface; `new_output` handles
    // the ones that appear later.
    let outputs: Vec<_> = shell.output_state.outputs().collect();
    for o in outputs {
        shell.add_output(o);
    }

    while !shell.done {
        loop_
            .dispatch(Duration::from_millis(16), &mut shell)
            .map_err(|e| e.to_string())?;
        shell.pump_auth();
        shell.tick_clock();
        shell.draw_all();
    }
    Ok(())
}

impl Shell {
    fn add_output(&mut self, output: wl_output::WlOutput) {
        let id = output.id().protocol_id();
        if self.screens.contains_key(&id) {
            return;
        }
        let info = self.output_state.info(&output);
        let scale = info.as_ref().map(|i| i.scale_factor).unwrap_or(1);
        let surface = self.compositor.create_surface(&self.qh);
        let window = self
            .xdg
            .create_window(surface, WindowDecorations::RequestServer, &self.qh);
        window.set_title("auroval");
        window.set_app_id("auroval.greet");
        // A greeter owns the screen. Under `cage` this is already true; on a
        // normal compositor it is what makes the preview comparable.
        window.set_fullscreen(Some(&output));
        window.commit();
        self.screens.insert(
            id,
            Screen {
                window,
                target: None,
                scale,
                configured: false,
            },
        );
    }

    fn tick_clock(&mut self) {
        let now = chrono::Local::now();
        // hyprlock's own formats, in hyprlock's own language: see `clock`.
        let (c, d) = crate::clock::lines(now, self.locale);
        self.ui.clock = c;
        self.ui.date = d;
    }

    /// Everything greetd has said since the last frame.
    fn pump_auth(&mut self) {
        let Some(rx) = self.from_auth.as_ref() else {
            return;
        };
        let mut ended = None;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                auth::Ui::Prompt { text, secret } => {
                    self.ui.prompt = text;
                    self.ui.secret = secret;
                    self.ui.stage = Stage::Answering;
                    self.ui.typed.clear();
                }
                auth::Ui::Message { text, is_error } => {
                    self.ui.notify(crate::ui::Notice {
                        text,
                        kind: if is_error {
                            crate::ui::NoticeKind::Error
                        } else {
                            crate::ui::NoticeKind::Info
                        },
                    });
                }
                auth::Ui::Starting => {
                    self.ui.stage = Stage::Starting;
                    self.ui.typed.clear();
                    // R4, and only now: remembering a login that failed would
                    // make the next boot worse, not better.
                    let last = crate::store::LastLogin {
                        user: Some(self.ui.username.clone()),
                        session: self.ui.session().map(|s| s.id.clone()),
                    };
                    if let Err(e) = self.store.save(&last) {
                        eprintln!("auroval-greet: could not remember this login: {e}");
                    }
                    self.done = true;
                }
                auth::Ui::Ended(why) => ended = Some(why),
            }
        }
        if let Some(why) = ended {
            self.ui.notice = Some(crate::ui::Notice {
                text: why,
                kind: crate::ui::NoticeKind::Error,
            });
            self.ui.stage = Stage::AskingUser;
            self.ui.typed.clear();
            self.to_auth = None;
            self.from_auth = None;
        }
    }

    fn submit(&mut self) {
        match self.ui.stage {
            Stage::AskingUser => {
                if self.ui.username.trim().is_empty() || self.preview {
                    return;
                }
                let (to_ui, from_auth) = channel();
                let (to_auth, from_ui) = channel();
                self.session_cmd = self
                    .ui
                    .session()
                    .map(|s| s.command.clone())
                    .unwrap_or_default();
                auth::spawn(
                    self.ui.username.clone(),
                    self.session_cmd.clone(),
                    to_ui,
                    from_ui,
                );
                self.to_auth = Some(to_auth);
                self.from_auth = Some(from_auth);
                self.ui.notice = None;
                self.ui.stage = Stage::Answering;
                self.ui.prompt = "…".into();
            }
            Stage::Answering => {
                if let Some(tx) = &self.to_auth {
                    let _ = tx.send(Answer::Text(std::mem::take(&mut self.ui.typed)));
                    self.ui.prompt = "…".into();
                    // The answer is the person acting on whatever the last
                    // message said, so it stops being news here. Without
                    // this an error from the previous attempt would sit
                    // under the new prompt looking like it describes it.
                    self.ui.notice = None;
                }
            }
            Stage::Starting => {}
        }
    }

    fn cancel(&mut self) {
        if let Some(tx) = &self.to_auth {
            let _ = tx.send(Answer::Cancel);
        }
        self.to_auth = None;
        self.from_auth = None;
        self.ui.stage = Stage::AskingUser;
        self.ui.typed.clear();
        self.ui.prompt.clear();
    }

    fn draw_all(&mut self) {
        let Some(gl) = self.gl.as_mut() else { return };
        let ids: Vec<u32> = self.screens.keys().copied().collect();
        let first = ids.first().copied().unwrap_or_default();
        for id in ids {
            let Some(screen) = self.screens.get(&id) else {
                continue;
            };
            let Some(target) = screen.target.as_ref() else {
                continue;
            };
            if gl.make_current(target).is_err() {
                continue;
            }
            // The poll has to happen with the context current, which is why
            // it is here and not before the loop over outputs.
            let bg = self.background.ready(gl);
            view::draw(gl, &self.ui, &self.style, bg);
            // Before `present`, not after: the buffer this reads is the one
            // just drawn, and a swapped buffer's contents are undefined.
            // Only the first output, because a dump is a record of what was
            // said and not of how many screens said it.
            if id == first {
                if let Some(d) = self.dump.as_mut() {
                    let (w, h) = (target.width, target.height);
                    d.maybe(&self.ui, w, h, || gl.read_pixels());
                }
            }
            gl.present(target);
            if self.first_frame.is_none() {
                self.first_frame = Some(Instant::now());
                // R16 is a budget, so the number is printed rather than
                // assumed. P0's 243 ms was measured at 1255x674 in a nested
                // cage; this is the figure that replaces it.
                // One line, carrying both numbers a measurement needs, in a
                // shape a script can parse. R16 is a budget, so this is not a
                // debug print: the harness in spike/p1e reads exactly this.
                eprintln!(
                    "auroval-greet: first frame {} ms, surface {}x{}",
                    (Instant::now() - self.started).as_millis(),
                    target.width,
                    target.height
                );
                // The measurement harness in spike/p1e wants this number and
                // nothing else, and `preview` otherwise draws forever. Eleven
                // runs that each have to be killed by a 30 s timeout is five
                // and a half minutes of black screen on a VT the measurement
                // needs you to stay on — and the one thing that reliably went
                // wrong was someone switching away out of boredom, which
                // makes every remaining run fail for want of DRM master. So
                // the harness asks for one frame and gets exactly one.
                if std::env::var_os("AUROVAL_EXIT_AFTER_FIRST_FRAME").is_some() {
                    self.done = true;
                }
            }
            if self.preview {
                // One frame is all a preview owes anybody, but the clock has
                // to keep moving for the look to be judged, so it keeps
                // drawing and simply never logs in.
            }
        }
    }
}

impl CompositorHandler for Shell {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new: i32,
    ) {
        for s in self.screens.values_mut() {
            if s.window.wl_surface() == surface {
                s.scale = new;
            }
        }
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
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

impl WindowHandler for Shell {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        // A greeter has no close button. In preview it is the way out.
        if self.preview {
            self.done = true;
            self.signal.stop();
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        let Some(gl) = self.gl.as_ref() else { return };
        for screen in self.screens.values_mut() {
            if &screen.window != window {
                continue;
            }
            let w = configure.new_size.0.map(|v| v.get()).unwrap_or(1920);
            let h = configure.new_size.1.map(|v| v.get()).unwrap_or(1080);
            // R10: the surface is sized in physical pixels, which is the
            // logical size times this output's scale. Getting this wrong is
            // how text comes out soft on one monitor and sharp on another.
            let (pw, ph) = (w * screen.scale as u32, h * screen.scale as u32);
            match screen.target.as_mut() {
                Some(t) => t.resize(pw, ph, screen.scale),
                None => match unsafe { gl.target(window.wl_surface(), pw, ph, screen.scale) } {
                    Ok(t) => screen.target = Some(t),
                    Err(e) => eprintln!("auroval-greet: no EGL surface for this output: {e}"),
                },
            }
            screen.configured = true;
        }
    }
}

impl OutputHandler for Shell {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, o: wl_output::WlOutput) {
        self.add_output(o);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, o: wl_output::WlOutput) {
        let id = o.id().protocol_id();
        if let Some(info) = self.output_state.info(&o) {
            if let Some(s) = self.screens.get_mut(&id) {
                s.scale = info.scale_factor;
            }
        }
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, o: wl_output::WlOutput) {
        self.screens.remove(&o.id().protocol_id());
    }
}

impl SeatHandler for Shell {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            // With repeat, because holding backspace in a password field is
            // not a luxury. SCTK drives the repeat off the event loop, which
            // is why the handle is kept.
            let r = self.seat_state.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.loop_handle.clone(),
                // Repeats arrive HERE and nowhere else. SCTK's calloop timer
                // calls this closure directly; `KeyboardHandler::repeat_key`
                // only fires for a server-sent `KeyState::Repeated`, which
                // most compositors never send. The first R6 sitting logged
                // zero repeats for that reason and read as a failure of the
                // greeter, which it was not.
                Box::new(|shell: &mut Shell, _kbd, event| {
                    shell.log_key("repeat", &event);
                    shell.on_key(event)
                }),
            );
            match r {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => eprintln!("auroval-greet: no keyboard: {e}"),
            }
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Shell {
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

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.log_key("press", &event);
        self.on_key(event);
    }

    /// Only for a repeat the *compositor* sends as `KeyState::Repeated`.
    /// Local repeat does not come through here; see the closure in
    /// `new_capability`.
    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.log_key("repeat-from-compositor", &event);
        self.on_key(event);
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

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        raw: RawModifiers,
        _: u32,
    ) {
        // R6: the warning has to be visible before you submit, so it tracks
        // the lock itself rather than the shift key.
        let was = self.ui.caps_lock;
        self.ui.caps_lock = raw.locked & 0x2 != 0;
        if self.log_keys && was != self.ui.caps_lock {
            eprintln!(
                "keys {:>7.3}  caps-lock {}",
                self.started.elapsed().as_secs_f32(),
                if self.ui.caps_lock { "on" } else { "off" }
            );
        }
    }

    fn update_keymap(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: smithay_client_toolkit::seat::keyboard::Keymap<'_>,
    ) {
    }

    fn update_repeat_info(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        info: RepeatInfo,
    ) {
        // SCTK does the repeating; this is only worth saying out loud,
        // because it is the number the logged intervals have to agree with.
        if self.log_keys {
            eprintln!(
                "keys {:>7.3}  repeat-info {info:?}",
                self.started.elapsed().as_secs_f32()
            );
        }
    }
}

impl Shell {
    /// One line per keyboard event, when `AUROVAL_LOG_KEYS` is set.
    ///
    /// The character is deliberately **not** logged: this runs in a password
    /// field, and an instrumentation flag that writes what you typed into a
    /// log is a worse bug than the one it was added to find. The keysym is
    /// enough to tell a repeat from a press, and the count is enough to tell
    /// whether the field agreed.
    ///
    /// The count is `ui.field()`, not `ui.typed`: at the username stage
    /// `push` fills `username` and `typed` stays empty, so the first R6
    /// sitting logged `typed 0 chars` for every keystroke and read as "no
    /// character ever reached the field" when every character had.
    fn log_key(&self, how: &str, event: &KeyEvent) {
        if !self.log_keys {
            return;
        }
        eprintln!(
            "keys {:>7.3}  {how:<6}  keysym {:?}  field {} chars",
            self.started.elapsed().as_secs_f32(),
            event.keysym.name().unwrap_or("?"),
            self.ui.field().chars().count()
        );
    }

    fn on_key(&mut self, event: KeyEvent) {
        match event.keysym {
            Keysym::Return | Keysym::KP_Enter => self.submit(),
            Keysym::BackSpace => self.ui.backspace(),
            Keysym::Escape => self.cancel(),
            Keysym::Tab => self.ui.cycle_session(true),
            Keysym::ISO_Left_Tab => self.ui.cycle_session(false),
            _ => {
                if let Some(t) = event.utf8 {
                    for c in t.chars() {
                        // Control characters are not text, and a stray one in
                        // a password field is a bug you cannot see.
                        if !c.is_control() {
                            self.ui.push(c);
                        }
                    }
                }
            }
        }
    }
}

impl ProvidesRegistryState for Shell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Shell);
delegate_output!(Shell);
delegate_seat!(Shell);
delegate_keyboard!(Shell);
delegate_xdg_shell!(Shell);
delegate_xdg_window!(Shell);
delegate_registry!(Shell);
