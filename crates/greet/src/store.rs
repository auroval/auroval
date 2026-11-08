//! R4: remember the last user and the last session, per machine, outside any
//! home directory.
//!
//! "Outside any home" is C10, and it is not a style choice: the greeter runs
//! as its own user and must still work when `/home/<user>` is mode 700. So
//! this lives under `/var/lib/auroval`, owned by the greeter.
//!
//! Written atomically — a greeter killed mid-write must not leave a file that
//! makes the next login worse than no memory at all. The read side treats
//! every failure as "no memory yet", because a greeter that refuses to start
//! because it cannot remember your name is a worse greeter than one that
//! forgets.

use std::io::Write;
use std::path::{Path, PathBuf};

pub const DEFAULT_PATH: &str = "/var/lib/auroval/last-login";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LastLogin {
    pub user: Option<String>,
    pub session: Option<String>,
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Store { path: path.into() }
    }

    pub fn load(&self) -> LastLogin {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => parse(&text),
            Err(_) => LastLogin::default(),
        }
    }

    /// Returns the error rather than logging it, so the caller decides how
    /// loud to be. Failing to remember must never fail a login.
    pub fn save(&self, last: &LastLogin) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            writeln!(f, "# auroval greeter, R4. Rewritten on every login.")?;
            if let Some(u) = &last.user {
                writeln!(f, "user={u}")?;
            }
            if let Some(s) = &last.session {
                writeln!(f, "session={s}")?;
            }
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)
    }
}

fn parse(text: &str) -> LastLogin {
    let mut out = LastLogin::default();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            match k.trim() {
                "user" => out.user = Some(v.to_string()),
                "session" => out.session = Some(v.to_string()),
                _ => {}
            }
        }
    }
    out
}

/// Whether a path is inside anybody's home. Used to assert C10 in tests, and
/// cheap enough to assert at startup too.
///
/// Lexical, and therefore not enough on its own: `..` walks out of it and a
/// symlink walks around it. Callers that are about to *open* something go
/// through [`refuse_a_home`], which resolves first.
pub fn is_outside_homes(p: &Path) -> bool {
    !p.starts_with("/home") && !p.starts_with("/root")
}

/// R15 and C10 in one place: refuse a path that is, or resolves to, a place
/// inside a home directory.
///
/// `background::decode` has done this properly since it was written — check
/// lexically, canonicalise, check again — and nothing else did. An audit on
/// 2026-11-08 found the state file being read in `main` *before* the
/// refusal in `shell::run` could fire, `probe` reading **and writing** it
/// with no refusal at all, and `AUROVAL_CONFIG` never checked. Each of those
/// is the same three lines, which is the argument for having them once.
///
/// The resolving half matters because the lexical half is trivially evaded:
/// `/var/lib/auroval/../../home/dirk/x` begins with `/var`. A path that does
/// not exist yet is normal here — the state file is created on first login —
/// so this resolves the nearest ancestor that does exist and re-checks with
/// the rest appended.
/// Resolve `.` and `..` without touching the filesystem.
///
/// Not a substitute for `canonicalize` -- it does not follow symlinks, and
/// for a path that crosses one the two disagree. It runs first precisely so
/// that `canonicalize` gets something it can be asked about.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                // Popping past the root leaves the root, as the kernel does.
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

pub fn refuse_a_home(path: &Path, what: &str) -> Result<(), String> {
    let say = |p: &Path| {
        format!(
            "refusing {what} at {} — it is inside a home directory, and the greeter runs as its own user which cannot read one (R15, C10)",
            p.display()
        )
    };
    // Lexically first, because `..` has to go before anything else looks at
    // the path. The walk below asks for each ancestor's `file_name`, and a
    // component that is `..` has none -- so an unnormalised path quietly
    // loses it and `/var/lib/auroval/../../home/dirk/x` comes back out as
    // `/var/lib/auroval/home/dirk/x`, which passes. That was the first
    // version of this function, and the test named for it caught it.
    let path = &normalise(path);
    if !is_outside_homes(path) {
        return Err(say(path));
    }
    // Walk up to something that exists. `canonicalize` fails on a missing
    // path, and "the file is not there yet" is not a C10 violation. Nor is
    // "the greeter cannot stat its own directory as you", which is why this
    // keeps walking rather than giving up at the first unreadable ancestor.
    let mut existing = path.as_path();
    let mut rest = std::path::PathBuf::new();
    loop {
        if existing.exists() {
            break;
        }
        let Some(parent) = existing.parent() else {
            // Nothing in the chain exists; the lexical check above is all
            // there is to go on, and it passed.
            return Ok(());
        };
        rest = match existing.file_name() {
            Some(name) => Path::new(name).join(&rest),
            None => rest,
        };
        existing = parent;
    }
    let Ok(real) = existing.canonicalize() else {
        return Ok(());
    };
    let real = real.join(&rest);
    if !is_outside_homes(&real) {
        return Err(format!(
            "refusing {what} at {} — it resolves to {}, inside a home directory the greeter cannot read (R15, C10)",
            path.display(),
            real.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The state file does not exist before the first login, so "missing" has
    /// to be allowed or the greeter refuses its own default path.
    #[test]
    fn a_path_that_does_not_exist_yet_is_fine() {
        assert!(refuse_a_home(Path::new("/var/lib/auroval/last-login"), "x").is_ok());
        assert!(refuse_a_home(Path::new("/var/lib/auroval/nope/nor/this"), "x").is_ok());
    }

    #[test]
    fn a_home_is_refused_and_the_message_says_which_rule() {
        let e = refuse_a_home(Path::new("/home/someone/last-login"), "the state file")
            .expect_err("a home is refused");
        assert!(e.contains("the state file"), "{e}");
        assert!(e.contains("R15"), "{e}");
        assert!(refuse_a_home(Path::new("/root/last-login"), "x").is_err());
    }

    /// The half the lexical check cannot do. `/var/...` passes
    /// `starts_with`, and resolving is the only thing that catches it.
    #[test]
    fn dot_dot_out_of_a_public_directory_is_caught() {
        let home = std::env::var("HOME").expect("a home to walk into");
        // Three, not two: /var/lib/auroval/../.. is /var, and a test that
        // lands on /var/home/dirk would pass for the wrong reason.
        let escape = PathBuf::from("/var/lib/auroval/../../..")
            .join(home.trim_start_matches('/'))
            .join("not-a-real-file");
        assert!(
            is_outside_homes(&escape),
            "the lexical check is supposed to be fooled by this; if it is not, this test proves nothing"
        );
        assert!(refuse_a_home(&escape, "x").is_err(), "{}", escape.display());
    }

    #[test]
    fn a_symlink_into_a_home_is_caught_too() {
        let link = std::env::temp_dir().join(format!("greet-c10-{}", std::process::id()));
        let _ = std::fs::remove_file(&link);
        let home = std::env::var("HOME").expect("a home to point at");
        std::os::unix::fs::symlink(&home, &link).expect("symlink");
        let through = link.join("last-login");
        assert!(is_outside_homes(&through), "lexically this looks like /tmp");
        let got = refuse_a_home(&through, "x");
        let _ = std::fs::remove_file(&link);
        assert!(got.is_err(), "a symlink into a home has to be refused");
    }

    #[test]
    fn round_trips() {
        let dir = std::env::temp_dir().join(format!("greet-store-{}", std::process::id()));
        let store = Store::at(dir.join("last-login"));
        let want = LastLogin {
            user: Some("dirk".into()),
            session: Some("hyprland-uwsm".into()),
        };
        store.save(&want).expect("saves");
        assert_eq!(store.load(), want);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_file_is_simply_no_memory() {
        let store = Store::at("/nonexistent/auroval/last-login");
        assert_eq!(store.load(), LastLogin::default());
    }

    #[test]
    fn rubbish_in_the_file_does_not_panic_or_half_apply() {
        assert_eq!(
            parse("user=\nsession=hyprland\n!!!\n= =\n"),
            LastLogin {
                user: None,
                session: Some("hyprland".into())
            }
        );
    }

    #[test]
    fn the_default_path_honours_c10() {
        assert!(is_outside_homes(Path::new(DEFAULT_PATH)));
        assert!(!is_outside_homes(Path::new("/home/dirk/.cache/greet")));
    }
}
