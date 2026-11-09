//! The seam between the conversation and the picture.
//!
//! `UiState` is everything the screen needs to know and nothing about how it
//! is drawn. That split is deliberate and load-bearing: the view (issue for
//! piece B) can be written and tested against this type by rendering to an
//! offscreen buffer, with no Wayland, no greetd and no working greeter.
//!
//! It is also where R1 is expressed as a type. The greeter never decides what
//! to ask — it renders `prompt` and `notice` as they arrive, whatever they
//! are, however many there are.

use crate::sessions::Session;

/// What the greeter is doing. One field per thing the eye needs.
#[derive(Debug, Clone)]
pub struct UiState {
    /// Which step of the login this is.
    pub stage: Stage,
    /// The name being offered or typed. Pre-filled from R4's memory.
    pub username: String,
    /// What greetd last asked, verbatim. Empty when it has asked nothing.
    pub prompt: String,
    /// Whether what is typed should be shown as dots.
    pub secret: bool,
    /// What has been typed in answer to `prompt`.
    pub typed: String,
    /// `info` and `error` messages, and the greeter's own. R17 wants these
    /// rendered as content, not squeezed into a status line.
    pub notice: Option<Notice>,
    /// Every session this machine offers, and which one is selected.
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub caps_lock: bool,
    /// Shown by the clock and the date line. Supplied by the shell so the
    /// view stays a pure function of its input.
    pub clock: String,
    pub date: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Asking who you are. The field holds `username`.
    AskingUser,
    /// greetd has asked something. The field holds `typed`.
    Answering,
    /// Authenticated; the session is starting. Nothing more to type.
    Starting,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub text: String,
    pub kind: NoticeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Error,
}

impl UiState {
    pub fn new(sessions: Vec<Session>, last: &crate::store::LastLogin) -> UiState {
        // R4: the remembered session is matched by id, not by name — the name
        // is translated and the id is not.
        let selected = last
            .session
            .as_deref()
            .and_then(|want| sessions.iter().position(|s| s.id == want))
            .unwrap_or(0);
        UiState {
            stage: Stage::AskingUser,
            username: last.user.clone().unwrap_or_default(),
            prompt: String::new(),
            // **True, not false, and the default is the point.** This was
            // `false` until 2026-11-09, and `secret` only became true when a
            // prompt arrived saying so. Between submitting a username and
            // greetd's first prompt landing, the stage is already
            // `Answering` and the field was therefore drawn in PLAIN TEXT --
            // milliseconds on a warm path, longer on a slow PAM stack, and
            // nobody waits for a prompt before typing a password they have
            // typed a thousand times. Found by an adversarial review.
            //
            // A field is now secret unless something says otherwise, so the
            // failure mode of a missing message is a hidden field rather than
            // a visible password. `auth.rs` still sets it per prompt, which
            // is what makes a `visible` prompt visible.
            secret: true,
            typed: String::new(),
            notice: None,
            sessions,
            selected,
            caps_lock: false,
            clock: String::new(),
            date: String::new(),
        }
    }

    /// What the input field currently contains, whichever stage it is.
    pub fn field(&self) -> &str {
        match self.stage {
            Stage::AskingUser => &self.username,
            _ => &self.typed,
        }
    }

    /// What to show in the field: the text itself, or one dot per character.
    /// Counted in characters rather than bytes, so a non-ASCII password does
    /// not leak its length in UTF-8 bytes.
    ///
    /// **An empty field is empty, not zero dots.** It used to return
    /// `Dots(0)` for a secret field nobody had typed into yet, and `dots`
    /// draws nothing for zero — so the field rendered completely blank and
    /// the prompt went with it. For `Password` that is survivable, because
    /// everybody knows what the box under their name wants. For the one-time
    /// password a PAM stack asked for on 2026-11-08 it was not: the operator
    /// sat in front of an empty rectangle for two minutes with no way to know
    /// what it wanted. R1 is the requirement that the greeter renders
    /// whatever the conversation asks, and a question drawn as nothing is not
    /// rendering it.
    pub fn field_display(&self) -> FieldDisplay<'_> {
        // `secret` can still be set from a previous prompt when the
        // conversation has gone back to asking who you are, so the stage
        // decides whether anything is hidden at all. The existing
        // `the_username_field_is_never_dotted` caught a first version of
        // this that got the order wrong and blanked the username.
        let hidden = self.stage != Stage::AskingUser && self.secret;
        if hidden && !self.typed.is_empty() {
            FieldDisplay::Dots(self.typed.chars().count())
        } else if hidden {
            // Nothing to hide yet, and the prompt is what belongs here.
            FieldDisplay::Text("")
        } else {
            FieldDisplay::Text(self.field())
        }
    }

    /// Put a notice on the screen, keeping an error that has not been
    /// answered yet.
    ///
    /// **An error must outlive the messages that follow it.** The shell
    /// drains everything the conversation has said since the last frame, and
    /// after a wrong password greetd sends the error and then immediately
    /// re-opens the conversation — whose first PAM module spoke again within
    /// milliseconds. Last-one-wins meant the error was replaced before a
    /// single frame was drawn. Forty-eight captured frames of a real login
    /// on 2026-11-08 contain no error at all for exactly this reason, and
    /// the person who typed the wrong password was never told.
    ///
    /// So an error stays until the person does something about it, which is
    /// `submit` clearing it. Another error still replaces it: the newest
    /// thing that went wrong is the useful one.
    pub fn notify(&mut self, n: Notice) {
        let holding_an_error = matches!(
            &self.notice,
            Some(Notice {
                kind: NoticeKind::Error,
                ..
            })
        );
        if holding_an_error && n.kind != NoticeKind::Error {
            return;
        }
        self.notice = Some(n);
    }

    pub fn session(&self) -> Option<&Session> {
        self.sessions.get(self.selected)
    }

    pub fn cycle_session(&mut self, forward: bool) {
        if self.sessions.is_empty() {
            return;
        }
        let n = self.sessions.len();
        self.selected = if forward {
            (self.selected + 1) % n
        } else {
            (self.selected + n - 1) % n
        };
    }

    pub fn push(&mut self, c: char) {
        match self.stage {
            Stage::AskingUser => self.username.push(c),
            Stage::Answering => self.typed.push(c),
            Stage::Starting => {}
        }
    }

    /// Empty whichever field `push` has been filling.
    ///
    /// Added because the locker cleared `typed` while its keystrokes were
    /// going into `username`: its stage never leaves `AskingUser`, so every
    /// denied attempt left its placeholder characters in the field and the
    /// screen showed the running total of every password tried that sitting.
    /// A length is not a secret, but it is not nothing either, and a field
    /// that will not clear is a bug whichever way you look at it.
    pub fn clear_field(&mut self) {
        match self.stage {
            Stage::AskingUser => self.username.clear(),
            Stage::Answering => self.typed.clear(),
            Stage::Starting => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.stage {
            Stage::AskingUser => {
                self.username.pop();
            }
            Stage::Answering => {
                self.typed.pop();
            }
            Stage::Starting => {}
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum FieldDisplay<'a> {
    Text(&'a str),
    Dots(usize),
}

#[cfg(test)]
mod tests {
    /// The regression the default exists for: `secret` was `false` until a
    /// prompt said otherwise, so the window between submitting a username
    /// and greetd's first prompt drew the field in plain text — and nobody
    /// waits for a prompt before typing a password they know by heart.
    #[test]
    fn a_field_is_secret_before_anything_says_so() {
        let mut ui = UiState::new(Vec::new(), &crate::store::LastLogin::default());
        ui.stage = Stage::Answering;
        ui.push('a');
        ui.push('b');
        assert!(
            matches!(ui.field_display(), FieldDisplay::Dots(2)),
            "a field nothing has described yet must be hidden, not shown"
        );
    }

    /// `clear_field` empties what `push` filled, in every stage. The locker
    /// cleared `typed` while its keystrokes went into `username`, so the
    /// field showed the running total of every attempt that sitting.
    #[test]
    fn clearing_a_field_empties_the_one_push_fills() {
        for stage in [Stage::AskingUser, Stage::Answering] {
            let mut ui = UiState::new(Vec::new(), &crate::store::LastLogin::default());
            ui.stage = stage;
            ui.push('x');
            assert_eq!(ui.field().chars().count(), 1, "{stage:?}");
            ui.clear_field();
            assert!(ui.field().is_empty(), "{stage:?} did not clear");
        }
    }

    use super::*;
    use crate::sessions::Kind;
    use crate::store::LastLogin;
    use std::path::PathBuf;

    fn session(id: &str) -> Session {
        Session {
            id: id.into(),
            name: id.into(),
            comment: None,
            command: vec![id.into()],
            kind: Kind::Wayland,
            path: PathBuf::from(format!("/usr/share/wayland-sessions/{id}.desktop")),
        }
    }

    #[test]
    fn the_remembered_session_is_preselected_by_id() {
        let s = vec![session("hyprland"), session("hyprland-uwsm")];
        let last = LastLogin {
            user: Some("dirk".into()),
            session: Some("hyprland-uwsm".into()),
        };
        let ui = UiState::new(s, &last);
        assert_eq!(ui.session().map(|s| s.id.as_str()), Some("hyprland-uwsm"));
        assert_eq!(ui.username, "dirk");
    }

    #[test]
    fn a_session_that_no_longer_exists_falls_back_rather_than_failing() {
        let last = LastLogin {
            user: None,
            session: Some("sway".into()),
        };
        let ui = UiState::new(vec![session("hyprland")], &last);
        assert_eq!(ui.selected, 0);
    }

    #[test]
    fn a_secret_field_shows_characters_not_bytes() {
        let mut ui = UiState::new(vec![], &LastLogin::default());
        ui.stage = Stage::Answering;
        ui.secret = true;
        ui.typed = "wachtwoordé".into();
        assert_eq!(ui.field_display(), FieldDisplay::Dots(11));
    }

    /// The failure this was written against: an empty secret field drew
    /// nothing at all, prompt included.
    fn notice(text: &str, kind: NoticeKind) -> Notice {
        Notice {
            text: text.into(),
            kind,
        }
    }

    /// The console sitting's finding, as a unit test. After a wrong password
    /// greetd sends the error and then re-opens the conversation, whose
    /// first PAM module spoke again milliseconds later — and the shell drains
    /// both before it draws. Last-one-wins made the error invisible.
    #[test]
    fn an_error_is_not_replaced_by_the_chatter_that_follows_it() {
        let mut ui = UiState::new(vec![], &LastLogin::default());
        ui.notify(notice("Authentication failure", NoticeKind::Error));
        ui.notify(notice(
            "an extra PAM step is in this stack",
            NoticeKind::Info,
        ));
        assert_eq!(
            ui.notice.as_ref().map(|n| n.text.as_str()),
            Some("Authentication failure"),
            "the error has to survive long enough to be read"
        );
    }

    #[test]
    fn a_newer_error_does_replace_an_older_one() {
        let mut ui = UiState::new(vec![], &LastLogin::default());
        ui.notify(notice("first", NoticeKind::Error));
        ui.notify(notice("second", NoticeKind::Error));
        assert_eq!(ui.notice.as_ref().map(|n| n.text.as_str()), Some("second"));
    }

    #[test]
    fn an_info_replaces_an_info() {
        let mut ui = UiState::new(vec![], &LastLogin::default());
        ui.notify(notice("first", NoticeKind::Info));
        ui.notify(notice("second", NoticeKind::Info));
        assert_eq!(ui.notice.as_ref().map(|n| n.text.as_str()), Some("second"));
    }

    #[test]
    fn an_empty_secret_field_shows_the_prompt_and_not_zero_dots() {
        let mut ui = UiState::new(vec![session("hyprland")], &LastLogin::default());
        ui.stage = Stage::Answering;
        ui.secret = true;
        ui.prompt = "One-time password (OATH) :".into();
        assert_eq!(
            ui.field_display(),
            FieldDisplay::Text(""),
            "an empty secret field must reach the placeholder path, or the \
             prompt is never drawn"
        );
        ui.push('1');
        assert_eq!(
            ui.field_display(),
            FieldDisplay::Dots(1),
            "once there is something to hide, hide it"
        );
    }

    #[test]
    fn the_username_field_is_never_dotted() {
        let mut ui = UiState::new(vec![], &LastLogin::default());
        ui.secret = true; // even if a previous prompt was a secret
        ui.username = "dirk".into();
        assert_eq!(ui.field_display(), FieldDisplay::Text("dirk"));
    }

    #[test]
    fn cycling_wraps_both_ways_and_survives_an_empty_list() {
        let mut ui = UiState::new(vec![session("a"), session("b")], &LastLogin::default());
        ui.cycle_session(false);
        assert_eq!(ui.selected, 1);
        ui.cycle_session(true);
        assert_eq!(ui.selected, 0);

        let mut empty = UiState::new(vec![], &LastLogin::default());
        empty.cycle_session(true);
        assert_eq!(empty.selected, 0);
    }
}
