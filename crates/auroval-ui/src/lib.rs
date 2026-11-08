//! The half of auroval's login screen that is not about logging in.
//!
//! This crate exists because of one sentence in issue #67: the lock screen is
//! *"closer to another front end over the same renderer than to a new
//! program"*. That claim was only true if the renderer could be pointed at a
//! surface it had not been written for — and it can, which is why this split
//! is mechanical rather than a rewrite. [`render::Gl::target`] takes a
//! `wl_surface` and nothing else; it does not know whether the compositor
//! handed that surface out through `wlr-layer-shell` or through
//! `ext-session-lock-v1`.
//!
//! So the cut is along the one line a second front end cares about:
//!
//! * **Here**, and shared: the GL renderer ([`render`]), the video decode
//!   ([`video`]), R9's fallback chain ([`background`]), the view and its
//!   geometry ([`view`], [`theme`]), the Lua config ([`config`]), the clock
//!   and date lines ([`clock`]), the screen's own state ([`ui`]), the
//!   last-login file ([`store`]) and the frame dump ([`dump`]).
//! * **Not here**, and in `crates/greet/`: the greetd socket, the auth loop,
//!   and the layer-shell event loop that drives them. A locker authenticates
//!   in its own process against PAM and runs a different protocol's event
//!   loop, so none of that is shared and pretending otherwise would be the
//!   expensive mistake.
//!
//! **The name is narrower than the contents, deliberately.** `ui` does not
//! obviously cover `config`, `store` and `sessions`. The alternative was to
//! split again — a `render` crate, a `config` crate — and that buys three
//! manifests and a dependency graph to maintain in exchange for a tidier
//! noun. One crate per *front end boundary* is the distinction that earns its
//! keep; one crate per module is not.
//!
//! **[`sessions`] is the one module here that a locker will never call.** It
//! reads `.desktop` files so the greeter can offer a session list, which is
//! greetd's business and no part of locking. It lives here because
//! [`ui::UiState`] preselects the last session and therefore depends on it,
//! and moving `UiState` without it would have meant making `UiState` generic
//! over its session type — a real design change, in a commit whose whole
//! claim is that it changes no behaviour. So the dependency was moved rather
//! than broken, and the untidiness is recorded instead of hidden. A locker
//! constructs `UiState` with an empty session list; see N3 in
//! `docs/prd/lock.md` for what to do about it, and when.

pub mod background;
pub mod clock;
pub mod config;
pub mod dump;
pub mod render;
pub mod sessions;
pub mod store;
pub mod theme;
pub mod ui;
pub mod video;
pub mod view;
