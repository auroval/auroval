//! The seam between the configuration file and everything that reads it.
//!
//! **This file defines the shape; it does not yet implement the loading.**
//! Q4 chose Lua in a restricted interpreter, and R13 and R22 say what that
//! has to survive. Building that is its own piece of work (issue C) and it
//! needs nothing from the renderer, the Wayland shell or greetd — which is
//! exactly why the type lives here and the implementation does not.
//!
//! The contract the loader has to meet, stated so it can be built against:
//!
//!   * **R13** — a config that fails to parse must not prevent login. Report
//!     the error with file and line, return `Config::default()`, carry on.
//!   * **R22** — theme code cannot prevent login either. A theme that errors,
//!     loops forever, allocates without bound, or reaches for the filesystem,
//!     the network or a process is stopped and the greeter continues on
//!     defaults, with the reason logged. The interpreter is created without
//!     `io`, `os`, `package`, `require`, `debug`, `load` and `dofile`, and
//!     with an instruction and memory ceiling.
//!
//! Both of those are the same sentence twice: **nothing in a file anybody can
//! edit may stand between a person and their machine.**

use std::path::PathBuf;

use crate::theme::{Geometry, Palette};

#[derive(Debug, Clone)]
pub struct Config {
    /// Where the still background comes from (R9). `None` means the solid
    /// colour, which is also what a missing or unreadable file degrades to.
    pub background: Option<PathBuf>,
    /// The palette table this theme ships, already parsed.
    pub palette: Palette,
    /// R11's numbers. A theme may move them; the defaults are hyprlock's.
    pub geometry: Geometry,
    /// R23, gated on C16: a sound to play while the login screen is up.
    /// `None` is the default and means nothing is opened and no device held.
    pub ambient_sound: Option<PathBuf>,
    /// Where R4's memory lives. Configurable so a test can point it at a
    /// temporary directory rather than at /var/lib.
    pub state_path: PathBuf,
    /// Everything that went wrong while loading, already formatted. The
    /// greeter shows these rather than hiding them: R13 says the error is
    /// reported, not swallowed.
    pub problems: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            background: None,
            palette: Palette::default(),
            geometry: Geometry::default(),
            ambient_sound: None,
            state_path: PathBuf::from(crate::store::DEFAULT_PATH),
            problems: Vec::new(),
        }
    }
}

impl Config {
    /// Load the configuration, or return the defaults and say why.
    ///
    /// **Stub.** It reads nothing yet and records that fact as a problem, so
    /// that a greeter built today behaves exactly as R13 requires — defaults,
    /// a visible reason, and a login that still works — while the real loader
    /// is written against this signature.
    pub fn load(path: &std::path::Path) -> Config {
        let mut c = Config::default();
        c.problems.push(format!(
            "configuration is not implemented yet; using defaults (wanted {})",
            path.display()
        ));
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_usable_on_its_own() {
        let c = Config::default();
        assert!(c.background.is_none());
        assert!(c.ambient_sound.is_none(), "R23 is silent unless asked for");
        assert!(crate::store::is_outside_homes(&c.state_path), "C10");
    }

    #[test]
    fn a_failure_to_load_still_yields_a_usable_config() {
        let c = Config::load(std::path::Path::new("/nonexistent/greet.lua"));
        assert_eq!(
            c.geometry.field_rounding,
            Geometry::default().field_rounding
        );
        assert!(!c.problems.is_empty(), "R13: the reason is reported");
    }
}
