//! The Wayland side of phase 2: one fullscreen surface under `cage`, showing
//! the video and whatever greetd asks.
//!
//! `wl_shm` is used deliberately and temporarily — see the note at the top of
//! `greet.rs`. Nothing here is a view on how `greet` should eventually render.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopSignal};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm, ShmHandler,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

use crate::greet::{Answer, FrameBus, Ui};

pub struct Greeter {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: Option<SlotPool>,
    buffer: Option<Buffer>,
    window: Window,
    width: u32,
    height: u32,
    configured: bool,
    exit: bool,
    signal: LoopSignal,
    keyboard: Option<wl_keyboard::WlKeyboard>,

    bus: Arc<FrameBus>,
    ui_rx: Receiver<Ui>,
    ans_tx: Sender<Answer>,
    font: Option<fontdue::Font>,

    /// What greetd last asked, and what has been typed in answer.
    prompt: String,
    secret: bool,
    awaiting: bool,
    typed: String,
    /// Messages to show: `info`, `error` and our own status.
    notice: String,
    caps_lock: bool,

    started: Instant,
    first_present: Option<Instant>,
    /// Set once the session is starting, so the UI can stop drawing.
    done: bool,

    /// The video is not started until the first `configure`, because only then
    /// is the surface size known. Scaling on the GPU to the exact surface size
    /// means the UI never rescales and never blits a mismatched stride.
    video: Option<String>,
    video_started: bool,
    /// Unattended runs stop on their own so a measurement can be collected.
    deadline: Option<Instant>,
}

const BG: [u8; 4] = [0x1e, 0x18, 0x24, 0xff]; // Catppuccin Mocha base, BGRx.

impl Greeter {
    /// Pull everything the auth thread has said since the last frame.
    fn drain_ui(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                Ui::Prompt { text, secret } => {
                    self.prompt = text;
                    self.secret = secret;
                    self.awaiting = true;
                    self.typed.clear();
                }
                Ui::Message { text, is_error } => {
                    // R17 wants info rendered as content. In the spike it is at
                    // least given its own line at full size rather than being
                    // squeezed into a status bar.
                    self.notice = if is_error { format!("! {text}") } else { text };
                }
                Ui::Starting => {
                    self.notice = "starting session…".into();
                    self.awaiting = false;
                    self.done = true;
                }
                Ui::Dead(why) => {
                    self.notice = format!("! {why}");
                    self.awaiting = false;
                }
            }
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let (w, h) = (self.width.max(1), self.height.max(1));
        let stride = w as i32 * 4;

        // Snapshot everything the drawing needs before the pool is borrowed
        // mutably, so the canvas and this state are never borrowed at once.
        let font = self.font.clone();
        let prompt = self.prompt.clone();
        let awaiting = self.awaiting;
        let secret = self.secret;
        let typed = self.typed.clone();
        let caps = self.caps_lock;
        let notice_lines = if self.notice.is_empty() {
            Vec::new()
        } else {
            wrap(&self.notice, 60)
        };

        if self.pool.is_none() {
            self.pool = SlotPool::new(w as usize * h as usize * 4, &self.shm).ok();
        }
        if self.buffer.is_none() {
            let Some(pool) = self.pool.as_mut() else {
                return;
            };
            match pool.create_buffer(w as i32, h as i32, stride, wl_shm::Format::Xrgb8888) {
                Ok((buf, _)) => self.buffer = Some(buf),
                Err(_) => return,
            }
        }

        let frame = self.bus.latest.lock().unwrap().clone();
        let mut got_frame = false;

        {
            let Some(pool) = self.pool.as_mut() else {
                return;
            };
            let Some(buffer) = self.buffer.as_ref() else {
                return;
            };
            let Some(canvas) = pool.canvas(buffer) else {
                return;
            };

            // 1. Background: the decoded frame if there is one, else the solid
            //    colour. R9's fallback order in miniature, and the reason the
            //    greeter is usable before the first frame arrives.
            match frame.as_ref() {
                Some(src) if src.len() == canvas.len() => {
                    canvas.copy_from_slice(src);
                    got_frame = true;
                }
                _ => {
                    for px in canvas.chunks_exact_mut(4) {
                        px.copy_from_slice(&BG);
                    }
                }
            }

            // 2. Whatever greetd asked, and whatever has been typed.
            if let Some(font) = font.as_ref() {
                let cx = w / 2;
                let cy = h / 2;
                if !prompt.is_empty() {
                    blit_text(canvas, w, h, font, 40.0, cx, cy - 80, &prompt);
                }
                if awaiting {
                    let shown = if secret {
                        "\u{2022}".repeat(typed.chars().count())
                    } else {
                        typed.clone()
                    };
                    blit_text(canvas, w, h, font, 44.0, cx, cy, &shown);
                }
                if caps {
                    // R6, cheaply.
                    blit_text(canvas, w, h, font, 26.0, cx, cy + 60, "CAPS LOCK");
                }
                // R17: wrap rather than clip, so a URL and a code stay readable.
                for (i, line) in notice_lines.iter().enumerate() {
                    blit_text(
                        canvas,
                        w,
                        h,
                        font,
                        32.0,
                        cx,
                        cy + 140 + (i as u32 * 42),
                        line,
                    );
                }
            }
        }

        if got_frame {
            self.bus.presented.fetch_add(1, Ordering::Relaxed);
            if self.first_present.is_none() {
                self.first_present = Some(Instant::now());
            }
        }

        let Some(buffer) = self.buffer.as_ref() else {
            return;
        };
        let surface = self.window.wl_surface();
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.frame(qh, surface.clone());
        buffer.attach_to(surface).ok();
        self.window.commit();
    }
}

/// Blit a string centred on `cx`. Not typography — just legible enough to read
/// a prompt and a device code. A free function rather than a method so it can
/// write into the shm canvas while the pool is mutably borrowed.
#[allow(clippy::too_many_arguments)]
fn blit_text(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    font: &fontdue::Font,
    size: f32,
    cx: u32,
    baseline: u32,
    s: &str,
) {
    let glyphs: Vec<_> = s
        .chars()
        .map(|c| {
            let (m, bm) = font.rasterize(c, size);
            (m, bm)
        })
        .collect();
    let total: f32 = glyphs.iter().map(|(m, _)| m.advance_width).sum();
    let mut pen = cx as f32 - total / 2.0;
    for (m, bm) in &glyphs {
        let gx = pen + m.xmin as f32;
        let gy = baseline as f32 - m.height as f32 - m.ymin as f32;
        for row in 0..m.height {
            for col in 0..m.width {
                let a = bm[row * m.width + col] as u32;
                if a == 0 {
                    continue;
                }
                let px = gx as i32 + col as i32;
                let py = gy as i32 + row as i32;
                if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                    continue;
                }
                let off = (py as usize * w as usize + px as usize) * 4;
                if off + 4 > canvas.len() {
                    continue;
                }
                // Catppuccin Mocha text (#cdd6f4), BGRx byte order.
                let fg = [0xf4u8, 0xd6, 0xcd];
                for k in 0..3 {
                    let dst = canvas[off + k] as u32;
                    canvas[off + k] = (((fg[k] as u32 * a) + dst * (255 - a)) / 255) as u8;
                }
            }
        }
        pen += m.advance_width;
    }
}

fn wrap(s: &str, cols: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        if line.chars().count() + word.chars().count() + 1 > cols && !line.is_empty() {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

impl CompositorHandler for Greeter {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.drain_ui();
        self.draw(qh);
    }
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

impl WindowHandler for Greeter {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
        self.signal.stop();
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        let (w, h) = configure.new_size;
        self.width = w.map(|v| v.get()).unwrap_or(2560);
        self.height = h.map(|v| v.get()).unwrap_or(1440);
        // The buffer and pool are sized to the surface, so both are discarded
        // on a resize rather than reused.
        self.buffer = None;
        self.pool = None;
        if !self.configured {
            self.configured = true;
            eprintln!(
                "[spike] configured {}x{} after {:.0}ms",
                self.width,
                self.height,
                self.started.elapsed().as_secs_f64() * 1000.0
            );
        }
        // Start (or restart) decoding at exactly this size.
        if !self.video_started {
            if let Some(v) = self.video.clone() {
                self.video_started = true;
                // R9: a decode failure must cost appearance, never login. If
                // this fails the greeter carries on over the solid colour.
                match crate::greet::spawn_video(v, self.width, self.height, Arc::clone(&self.bus)) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[spike] video unavailable, falling back to solid colour: {e}")
                    }
                }
            }
        }
        self.draw(qh);
    }
}

impl SeatHandler for Greeter {
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
            self.keyboard = self.seat_state.get_keyboard(qh, &seat, None).ok();
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

impl KeyboardHandler for Greeter {
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
        match event.keysym {
            Keysym::Return | Keysym::KP_Enter => {
                if self.awaiting {
                    self.awaiting = false;
                    let answer = std::mem::take(&mut self.typed);
                    self.notice = "checking…".into();
                    let _ = self.ans_tx.send(Answer::Text(answer));
                }
            }
            Keysym::BackSpace => {
                self.typed.pop();
            }
            Keysym::Escape => {
                // Esc cancels the conversation rather than the process: R3 wants
                // the session cancelled cleanly, not the greeter gone.
                let _ = self.ans_tx.send(Answer::Cancel);
                self.awaiting = false;
            }
            _ => {
                if let Some(t) = event.utf8 {
                    if !t.chars().any(|c| c.is_control()) {
                        self.typed.push_str(&t);
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
    /// Key repeat matters for backspace on a mistyped password, so it is routed
    /// to the same handler rather than ignored.
    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(conn, qh, kb, serial, event);
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.caps_lock = modifiers.caps_lock;
    }
}

impl ShmHandler for Greeter {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl OutputHandler for Greeter {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for Greeter {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Greeter);
delegate_output!(Greeter);
delegate_shm!(Greeter);
delegate_seat!(Greeter);
delegate_keyboard!(Greeter);
delegate_xdg_shell!(Greeter);
delegate_xdg_window!(Greeter);
delegate_registry!(Greeter);

/// Run the greeter. Returns once the session starts or the window closes.
pub fn run(
    bus: Arc<FrameBus>,
    ui_rx: Receiver<Ui>,
    ans_tx: Sender<Answer>,
    video: Option<String>,
    run_for: Option<std::time::Duration>,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let mut event_loop: EventLoop<Greeter> = EventLoop::try_new()?;
    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle())?;

    let compositor = CompositorState::bind(&globals, &qh)?;
    let xdg_shell = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;

    let surface = compositor.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_title("auroval greet (P0 spike)");
    window.set_app_id("dev.auroval.greet.spike");
    window.set_fullscreen(None);
    window.commit();

    let mut state = Greeter {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool: None,
        buffer: None,
        window,
        width: 0,
        height: 0,
        configured: false,
        exit: false,
        signal: event_loop.get_signal(),
        keyboard: None,
        bus: Arc::clone(&bus),
        ui_rx,
        ans_tx,
        font: crate::greet::load_font(),
        prompt: String::new(),
        secret: false,
        awaiting: false,
        typed: String::new(),
        notice: "connecting to greetd…".into(),
        caps_lock: false,
        started,
        first_present: None,
        done: false,
        video,
        video_started: false,
        deadline: run_for.map(|d| Instant::now() + d),
    };

    loop {
        event_loop.dispatch(std::time::Duration::from_millis(16), &mut state)?;
        if state.exit || state.done {
            break;
        }
        if let Some(d) = state.deadline {
            if Instant::now() >= d {
                eprintln!("[spike] deadline reached; stopping");
                break;
            }
        }
    }

    bus.stop.store(true, Ordering::Relaxed);

    // R16's measurement: the budget is to be set from this number, not asserted.
    let decoded = bus.decoded.load(Ordering::Relaxed);
    let presented = bus.presented.load(Ordering::Relaxed);
    eprintln!("[spike] --- presentation ---");
    eprintln!(
        "[spike] surface configured : {}x{}",
        state.width, state.height
    );
    eprintln!(
        "[spike] first decoded frame: {}",
        match *bus.first_frame.lock().unwrap() {
            Some(t) => format!("{:.0}ms after start", (t - started).as_secs_f64() * 1000.0),
            None => "never".into(),
        }
    );
    eprintln!(
        "[spike] first presented    : {}",
        match state.first_present {
            Some(t) => format!("{:.0}ms after start", (t - started).as_secs_f64() * 1000.0),
            None => "never".into(),
        }
    );
    eprintln!("[spike] frames decoded     : {decoded}");
    eprintln!("[spike] frames presented   : {presented}");
    eprintln!(
        "[spike] ran for            : {:.1}s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
