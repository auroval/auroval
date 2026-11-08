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

/// The tests `auth.rs` went without until a console sitting proved it needed
/// them.
///
/// This module had neither a test nor a real run behind it through all of P1,
/// on the reasoning that it only makes sense against a real greetd. That was
/// wrong twice over. greetd's half of the conversation is a few JSON frames
/// over a Unix socket, so a scripted server is a dozen lines — and the thing
/// the sitting found, an `auth_error` that never reaches the screen, is
/// exactly what a scripted server can reproduce in milliseconds and a console
/// takes an afternoon to show once.
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc::{channel, RecvTimeoutError};
    use std::sync::Mutex;
    use std::time::Duration;

    /// `Connection::from_env` reads `GREETD_SOCK`, and an environment is one
    /// per process however many threads are looking at it.
    static ENV: Mutex<()> = Mutex::new(());

    /// A greetd that says what it is told to, in order.
    ///
    /// It does not parse the requests. What is being tested is what the
    /// greeter does with the *replies*, and a server that only counts frames
    /// cannot accidentally become the thing under test.
    fn fake_greetd(
        replies: Vec<&'static str>,
    ) -> (std::path::PathBuf, std::thread::JoinHandle<usize>) {
        let path = std::env::temp_dir().join(format!(
            "auroval-fake-greetd-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("bind the fake greetd");
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().expect("the greeter connects");
            let mut served = 0usize;
            for reply in replies {
                let mut len = [0u8; 4];
                if sock.read_exact(&mut len).is_err() {
                    break;
                }
                let mut payload = vec![0u8; u32::from_ne_bytes(len) as usize];
                if sock.read_exact(&mut payload).is_err() {
                    break;
                }
                let body = reply.as_bytes();
                let mut frame = (body.len() as u32).to_ne_bytes().to_vec();
                frame.extend_from_slice(body);
                if sock.write_all(&frame).is_err() {
                    break;
                }
                let _ = sock.flush();
                served += 1;
            }
            served
        });
        (path, handle)
    }

    const PASSWORD_PROMPT: &str =
        r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password:"}"#;
    const AUTH_ERROR: &str =
        r#"{"type":"error","error_type":"auth_error","description":"Authentication failure"}"#;
    const OK: &str = r#"{"type":"success"}"#;

    fn next(rx: &Receiver<Ui>) -> Ui {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(u) => u,
            Err(RecvTimeoutError::Timeout) => panic!("the conversation said nothing for 5 s"),
            Err(e) => panic!("the conversation ended: {e}"),
        }
    }

    /// The finding from the console sitting of 2026-11-08, as a test.
    ///
    /// Forty-eight captured frames of a real login contained no error at all:
    /// a wrong password threw the one-time password away and asked again with
    /// nothing said. R3's fit criterion is three clauses and this is the one
    /// that was failing.
    #[test]
    fn a_wrong_password_produces_a_message_before_the_conversation_restarts() {
        let _guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (path, server) = fake_greetd(vec![
            PASSWORD_PROMPT, // reply to create_session
            AUTH_ERROR,      // reply to the answer: wrong
            OK,              // reply to cancel_session
            PASSWORD_PROMPT, // reply to the second create_session
        ]);
        std::env::set_var("GREETD_SOCK", &path);

        let (to_ui, from_auth) = channel();
        let (to_auth, from_ui) = channel();
        let joined = spawn("dirk".into(), vec!["/bin/true".into()], to_ui, from_ui);

        match next(&from_auth) {
            Ui::Prompt { text, secret } => {
                assert_eq!(text, "Password:");
                assert!(secret, "a password prompt is a secret one");
            }
            other => panic!("expected the first prompt, got {other:?}"),
        }
        to_auth
            .send(Answer::Text("wrong-on-purpose".into()))
            .unwrap();

        match next(&from_auth) {
            Ui::Message { text, is_error } => {
                assert!(is_error, "a wrong password is an error, not a notice");
                assert_eq!(text, "Authentication failure");
            }
            other => {
                panic!("a wrong password must say so before anything else happens; got {other:?}")
            }
        }

        match next(&from_auth) {
            Ui::Prompt { text, .. } => assert_eq!(text, "Password:", "and then ask again"),
            other => panic!("expected the conversation to restart, got {other:?}"),
        }

        drop(to_auth);
        let _ = joined.join();
        assert_eq!(
            server.join().unwrap(),
            4,
            "the server served its whole script"
        );
        let _ = std::fs::remove_file(&path);
        std::env::remove_var("GREETD_SOCK");
    }
}
