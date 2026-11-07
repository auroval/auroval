//! One xdg-shell surface, driven by frame callbacks, shared by both modes.
//!
//! Both renderers present on a frame callback and pull the newest sample the
//! pipeline has produced. That is deliberate: the decode and upload paths are
//! what this spike compares, so the presentation cadence has to be identical
//! or the comparison measures the cadence instead.

use std::time::{Duration, Instant};

use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopSignal};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_output, delegate_registry, delegate_shm, delegate_xdg_shell,
    delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_surface},
    Connection, QueueHandle,
};

use crate::{cpu_seconds, Report};

/// What a renderer has to be able to do. Deliberately small: everything that
/// differs between the two paths is behind these three calls.
pub trait Renderer {
    /// The surface has its size. Create whatever depends on it.
    fn configure(&mut self, conn: &Connection, surface: &wl_surface::WlSurface, w: u32, h: u32);
    /// Present one frame if a new sample is available. Returns true if the
    /// surface was committed with new content.
    fn present(&mut self, app: &mut Surface<'_>) -> bool;
    /// Frames the pipeline has decoded so far.
    fn decoded(&self) -> u64;
    fn mode(&self) -> &'static str;
}

/// The bits of the app a renderer is allowed to touch while presenting.
pub struct Surface<'a> {
    pub qh: &'a QueueHandle<App>,
    pub surface: &'a wl_surface::WlSurface,
    pub pool: &'a mut Option<SlotPool>,
    pub shm: &'a Shm,
    pub width: u32,
    pub height: u32,
}

pub struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: Option<SlotPool>,
    window: Window,
    pub width: u32,
    pub height: u32,
    configured: bool,
    signal: LoopSignal,
    deadline: Instant,

    renderer: Box<dyn Renderer>,
    pub presented: u64,
    started: Instant,
    first_present: Option<Instant>,
}

/// Build the surface, run until the deadline, and report.
pub fn run_with(
    mut make: impl FnMut() -> Box<dyn Renderer>,
    w: u32,
    h: u32,
    seconds: f64,
) -> Result<Report, Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let mut loop_: EventLoop<App> = EventLoop::try_new()?;

    let compositor = CompositorState::bind(&globals, &qh)?;
    let xdg = XdgShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let surface = compositor.create_surface(&qh);
    let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_title("auroval Q5 spike");
    window.set_app_id("auroval.spike.q5");
    window.set_min_size(Some((w, h)));
    // Fullscreen, because R7's budget is about an output's native resolution
    // and a tiled window would measure a smaller surface. The compositor
    // decides the final size; the report prints what it actually got.
    window.set_fullscreen(None);
    window.commit();

    let renderer = make();
    let mut app = App {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool: None,
        window,
        width: w,
        height: h,
        configured: false,
        signal: loop_.get_signal(),
        deadline: Instant::now() + Duration::from_secs_f64(seconds + 2.0),
        renderer,
        presented: 0,
        started: Instant::now(),
        first_present: None,
    };

    WaylandSource::new(conn.clone(), queue).insert(loop_.handle())?;
    let stop_at = Instant::now() + Duration::from_secs_f64(seconds);

    // Measurement starts once the first frame is on screen, so that pipeline
    // construction and GStreamer's registry scan do not land in the average.
    let mut measure_from: Option<(Instant, f64, u64, u64)> = None;

    loop {
        loop_.dispatch(Duration::from_millis(8), &mut app)?;
        if app.configured {
            app.try_present(&conn, &qh);
        }
        if measure_from.is_none() && app.first_present.is_some() {
            measure_from = Some((
                Instant::now(),
                cpu_seconds(),
                app.presented,
                app.renderer.decoded(),
            ));
        }
        if Instant::now() >= stop_at || Instant::now() >= app.deadline {
            break;
        }
    }

    let (t0, c0, p0, d0) = measure_from.ok_or("no frame was ever presented")?;
    Ok(Report {
        mode: app.renderer.mode(),
        width: app.width,
        height: app.height,
        wall: t0.elapsed(),
        cpu: cpu_seconds() - c0,
        presented: app.presented - p0,
        decoded: app.renderer.decoded() - d0,
        first_frame_ms: app
            .first_present
            .map(|t| (t - app.started).as_secs_f64() * 1000.0)
            .unwrap_or(f64::NAN),
    })
}

impl App {
    fn try_present(&mut self, conn: &Connection, qh: &QueueHandle<App>) {
        let surface = self.window.wl_surface().clone();
        let _ = conn;
        let mut s = Surface {
            qh,
            surface: &surface,
            pool: &mut self.pool,
            shm: &self.shm,
            width: self.width,
            height: self.height,
        };
        if self.renderer.present(&mut s) {
            self.presented += 1;
            if self.first_present.is_none() {
                self.first_present = Some(Instant::now());
            }
        }
    }
}

impl CompositorHandler for App {
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
    fn frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.try_present(conn, qh);
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

impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.signal.stop();
    }
    fn configure(
        &mut self,
        conn: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        if let (Some(w), Some(h)) = (configure.new_size.0, configure.new_size.1) {
            self.width = w.get();
            self.height = h.get();
        }
        if !self.configured {
            self.configured = true;
            let surface = self.window.wl_surface().clone();
            self.renderer
                .configure(conn, &surface, self.width, self.height);
        }
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_xdg_shell!(App);
delegate_xdg_window!(App);
delegate_registry!(App);

/// `wl_display` pointer, for handing to EGL.
pub fn display_ptr(conn: &Connection) -> *mut std::ffi::c_void {
    conn.backend().display_ptr() as *mut _
}
