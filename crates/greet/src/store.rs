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
pub fn is_outside_homes(p: &Path) -> bool {
    !p.starts_with("/home") && !p.starts_with("/root")
}

#[cfg(test)]
mod tests {
    use super::*;

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
