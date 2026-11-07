//! R1, R3, R18: drive greetd's conversation, assuming nothing about it.
//!
//! The shape is taken from the P0 spike, which demonstrated it against a real
//! greetd, and the reasoning is worth repeating because it is the requirement:
//!
//!   * **R1 — no assumptions.** greetd relays PAM. A stack with an extra
//!     `pam_exec` prompt, or two prompts, or none, must work with no code
//!     change. So this is a loop over whatever arrives, not a password field
//!     with a login button next to it.
//!   * **R3 — `auth_error` is not fatal.** A wrong password cancels the
//!     session and starts a new one; the process does not exit.
//!   * **R18 — no deadline of our own.** This blocks on the socket
//!     indefinitely, which is why it is not the UI thread: a device-code flow
//!     can idle for minutes while the background keeps moving.

use std::sync::mpsc::{Receiver, Sender};

use greetd_proto::{AuthMessageType, Connection, ErrorType, Request, Response};

/// What the conversation wants shown. The UI renders what arrives and decides
/// nothing: that is R1 expressed as a type.
#[derive(Debug, Clone)]
pub enum Ui {
    Prompt {
        text: String,
        secret: bool,
    },
    Message {
        text: String,
        is_error: bool,
    },
    /// Authentication succeeded and the session is being started.
    Starting,
    /// This conversation is over and the UI should return to asking who you
    /// are. Not fatal to the process.
    Ended(String),
}

#[derive(Debug)]
pub enum Answer {
    Text(String),
    Cancel,
}

/// Run one conversation for `username` on its own thread.
pub fn spawn(
    username: String,
    session_cmd: Vec<String>,
    to_ui: Sender<Ui>,
    from_ui: Receiver<Answer>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let fail = |to_ui: &Sender<Ui>, why: String| {
            let _ = to_ui.send(Ui::Ended(why));
        };

        let mut conn = match Connection::from_env() {
            Ok(c) => c,
            Err(e) => return fail(&to_ui, e.to_string()),
        };
        let mut reply = match conn.round_trip(&Request::CreateSession {
            username: username.clone(),
        }) {
            Ok(r) => r,
            Err(e) => return fail(&to_ui, e.to_string()),
        };

        loop {
            match reply {
                Response::AuthMessage {
                    auth_message_type,
                    auth_message,
                } => {
                    let next = if auth_message_type.wants_response() {
                        let _ = to_ui.send(Ui::Prompt {
                            text: auth_message,
                            secret: auth_message_type == AuthMessageType::Secret,
                        });
                        match from_ui.recv() {
                            Ok(Answer::Text(t)) => {
                                Request::PostAuthMessageResponse { response: Some(t) }
                            }
                            Ok(Answer::Cancel) | Err(_) => Request::CancelSession,
                        }
                    } else {
                        // `info` and `error` are statements, not questions.
                        // Show them and answer with no response at all, which
                        // is what makes a device-code prompt work (R17).
                        let _ = to_ui.send(Ui::Message {
                            text: auth_message,
                            is_error: auth_message_type == AuthMessageType::Error,
                        });
                        Request::PostAuthMessageResponse { response: None }
                    };

                    let cancelled = matches!(next, Request::CancelSession);
                    reply = match conn.round_trip(&next) {
                        Ok(r) => r,
                        Err(e) => return fail(&to_ui, e.to_string()),
                    };
                    if cancelled {
                        return fail(&to_ui, "cancelled".into());
                    }
                }

                Response::Success => {
                    let _ = to_ui.send(Ui::Starting);
                    return match conn.round_trip(&Request::StartSession {
                        cmd: session_cmd.clone(),
                        env: vec![],
                    }) {
                        Ok(Response::Success) => (),
                        Ok(Response::Error { description, .. }) => fail(&to_ui, description),
                        Ok(other) => fail(&to_ui, format!("unexpected: {other:?}")),
                        Err(e) => fail(&to_ui, e.to_string()),
                    };
                }

                Response::Error {
                    error_type,
                    description,
                } => match error_type {
                    // R3: a wrong password is not an exit. Cancel, say so, and
                    // start the conversation again from the top.
                    ErrorType::AuthError => {
                        let _ = to_ui.send(Ui::Message {
                            text: description,
                            is_error: true,
                        });
                        let _ = conn.round_trip(&Request::CancelSession);
                        reply = match conn.round_trip(&Request::CreateSession {
                            username: username.clone(),
                        }) {
                            Ok(r) => r,
                            Err(e) => return fail(&to_ui, e.to_string()),
                        };
                    }
                    ErrorType::Error => return fail(&to_ui, description),
                },
            }
        }
    })
}
