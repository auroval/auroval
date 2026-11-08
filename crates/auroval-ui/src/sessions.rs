//! R2: the sessions this machine offers, and what each one starts.
//!
//! Desktop entries, read from the directories a Wayland login manager is
//! expected to read. The spike hardcoded a command and started a shell script,
//! which came back as `XDG_SESSION_TYPE=tty` — proof that login worked, and no
//! evidence at all about sessions.
//!
//! The parser is deliberately small and deliberately forgiving: a malformed
//! `.desktop` file on a user's machine must cost that one entry and nothing
//! else. R13's rule is about configuration, but the spirit is the same — the
//! greeter's job is to let you in.

use std::path::{Path, PathBuf};

/// Where desktop entries live, most specific first. `/usr/local` wins over
/// `/usr` by the same convention the rest of the system uses.
pub const WAYLAND_DIRS: &[&str] = &[
    "/usr/local/share/wayland-sessions",
    "/usr/share/wayland-sessions",
];
pub const X11_DIRS: &[&str] = &["/usr/local/share/xsessions", "/usr/share/xsessions"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The entry's file name without `.desktop`: stable, and what gets stored
    /// as "last session" rather than the display name, which is translated.
    pub id: String,
    pub name: String,
    pub comment: Option<String>,
    /// Argv to hand greetd. Already split; `Exec` field codes are stripped.
    pub command: Vec<String>,
    pub kind: Kind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Wayland,
    X11,
}

/// Every session on this machine, Wayland first, each id appearing once.
pub fn discover() -> Vec<Session> {
    let mut out: Vec<Session> = Vec::new();
    for (dirs, kind) in [(WAYLAND_DIRS, Kind::Wayland), (X11_DIRS, Kind::X11)] {
        for dir in dirs {
            collect(Path::new(dir), kind, &mut out);
        }
    }
    out
}

fn collect(dir: &Path, kind: Kind, out: &mut Vec<Session>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return, // a missing directory is the normal case, not an error
    };
    let mut found: Vec<Session> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if let Some(s) = parse(&text, &path, kind) {
            found.push(s);
        }
    }
    // Stable order: the compositor list should not shuffle between boots.
    found.sort_by(|a, b| a.name.cmp(&b.name));
    for s in found {
        if !out.iter().any(|existing| existing.id == s.id) {
            out.push(s);
        }
    }
}

/// Parse one desktop entry. `None` when it is not a usable session.
pub fn parse(text: &str, path: &Path, kind: Kind) -> Option<Session> {
    let id = path.file_stem()?.to_str()?.to_string();
    let mut name = None;
    let mut comment = None;
    let mut exec = None;
    let mut try_exec = None;
    let mut hidden = false;
    let mut in_entry = false;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            // Only the main group counts; actions and other groups carry their
            // own Name and Exec and would otherwise overwrite the real ones.
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let (key, value) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        match key {
            // Localised keys look like `Name[nl]`. The greeter has no locale of
            // its own, so the untranslated key is the one that is read.
            "Name" => name = Some(value.to_string()),
            "Comment" => comment = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "TryExec" => try_exec = Some(value.to_string()),
            "Hidden" | "NoDisplay" => hidden |= value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    if hidden {
        return None;
    }
    let exec = exec?;
    if let Some(t) = try_exec {
        // The spec says a TryExec that does not resolve means the entry should
        // not be shown. Honouring it keeps dead compositors out of the list.
        if !resolves(&t) {
            return None;
        }
    }
    let command = split_exec(&exec);
    if command.is_empty() {
        return None;
    }
    Some(Session {
        name: name.unwrap_or_else(|| id.clone()),
        id,
        comment,
        command,
        kind,
        path: path.to_path_buf(),
    })
}

fn resolves(prog: &str) -> bool {
    let p = Path::new(prog);
    if p.is_absolute() {
        return p.is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(prog).is_file()))
        .unwrap_or(false)
}

/// Split an `Exec` value the way the desktop entry spec asks: quoted
/// arguments survive, and the `%f %F %u %U %i %c %k` field codes are dropped
/// because none of them means anything for a session.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut escape = false;
    let mut any = false;

    for ch in exec.chars() {
        if escape {
            cur.push(ch);
            escape = false;
            any = true;
            continue;
        }
        match ch {
            '\\' if quoted => escape = true,
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out.retain(|a| !matches!(a.as_str(), "%f" | "%F" | "%u" | "%U" | "%i" | "%c" | "%k"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> PathBuf {
        PathBuf::from("/usr/share/wayland-sessions/hyprland.desktop")
    }

    #[test]
    fn reads_a_normal_entry() {
        let s = parse(
            "[Desktop Entry]\nName=Hyprland\nComment=A tiling compositor\nExec=Hyprland\nType=Application\n",
            &p(),
            Kind::Wayland,
        )
        .expect("parses");
        assert_eq!(s.id, "hyprland");
        assert_eq!(s.name, "Hyprland");
        assert_eq!(s.command, vec!["Hyprland"]);
        assert_eq!(s.comment.as_deref(), Some("A tiling compositor"));
    }

    #[test]
    fn uwsm_style_exec_keeps_its_arguments() {
        let s = parse(
            "[Desktop Entry]\nName=Hyprland (uwsm)\nExec=uwsm start -- hyprland.desktop\n",
            &p(),
            Kind::Wayland,
        )
        .expect("parses");
        assert_eq!(s.command, vec!["uwsm", "start", "--", "hyprland.desktop"]);
    }

    #[test]
    fn hidden_entries_are_not_offered() {
        assert!(parse(
            "[Desktop Entry]\nName=X\nExec=x\nNoDisplay=true\n",
            &p(),
            Kind::Wayland
        )
        .is_none());
    }

    #[test]
    fn other_groups_do_not_overwrite_the_main_one() {
        let s = parse(
            "[Desktop Entry]\nName=Real\nExec=real\n\n[Desktop Action new]\nName=Fake\nExec=fake\n",
            &p(),
            Kind::Wayland,
        )
        .expect("parses");
        assert_eq!(s.name, "Real");
        assert_eq!(s.command, vec!["real"]);
    }

    #[test]
    fn an_entry_with_no_exec_is_not_a_session() {
        assert!(parse("[Desktop Entry]\nName=X\n", &p(), Kind::Wayland).is_none());
    }

    #[test]
    fn quotes_and_field_codes() {
        assert_eq!(
            split_exec(r#"env FOO="a b" start %U --flag"#),
            vec!["env", "FOO=a b", "start", "--flag"]
        );
    }
}
