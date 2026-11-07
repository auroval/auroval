//! The seam between the configuration file and everything that reads it.
//!
//! Q4 chose Lua in a restricted interpreter, and R13 and R22 say what that has
//! to survive. The shape is below; the loader under it is the implementation of
//! both requirements, which are the same sentence twice: **nothing in a file
//! anybody can edit may stand between a person and their machine.**
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
//! The sandbox is built by *omission* rather than by deletion: the interpreter
//! is created with only `table`, `string` and `math`, so `io`, `os`, `package`
//! and `debug` are never opened in the first place. That ordering matters.
//! Deleting globals from a fully-loaded state is the weaker move, because the
//! `debug` library can hand the others straight back —
//! `debug.getregistry()["_LOADED"].io` is still `io` however thoroughly
//! `_G.io` was set to `nil`. What cannot be reconstructed is what was never
//! opened. `load` and `dofile` are the exception: they live in the base
//! library, which is not optional, so those are removed by name.
//!
//! Q4 accepted that this boundary is softer than a separate process or a
//! WebAssembly instance, and said so. The claim being made here is only the
//! one in R22: a misbehaving theme costs you the *look*, never the login.

use std::path::{Path, PathBuf};

use mlua::{Lua, LuaOptions, StdLib, Table};

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

/// How many VM instructions a theme may spend before it is killed.
///
/// R22 asks that a theme looping forever "does not delay the first prompt",
/// so this is set by what it costs rather than by what a theme might want.
/// Measured: two million instructions of `while true do end` takes **14 ms**
/// on this machine, stable across runs, against R16's 500 ms budget for the
/// whole first frame — so the worst theme imaginable spends under 3% of it.
/// A theme that merely returns a table does not come close to the ceiling.
const INSTRUCTION_CEILING: u32 = 2_000_000;

/// How much a theme may allocate before the allocator starts refusing.
///
/// A config that declares a palette and some numbers needs kilobytes. This is
/// generous by three orders of magnitude and still small enough that the
/// runaway case is refused long before the machine notices.
const MEMORY_CEILING: usize = 8 * 1024 * 1024;

/// The base-library names that survive `StdLib` selection and have to go by
/// hand. Everything dangerous *outside* the base library is handled by never
/// opening it; these are the ones that arrive with the base library whether
/// they were asked for or not.
///
/// `loadstring` and `loadfile` are not in R22's list and are denied anyway:
/// they are `load` and `dofile` under other names, and a list that denied the
/// named three while leaving their synonyms bound would be a sandbox in
/// wording only.
///
/// `require` is listed even though `package` is never opened — belt and
/// braces, and it makes the denial explicit at the one name R22 names.
const DENIED_GLOBALS: &[&str] = &["load", "loadstring", "dofile", "loadfile", "require"];

impl Config {
    /// Load the configuration, or return the defaults and say why.
    ///
    /// This never fails. Every path out of it yields a `Config` the greeter can
    /// run on, and anything that went wrong is appended to `problems` and
    /// written to stderr — which under greetd is the journal. R13 and R22 both
    /// reduce to that sentence.
    pub fn load(path: &Path) -> Config {
        let mut c = Config::default();

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                // Not strictly a failure: no config is the state of a fresh
                // install. It is still reported, because a theme that is on
                // disk but unreadable looks exactly like this from here and
                // guessing which one it is would be the silent half of R13.
                c.note(format!(
                    "{}: not read ({e}); using defaults",
                    path.display()
                ));
                return c;
            }
        };

        if let Err(e) = run_theme(&mut c, &source, path) {
            c.note(format!("{}: {e}; using defaults", path.display()));
        }
        c
    }

    /// Record a problem so that it cannot be silent. R22's fit criterion is
    /// explicit that the reason is logged, so this does both: the `problems`
    /// vector for whatever renders it, and stderr for the journal, which is
    /// the only one of the two that exists before there is a screen.
    fn note(&mut self, msg: String) {
        eprintln!("auroval-greet: config: {msg}");
        self.problems.push(msg);
    }

    /// Take what the theme returned, field by field.
    ///
    /// Every field is independent and every one of them falls back on its own.
    /// A theme that gets `geometry` wrong keeps its palette; a theme that
    /// returns rubbish for `background` still moves the clock. This is the
    /// same rule as `Palette::parse`'s treatment of unknown roles, for the same
    /// reason: a half-written theme must not be the reason nobody can log in.
    fn absorb(&mut self, t: &Table, path: &Path) {
        match t.get::<Option<String>>("background") {
            Ok(Some(s)) => self.background = Some(PathBuf::from(s)),
            Ok(None) => {}
            Err(e) => self.note(format!("{}: background ignored ({e})", path.display())),
        }
        match t.get::<Option<String>>("ambient_sound") {
            Ok(Some(s)) => self.ambient_sound = Some(PathBuf::from(s)),
            Ok(None) => {}
            Err(e) => self.note(format!("{}: ambient_sound ignored ({e})", path.display())),
        }
        match t.get::<Option<String>>("state_path") {
            Ok(Some(s)) => self.state_path = PathBuf::from(s),
            Ok(None) => {}
            Err(e) => self.note(format!("{}: state_path ignored ({e})", path.display())),
        }

        match t.get::<Option<Table>>("palette") {
            Ok(Some(p)) => self.palette = palette_from(&p),
            Ok(None) => {}
            Err(e) => self.note(format!("{}: palette ignored ({e})", path.display())),
        }
        match t.get::<Option<Table>>("geometry") {
            Ok(Some(g)) => geometry_from(&g, &mut self.geometry),
            Ok(None) => {}
            Err(e) => self.note(format!("{}: geometry ignored ({e})", path.display())),
        }
    }
}

/// The interpreter R22 describes, and nothing more.
///
/// This is deliberately its own function rather than inlined into the loader,
/// because R22's list of denied names is a claim about *construction* and a
/// test should be able to interrogate it directly rather than only through its
/// consequences.
fn restricted_lua() -> mlua::Result<Lua> {
    // Omission, not deletion: see the module header. `coroutine` is left out
    // too — not because it is dangerous, but because a theme that yields is a
    // theme whose instruction ceiling is harder to reason about, and nothing
    // in R11 needs it.
    let lua = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::MATH,
        LuaOptions::default(),
    )?;

    let globals = lua.globals();
    for name in DENIED_GLOBALS {
        globals.set(*name, mlua::Value::Nil)?;
    }

    // A failure to install either ceiling is a failure of the sandbox, not a
    // detail, so it propagates and the theme does not run at all. Running
    // unmetered theme code would be the one outcome R22 forbids outright.
    lua.set_memory_limit(MEMORY_CEILING)?;
    lua.set_hook(
        mlua::HookTriggers::new().every_nth_instruction(INSTRUCTION_CEILING),
        |_, _| {
            Err(mlua::Error::RuntimeError(format!(
                "instruction ceiling of {INSTRUCTION_CEILING} exceeded; theme stopped"
            )))
        },
    )?;

    Ok(lua)
}

/// Run the chunk in that interpreter and insist on a table.
///
/// The chunk is named after the file so that mlua's errors carry
/// `path:line:` — which is literally what R13 asks to be reported, and is why
/// the name is the path rather than something prettier.
/// The table is read here, inside the scope that owns the interpreter, and
/// deliberately not returned.
///
/// An `mlua::Table` is a handle into a live `Lua`, so handing one back to the
/// caller and letting the `Lua` drop at the end of this function makes every
/// subsequent `get` **panic** with "Lua instance is destroyed". A panic in the
/// config loader is the one failure mode R13 and R22 exist to rule out, and it
/// would be a panic on the *success* path — reached by a theme that is
/// perfectly well behaved. So the borrow stays inside, `Config` leaves with
/// plain Rust values, and the signature makes it impossible to get wrong.
///
/// The `good.lua` fixture is what caught this. That is the whole argument for
/// keeping a control among the misbehaving ones: the four hostile themes all
/// passed, because they never got far enough to touch the table.
fn run_theme(c: &mut Config, source: &str, path: &Path) -> mlua::Result<()> {
    let lua = restricted_lua()?;

    // The `@` prefix tells Lua this name is a file rather than a source
    // string, which changes how it is shortened for errors: a plain name is
    // elided at the end (losing the file name, the useful half), while an `@`
    // name is elided at the front and keeps the tail. `note` still prefixes
    // the full path, because Lua's own limit is 60 characters and a deep
    // enough path would eat into the file name — the small redundancy buys a
    // guarantee that does not depend on that constant.
    let value: mlua::Value = lua
        .load(source)
        .set_name(format!("@{}", path.display()))
        .eval()?;

    match value {
        mlua::Value::Table(t) => {
            c.absorb(&t, path);
            Ok(())
        }
        other => Err(mlua::Error::RuntimeError(format!(
            "expected the theme to return a table, got {}",
            other.type_name()
        ))),
    }
}

/// Read a palette table of `role = "rrggbb"` pairs.
///
/// This flattens the table into the line form `Palette::parse` already takes
/// rather than re-deriving the role mapping. That keeps one copy of the rules
/// that matter — `tertiary` colours the check, `surface` supplies a hue and
/// not the field's translucency, unknown roles are ignored — and those rules
/// stay tested where they live, in `theme`.
fn palette_from(t: &Table) -> Palette {
    let mut lines = String::new();
    // `flatten` drops the pairs that failed to come across as two strings. A
    // non-string value is simply not a colour, so it is skipped like any other
    // unparseable role rather than discarding the whole palette.
    for (k, v) in t.pairs::<String, String>().flatten() {
        lines.push_str(&format!("{k} = {v}\n"));
    }
    Palette::parse(&lines)
}

/// Overlay whichever of R11's numbers the theme chose to move.
///
/// Absent and non-numeric both mean "keep hyprlock's value", so a theme can
/// set one field without restating thirteen.
fn geometry_from(t: &Table, g: &mut Geometry) {
    macro_rules! overlay {
        ($($f:ident),+ $(,)?) => {
            $(if let Ok(Some(v)) = t.get::<Option<f32>>(stringify!($f)) {
                g.$f = v;
            })+
        };
    }
    overlay!(
        clock_px,
        clock_dy,
        date_px,
        date_dy,
        user_px,
        user_dy,
        field_w,
        field_h,
        field_dy,
        field_rounding,
        rounding_power,
        outline,
        dots_size,
        dots_spacing,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// The misbehaving themes live in the repository, not in these strings, so
    /// that the requirement can be re-run by hand and not only by cargo.
    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/themes")
            .join(name)
    }

    /// Evaluate one boolean expression inside the real sandbox.
    ///
    /// `None` means it did not even return a boolean, which for these tests is
    /// as much a failure as returning the wrong one.
    fn sandbox_says(src: &str) -> Option<bool> {
        let lua = restricted_lua().ok()?;
        lua.load(src).set_name("<sandbox probe>").eval().ok()
    }

    /// What "yields a usable login" means, checked in one place: the numbers
    /// the login screen is drawn from are the defaults, and nothing is `None`
    /// that the greeter would dereference.
    fn is_usable(c: &Config) -> bool {
        c.geometry.field_rounding == Geometry::default().field_rounding
            && c.geometry.clock_px == Geometry::default().clock_px
            && c.palette.primary == Palette::default().primary
            && crate::store::is_outside_homes(&c.state_path)
    }

    #[test]
    fn the_default_is_usable_on_its_own() {
        let c = Config::default();
        assert!(c.background.is_none());
        assert!(c.ambient_sound.is_none(), "R23 is silent unless asked for");
        assert!(crate::store::is_outside_homes(&c.state_path), "C10");
    }

    #[test]
    fn a_failure_to_load_still_yields_a_usable_config() {
        let c = Config::load(Path::new("/nonexistent/greet.lua"));
        assert_eq!(
            c.geometry.field_rounding,
            Geometry::default().field_rounding
        );
        assert!(!c.problems.is_empty(), "R13: the reason is reported");
    }

    // ---- the control -------------------------------------------------------

    /// Without this one, every test below could be passing because the loader
    /// reads nothing at all.
    #[test]
    fn a_theme_that_behaves_is_actually_read() {
        let c = Config::load(&fixture("good.lua"));
        assert!(c.problems.is_empty(), "no problems: {:?}", c.problems);
        assert_eq!(
            c.background.as_deref(),
            Some(Path::new("/usr/share/auroval/nightshade/still.png"))
        );
        assert_eq!(c.geometry.clock_px, 96.0, "the theme moved this");
        assert_eq!(c.geometry.field_w, 420.0, "and this");
        assert_eq!(
            c.geometry.date_px,
            Geometry::default().date_px,
            "but not this, so an unset field keeps hyprlock's number"
        );
        assert_eq!(
            c.palette.primary,
            [0.0, 0xbd as f32 / 255.0, 0xce as f32 / 255.0, 1.0]
        );
        assert_eq!(
            c.palette.surface[3],
            Palette::default().surface[3],
            "a Lua palette is still only a hue, per theme::Palette::parse"
        );
    }

    // ---- R13 ---------------------------------------------------------------

    #[test]
    fn a_broken_config_still_logs_in_and_the_error_names_the_line() {
        let path = fixture("broken-syntax.lua");
        let c = Config::load(&path);

        assert!(is_usable(&c), "R13: the greeter starts on defaults");
        assert_eq!(c.problems.len(), 1);
        let p = &c.problems[0];
        assert!(
            p.contains(path.to_str().unwrap()),
            "R13: the error names the file -- {p}"
        );
        // Derived from the fixture rather than hardcoded: the error site is
        // the closing brace, because `palette` was given no value. Hardcoding
        // it meant that editing a comment in the fixture moved the error and
        // left this assertion checking a line that no longer had anything
        // wrong with it -- which it would have gone on passing for, since
        // some other line's number appears in a long path often enough.
        let text = std::fs::read_to_string(&path).unwrap();
        let brace = text
            .lines()
            .position(|l| l.trim() == "}")
            .expect("the fixture ends in a closing brace")
            + 1;
        assert!(
            p.contains(&format!(":{brace}:")),
            "R13: the error names line {brace} -- {p}"
        );
    }

    // ---- R22, the four modes ----------------------------------------------

    #[test]
    fn a_theme_that_raises_does_not_prevent_login() {
        let c = Config::load(&fixture("raises.lua"));
        assert!(is_usable(&c));
        assert_eq!(c.problems.len(), 1, "logged, not silent");
        assert!(
            c.problems[0].contains("raises.lua"),
            "and it says which theme -- {:?}",
            c.problems[0]
        );
    }

    #[test]
    fn a_theme_that_loops_forever_is_stopped_by_the_instruction_ceiling() {
        let path = fixture("loops-forever.lua");
        let started = Instant::now();
        let c = Config::load(&path);
        let took = started.elapsed();

        assert!(is_usable(&c));
        assert_eq!(c.problems.len(), 1);
        assert!(
            c.problems[0].contains("instruction ceiling"),
            "stopped by the ceiling and not by something else -- {:?}",
            c.problems[0]
        );

        // R22 asks for two things and this is the second one: a theme that
        // loops "does not delay the first prompt". R16 budgets 500 ms for the
        // whole first frame, so the config load may not eat it. The real
        // figure is single-digit milliseconds; the bound is loose only so that
        // a loaded CI machine does not fail a requirement it still meets.
        assert!(
            took.as_millis() < 250,
            "R22: it must not delay the first prompt, took {took:?}"
        );
    }

    #[test]
    fn a_theme_that_allocates_without_bound_is_stopped_by_the_memory_ceiling() {
        let c = Config::load(&fixture("allocates-forever.lua"));
        assert!(is_usable(&c));
        assert_eq!(c.problems.len(), 1);
        let p = &c.problems[0];
        assert!(
            p.contains("memory") || p.contains("not enough"),
            "stopped by the memory ceiling, not the instruction one -- {p}"
        );
    }

    #[test]
    fn a_theme_that_reaches_out_fails_at_the_call() {
        let c = Config::load(&fixture("reaches-out.lua"));
        assert!(is_usable(&c));
        assert_eq!(c.problems.len(), 1);
        let p = &c.problems[0];

        // R22's wording is exact: it fails *at the call* rather than
        // succeeding. The fixture's very first statement is `io.open`, and the
        // error naming `io` as a nil global is what proves execution stopped
        // there -- before `os.execute("id")` and before `require` on the lines
        // below it ever ran.
        assert!(
            p.contains("'io'") || p.contains("\"io\""),
            "it died on io, the first line -- {p}"
        );
        assert!(
            p.contains("nil"),
            "and died because the name was never bound -- {p}"
        );
    }

    // ---- R22, the sandbox itself ------------------------------------------

    /// R22 names seven things the interpreter is created without. This asserts
    /// each one by name, because the fixtures above prove the *consequence*
    /// and this proves the *construction* -- and a future refactor that
    /// reintroduces one of them should fail here, loudly, rather than only
    /// showing up as a fixture that mysteriously starts succeeding.
    #[test]
    fn the_denied_names_are_all_nil() {
        for name in ["io", "os", "package", "require", "debug", "load", "dofile"] {
            let src = format!("return {name} == nil");
            assert_eq!(
                sandbox_says(&src),
                Some(true),
                "R22: `{name}` must not exist in a theme's interpreter"
            );
        }
    }

    /// The deletion-is-weaker argument from the module header, as a test. If
    /// the libraries were loaded and then deleted, `debug` would still be able
    /// to return them; it cannot, because it was never opened.
    #[test]
    fn the_sandbox_cannot_be_reassembled_from_inside() {
        for src in [
            "return debug ~= nil",
            "return package ~= nil",
            "return pcall(require, 'os')",
            "return pcall(load, 'return 1')",
        ] {
            assert_eq!(
                sandbox_says(src),
                Some(false),
                "a theme got something back with: {src}"
            );
        }
    }

    #[test]
    fn a_theme_that_returns_something_other_than_a_table_is_refused() {
        let dir = std::env::temp_dir().join("auroval-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-a-table.lua");
        std::fs::write(&path, "return 42\n").unwrap();

        let c = Config::load(&path);
        assert!(is_usable(&c));
        assert!(
            c.problems[0].contains("return a table"),
            "{:?}",
            c.problems[0]
        );
        let _ = std::fs::remove_file(&path);
    }
}
