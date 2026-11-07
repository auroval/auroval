//! The greetd IPC protocol, implemented from the `greetd-ipc(7)` manual page.
//!
//! This exists so that auroval does not depend on the `greetd_ipc` crate, which
//! is GPL-3.0-only (verified in its own `Cargo.toml`) and would force the same
//! licence on this project. The protocol is small enough that reimplementing it
//! is cheaper than the licence constraint. See PRD C1 and Q2/Q3.
//!
//! Wire format, quoting the manual page: `<payload-length> <payload>`, where
//! the length is "a 32-bit integer in native byte order" and the payload is "a
//! UTF-8-encoded JSON string". The socket path comes from `GREETD_SOCK`.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Requests a greeter may send. Four of them, per C1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Begin a session for `username`. The reply is the first `auth_message`,
    /// or `success` if the stack needs nothing.
    CreateSession { username: String },
    /// Answer the outstanding `auth_message`. `None` means "no answer", which
    /// is the correct response to an `info` or `error` message.
    PostAuthMessageResponse {
        #[serde(skip_serializing_if = "Option::is_none")]
        response: Option<String>,
    },
    /// Start the authenticated session. Only valid after `success`.
    StartSession {
        cmd: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        env: Vec<String>,
    },
    /// Abandon the session in progress. Always valid.
    CancelSession,
}

/// Responses greetd may send. Three of them, per C1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Success,
    Error {
        error_type: ErrorType,
        description: String,
    },
    AuthMessage {
        auth_message_type: AuthMessageType,
        auth_message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// Authentication failed. Per R3 this is **not** fatal: show it and loop.
    AuthError,
    /// Anything else — a protocol or internal error.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMessageType {
    /// Prompt whose answer may be echoed (a username).
    Visible,
    /// Prompt whose answer must not be echoed (a password).
    Secret,
    /// Not a prompt. Information to display; answer with `None`.
    /// R17 makes this first-class content — a device code lands here.
    Info,
    /// Not a prompt. An error to display; answer with `None`.
    Error,
}

impl AuthMessageType {
    /// Whether this message expects an answer at all. C2 warns against assuming
    /// anything about messages; this is the one safe inference, since `info`
    /// and `error` are statements rather than questions.
    pub fn wants_response(self) -> bool {
        matches!(self, Self::Visible | Self::Secret)
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Json(serde_json::Error),
    /// A frame claimed a length we refuse to allocate.
    FrameTooLarge(u32),
    /// `GREETD_SOCK` was not set, so we are not running under greetd.
    NoSocket,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "greetd socket I/O: {e}"),
            Self::Json(e) => write!(f, "greetd payload JSON: {e}"),
            Self::FrameTooLarge(n) => write!(f, "greetd frame claims {n} bytes; refusing"),
            Self::NoSocket => write!(f, "GREETD_SOCK is not set; not running under greetd"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Frames larger than this are a bug or an attack, not a message. The protocol
/// carries prompts and session lists; nothing legitimate approaches this.
const MAX_FRAME: u32 = 1 << 20;

/// A synchronous greetd connection.
///
/// The protocol is strictly request/response, so a blocking round trip is the
/// honest model. R18 forbids imposing our own deadline on an auth conversation,
/// so no read timeout is set: a device-code flow (C13) may idle for minutes and
/// must still complete. That means the caller has to drive this off the UI
/// thread.
pub struct Connection {
    sock: UnixStream,
}

impl Connection {
    /// Connect using the path in `GREETD_SOCK`.
    pub fn from_env() -> Result<Self, Error> {
        let path = std::env::var_os("GREETD_SOCK").ok_or(Error::NoSocket)?;
        Self::connect(path)
    }

    pub fn connect(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(Self {
            sock: UnixStream::connect(path.as_ref())?,
        })
    }

    /// Send a request and block for its response.
    pub fn round_trip(&mut self, req: &Request) -> Result<Response, Error> {
        self.write(req)?;
        self.read()
    }

    fn write(&mut self, req: &Request) -> Result<(), Error> {
        let payload = serde_json::to_vec(req)?;
        let len = u32::try_from(payload.len()).map_err(|_| Error::FrameTooLarge(u32::MAX))?;
        if len > MAX_FRAME {
            return Err(Error::FrameTooLarge(len));
        }
        // "native byte order" per the man page — so `to_ne_bytes`, deliberately
        // not `to_be_bytes`. This is a Unix socket on one machine; there is no
        // endianness negotiation and none is needed.
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&len.to_ne_bytes());
        frame.extend_from_slice(&payload);
        self.sock.write_all(&frame)?;
        self.sock.flush()?;
        Ok(())
    }

    fn read(&mut self) -> Result<Response, Error> {
        let mut len = [0u8; 4];
        self.sock.read_exact(&mut len)?;
        let len = u32::from_ne_bytes(len);
        if len > MAX_FRAME {
            return Err(Error::FrameTooLarge(len));
        }
        let mut payload = vec![0u8; len as usize];
        self.sock.read_exact(&mut payload)?;
        Ok(serde_json::from_slice(&payload)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_match_the_documented_shapes() {
        let j = |r: &Request| serde_json::to_string(r).unwrap();
        assert_eq!(
            j(&Request::CreateSession {
                username: "dirk".into()
            }),
            r#"{"type":"create_session","username":"dirk"}"#
        );
        // An absent response must be omitted, not sent as null: the man page
        // documents the field as optional.
        assert_eq!(
            j(&Request::PostAuthMessageResponse { response: None }),
            r#"{"type":"post_auth_message_response"}"#
        );
        assert_eq!(
            j(&Request::PostAuthMessageResponse {
                response: Some("hunter2".into())
            }),
            r#"{"type":"post_auth_message_response","response":"hunter2"}"#
        );
        assert_eq!(j(&Request::CancelSession), r#"{"type":"cancel_session"}"#);
        assert_eq!(
            j(&Request::StartSession {
                cmd: vec!["Hyprland".into()],
                env: vec![]
            }),
            r#"{"type":"start_session","cmd":["Hyprland"]}"#
        );
    }

    #[test]
    fn responses_parse() {
        let p = |s: &str| serde_json::from_str::<Response>(s).unwrap();
        assert!(matches!(p(r#"{"type":"success"}"#), Response::Success));
        assert!(matches!(
            p(r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password:"}"#),
            Response::AuthMessage {
                auth_message_type: AuthMessageType::Secret,
                ..
            }
        ));
        assert!(matches!(
            p(r#"{"type":"error","error_type":"auth_error","description":"nope"}"#),
            Response::Error {
                error_type: ErrorType::AuthError,
                ..
            }
        ));
    }

    #[test]
    fn only_prompts_want_answers() {
        assert!(AuthMessageType::Visible.wants_response());
        assert!(AuthMessageType::Secret.wants_response());
        assert!(!AuthMessageType::Info.wants_response());
        assert!(!AuthMessageType::Error.wants_response());
    }

    #[test]
    fn framing_round_trips_over_a_socket_pair() {
        use std::os::unix::net::UnixStream as S;
        let (a, b) = S::pair().unwrap();
        let mut client = Connection { sock: a };
        let mut server = Connection { sock: b };
        client
            .write(&Request::CreateSession {
                username: "dirk".into(),
            })
            .unwrap();
        // Read it back off the far end as a raw frame to prove the length prefix.
        let mut len = [0u8; 4];
        {
            use std::io::Read;
            server.sock.read_exact(&mut len).unwrap();
            let n = u32::from_ne_bytes(len) as usize;
            let mut buf = vec![0u8; n];
            server.sock.read_exact(&mut buf).unwrap();
            let r: Request = serde_json::from_slice(&buf).unwrap();
            assert!(matches!(r, Request::CreateSession { ref username } if username == "dirk"));
        }
    }
}
