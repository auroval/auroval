//! A PAM client, implemented from `pam_conv(3)`, `pam_start(3)` and
//! `/usr/include/security/_pam_types.h`.
//!
//! This exists for the reason `crates/greetd-proto` exists: so that auroval
//! owns the code on a path it cannot afford to be wrong about. The candidates
//! were `pam-sys`, untouched since 2023, and `pam-client2`, actively
//! maintained but MPL-2.0 and 4231 lines of which a locker needs perhaps a
//! third — `env_list`, `session` and a terminal password reader are all in
//! there and a Wayland locker must never reach any of them. What is actually
//! needed is four functions and three structs against an ABI that has not
//! moved in twenty years, so it is written out here where it can be read in
//! one sitting.
//!
//! **Not bound, deliberately, and each absence is a decision:**
//!
//! * `pam_acct_mgmt` — `/etc/pam.d/auroval-lock` has no `account` stanza, by
//!   the same choice hyprlock made, so a service with no account lines falls
//!   through to `/etc/pam.d/other`, which denies. Calling it could not
//!   succeed. It is also the safer side here: an expired-account check that
//!   refuses is indistinguishable from a lockout.
//! * `pam_setcred` — a locker re-authenticates a user who is already logged
//!   in. There are no credentials to establish; the session has them.
//! * `pam_open_session` / `pam_close_session` — same reason. The session
//!   exists and is not ours to open.
//! * `pam_chauthtok` — a lock screen is not where anybody changes a password.
//!
//! # L4 cannot be met as written, and that is the protocol's doing
//!
//! L4 asks for the secret to live in one buffer, zeroed after the response.
//! The conversation contract makes that impossible: `pam_conv(3)` says of the
//! array we hand back that *"it is the caller's responsibility to release
//! both, this array and the responses themselves, using free(3)"*, and the
//! caller there is PAM. So the secret must be copied into memory PAM owns and
//! frees, and `free` does not clear. Linux-PAM overwrites the authtok on some
//! paths; that is an implementation detail and this module does not claim it.
//!
//! What is true, and is what L4 now says: the one copy this *process* owns is
//! zeroed as soon as it has been handed over, and it is never logged, dumped
//! or stored. The copy PAM owns is PAM's.

use std::ffi::{c_char, c_int, c_void, CStr, CString};

/// The one PAM stack this locker will ever use.
///
/// A constant and not a variable, since 2026-11-09. It used to be
/// `AUROVAL_LOCK_PAM_SERVICE`, so a stack could be tried without editing
/// `/etc/pam.d`, and `main.rs` asserted that the override *"cannot make
/// authentication weaker than that file is"*. That was false, and an
/// adversarial review traced it end to end: the only check was that
/// `/etc/pam.d/<name>` **existed**, never what was in it. This machine ships
/// `sddm-greeter`, `sddm-autologin` and `system-services`, each carrying an
/// `auth ... pam_permit.so` line that accepts anything at all, and a
/// leftover spike stack that accepts one fixed word. One `env =` line in a
/// user-writable `hyprland.conf` or `hypridle.conf`, and the next lock opened
/// on a single keystroke with no delay and no faillock tally.
///
/// The convenience was real and it cost the entire authentication. A
/// developer who wants another stack edits this line and rebuilds — which is
/// something an attacker with a text editor cannot do to a running system.
pub const SERVICE: &str = "auroval-lock";

// ── the ABI, from /usr/include/security/_pam_types.h ──────────────────────
//
// Transcribed rather than bound, so the line numbers are the check: struct
// pam_message is at 254, pam_response at 279, pam_conv at 286, and the
// message styles at 229-232 of that header.

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type ConvFn = unsafe extern "C" fn(
    num_msg: c_int,
    msg: *const *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
) -> c_int;

#[repr(C)]
struct PamConv {
    conv: Option<ConvFn>,
    appdata_ptr: *mut c_void,
}

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_AUTH_ERR: c_int = 7;
const PAM_CONV_ERR: c_int = 19;

/// 229-232 of the header. These four are the whole of `msg_style`.
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_ERROR_MSG: c_int = 3;
const PAM_TEXT_INFO: c_int = 4;

/// 246 of the header. PAM will not send more than this, so a larger count is
/// a sign something is wrong rather than a conversation to answer.
const PAM_MAX_NUM_MSG: c_int = 32;

/// `PAM_DISALLOW_NULL_AUTHTOK`, 103 of the header.
///
/// Passed on every call, and it is a decision rather than a default.
/// `/etc/pam.d/system-auth` carries `pam_unix.so try_first_pass nullok`, so
/// the stack as configured would accept an empty password for an account that
/// has none. On a lock screen that is a hole you could walk through, and this
/// flag closes it for this service without touching the shared stack.
const PAM_DISALLOW_NULL_AUTHTOK: c_int = 0x0001;

/// Refuse to be core-dumped, before there is anything worth dumping.
///
/// `pam::Secret` keeps the one copy this process owns in a buffer it zeroes,
/// which does nothing about a kernel that writes the whole address space to
/// disk when the process crashes. This machine has `systemd-coredump` with
/// `ulimit -c unlimited`, so a panic in the renderer while somebody was
/// halfway through typing would have put that half on disk, readable later.
/// Found by an adversarial review on 2026-11-09.
///
/// `PR_SET_DUMPABLE` to 0 also stops another process of the same user from
/// attaching with `ptrace`, which is the same secret by a different route.
///
/// A warning rather than a refusal if it fails: a locker that will not start
/// is worse than one whose crash dumps are not suppressed, and the caller is
/// told so it is in the journal rather than in nobody's head.
pub fn refuse_core_dumps() -> Result<(), String> {
    // SAFETY: prctl with PR_SET_DUMPABLE takes an int and touches nothing of
    // ours. The remaining arguments are ignored for this option and passed as
    // zero, which is what the man page asks for.
    let rc = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[link(name = "pam")]
unsafe extern "C" {
    fn pam_start(
        service_name: *const c_char,
        user: *const c_char,
        pam_conversation: *const PamConv,
        pamh: *mut *mut c_void,
    ) -> c_int;
    fn pam_authenticate(pamh: *mut c_void, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut c_void, pam_status: c_int) -> c_int;
    fn pam_strerror(pamh: *mut c_void, errnum: c_int) -> *const c_char;
}

// ── what to answer, decided apart from the FFI so it can be tested ────────

/// What the conversation hands back for one message.
///
/// Separated from the callback because this is where L1 lives — *"without
/// assuming the number, order or type of messages"* — and a requirement with
/// a test attached has to be reachable without a PAM stack.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    /// The secret. `PAM_PROMPT_ECHO_OFF` is the only thing that gets it.
    Secret,
    /// The session's own username, for a prompt that expects to see what is
    /// typed. L3 says this locker authenticates nobody else, so there is
    /// exactly one answer it can give and it is not the user's to choose.
    User,
    /// A message to show, not a question. `resp` stays null, which is what
    /// `pam_conv(3)` asks for.
    Nothing,
}

pub(crate) fn answer_for(msg_style: c_int) -> Option<Answer> {
    match msg_style {
        PAM_PROMPT_ECHO_OFF => Some(Answer::Secret),
        PAM_PROMPT_ECHO_ON => Some(Answer::User),
        PAM_ERROR_MSG | PAM_TEXT_INFO => Some(Answer::Nothing),
        // An unknown style is not a message to guess at. PAM_BINARY_PROMPT
        // exists on some systems and is not text at all; answering it with a
        // password would hand the secret to something that asked for bytes.
        _ => None,
    }
}

/// Everything the callback needs, reached through `appdata_ptr`.
struct Appdata<'a> {
    secret: &'a str,
    user: &'a str,
    /// Messages PAM sent, for the caller to show. Collected rather than
    /// printed: a locker has a screen, not a terminal.
    info: Vec<String>,
    /// Set when a message arrived that `answer_for` would not answer, so the
    /// failure can say which style rather than only that it failed.
    unanswerable: Option<c_int>,
}

/// The conversation function PAM calls back into.
///
/// `extern "C"`, so a panic here aborts the process — and an abort while the
/// session is locked leaves it locked (L11), which is fail-closed but is also
/// a person shut out of their machine. Hence `catch_unwind`: a bug in this
/// function becomes `PAM_CONV_ERR` and a refused unlock attempt, not a
/// lockout.
unsafe extern "C" fn converse(
    num_msg: c_int,
    msg: *const *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
) -> c_int {
    let guarded =
        std::panic::catch_unwind(|| unsafe { converse_inner(num_msg, msg, resp, appdata_ptr) });
    guarded.unwrap_or(PAM_CONV_ERR)
}

unsafe fn converse_inner(
    num_msg: c_int,
    msg: *const *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
) -> c_int {
    if msg.is_null() || resp.is_null() || appdata_ptr.is_null() {
        return PAM_CONV_ERR;
    }
    if num_msg <= 0 || num_msg > PAM_MAX_NUM_MSG {
        return PAM_CONV_ERR;
    }
    let data = unsafe { &mut *(appdata_ptr as *mut Appdata) };
    let n = num_msg as usize;

    // One array of `num_msg` structs, as `pam_conv(3)` is explicit about:
    // "*resp is a struct pam_response array and not an array of pointers".
    // calloc, so every `resp` starts null and a bail-out frees a consistent
    // array rather than reading uninitialised pointers.
    let out = unsafe { libc::calloc(n, size_of::<PamResponse>()) } as *mut PamResponse;
    if out.is_null() {
        return PAM_BUF_ERR;
    }

    for i in 0..n {
        let m = unsafe { *msg.add(i) };
        if m.is_null() {
            unsafe { free_array(out, n) };
            return PAM_CONV_ERR;
        }
        let m = unsafe { &*m };

        let answer = match answer_for(m.msg_style) {
            Some(a) => a,
            None => {
                data.unanswerable = Some(m.msg_style);
                unsafe { free_array(out, n) };
                return PAM_CONV_ERR;
            }
        };

        // The text, kept for the screen. A message with a null pointer is
        // not an error -- some modules send an empty info -- it is just
        // nothing to show.
        if matches!(answer, Answer::Nothing) && !m.msg.is_null() {
            if let Ok(s) = unsafe { CStr::from_ptr(m.msg) }.to_str() {
                if !s.trim().is_empty() {
                    data.info.push(s.trim().to_string());
                }
            }
        }

        let text: Option<&str> = match answer {
            Answer::Secret => Some(data.secret),
            Answer::User => Some(data.user),
            Answer::Nothing => None,
        };

        if let Some(text) = text {
            // Handed to PAM, which frees it. This is the copy L4's note is
            // about: it cannot be zeroed from here, because the moment it is
            // written into the array it is PAM's to release and this function
            // does not learn when that happens.
            let copy = unsafe { libc::calloc(text.len() + 1, 1) } as *mut c_char;
            if copy.is_null() {
                unsafe { free_array(out, n) };
                return PAM_BUF_ERR;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(text.as_ptr(), copy as *mut u8, text.len());
                (*out.add(i)).resp = copy;
            }
        }
        unsafe { (*out.add(i)).resp_retcode = 0 };
    }

    unsafe { *resp = out };
    PAM_SUCCESS
}

/// Release a part-built response array. Called only on the failure paths,
/// where PAM never sees the array and so never frees it: leaking it would
/// leak a copy of the secret for the life of the process.
unsafe fn free_array(arr: *mut PamResponse, n: usize) {
    for i in 0..n {
        let r = unsafe { (*arr.add(i)).resp };
        if !r.is_null() {
            unsafe { libc::free(r as *mut c_void) };
        }
    }
    unsafe { libc::free(arr as *mut c_void) };
}

// ── who is locked out ─────────────────────────────────────────────────────

/// The name of the user running this process, from `getpwuid(getuid())`.
///
/// L3: *"Never authenticate anyone but the session's own user, and offer no
/// way to reach another account."* This is the whole of how that is enforced,
/// and the shape is deliberate: there is no `getpwnam`, no name from the
/// environment, and no argument. `USER` and `LOGNAME` are inherited and a
/// parent can set them to anything; the uid cannot be talked out of.
pub fn own_username() -> Result<String, String> {
    let uid = unsafe { libc::getuid() };
    // SAFETY: getpwuid returns a pointer into a static buffer, valid until
    // the next call. The name is copied out before anything else can call it.
    let pw = unsafe { libc::getpwuid(uid) };
    if pw.is_null() {
        return Err(format!("no passwd entry for uid {uid}"));
    }
    let name = unsafe { (*pw).pw_name };
    if name.is_null() {
        return Err(format!("passwd entry for uid {uid} has no name"));
    }
    unsafe { CStr::from_ptr(name) }
        .to_str()
        .map(str::to_string)
        .map_err(|_| format!("the name for uid {uid} is not UTF-8"))
}

// ── the secret ────────────────────────────────────────────────────────────

/// The typed secret, in the one buffer this process owns.
///
/// It is a type rather than a `String` for two reasons, and the second is the
/// one that matters.
///
/// It zeroes itself on drop, with volatile writes so the optimiser cannot
/// decide that writing to memory nobody reads again is pointless — which it
/// otherwise may, and does.
///
/// And it keeps the secret **out of `UiState`** entirely. The view needs only
/// how many characters were typed, because `field_display` returns `Dots(n)`
/// when `secret` is set, so `ui.typed` is given a placeholder per keystroke
/// and never the character itself. That is stronger than L4 asks for and it
/// is almost free: it means no frame dump, no debug print of the view, and no
/// future change to the view can reach the secret, because it was never
/// there. N6 in the PRD is about refusing dumps while locked; this makes that
/// a second line of defence rather than the only one.
pub struct Secret(String);

/// Room for a password without the buffer ever moving.
///
/// `String::new()` starts at capacity 0 and grows by reallocation, and a
/// realloc copies the bytes to a new allocation and frees the old one
/// **without clearing it**. So `zero` reached the live buffer and left a
/// plaintext prefix of every earlier length behind in freed heap: `s`, `s3`,
/// `s3cr`, and so on. Found by an adversarial review.
///
/// Reserving once, up front, means a secret of this length or less is never
/// copied anywhere. It is not a guarantee for every possible input — a longer
/// one still grows — but `pam_unix` reads at most `PAM_MAX_RESP_SIZE` (512)
/// anyway, so this covers everything the stack below will look at.
const SECRET_CAPACITY: usize = 512;

impl Secret {
    pub fn new() -> Self {
        Self(String::with_capacity(SECRET_CAPACITY))
    }
    pub fn push(&mut self, c: char) {
        self.0.push(c);
    }
    pub fn pop(&mut self) {
        self.0.pop();
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Zero and empty it, without waiting for a drop.
    pub fn clear(&mut self) {
        zero(&mut self.0);
        self.0.clear();
    }

    /// The capacity a fresh one starts with, for the test below.
    #[cfg(test)]
    fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        zero(&mut self.0);
    }
}

fn zero(s: &mut String) {
    // SAFETY: zero is a valid UTF-8 byte, so the string stays well-formed
    // for the `clear` or the drop that follows.
    let bytes = unsafe { s.as_mut_vec() };
    for b in bytes.iter_mut() {
        unsafe { std::ptr::write_volatile(b, 0) };
    }
}

// ── the call ──────────────────────────────────────────────────────────────

/// What came back. Three outcomes and not two, because "denied" and "the
/// stack is broken" need different handling: the first is the user's to fix
/// by typing again, the second is not.
#[derive(Debug)]
pub enum Outcome {
    /// PAM said yes.
    Success { info: Vec<String> },
    /// PAM said no. The user may try again; nothing here retries for them
    /// (L10).
    Denied { reason: String, info: Vec<String> },
    /// Something below us failed. Not a wrong password.
    Error { reason: String },
}

/// Authenticate `user` against `service`, blocking until PAM answers.
///
/// **This blocks for seconds and must not be called on the render thread.**
/// `pam_unix(8)`'s `nodelay` is not set in `/etc/pam.d/system-auth`, and its
/// documented default is *"a delay-on-failure of the order of two seconds"*.
/// On the draw loop that is a lock screen frozen mid-animation at exactly the
/// moment somebody is wondering whether they mistyped. L7 predicted this:
/// *"if the PAM conversation shares a thread with the renderer, a render path
/// that cannot recover takes the unlock with it."*
///
/// `secret` is borrowed and never copied into this module's own storage. The
/// caller owns it and zeroes it; see L4's note at the top of this file for the
/// copy that PAM owns.
pub fn authenticate(service: &str, user: &str, secret: &str) -> Outcome {
    let (Ok(c_service), Ok(c_user)) = (CString::new(service), CString::new(user)) else {
        return Outcome::Error {
            reason: "service or user name contains a NUL byte".into(),
        };
    };

    let mut data = Appdata {
        secret,
        user,
        info: Vec::new(),
        unanswerable: None,
    };
    let conv = PamConv {
        conv: Some(converse),
        appdata_ptr: &raw mut data as *mut c_void,
    };

    let mut handle: *mut c_void = std::ptr::null_mut();
    let rc = unsafe {
        pam_start(
            c_service.as_ptr(),
            c_user.as_ptr(),
            &conv as *const PamConv,
            &mut handle,
        )
    };
    if rc != PAM_SUCCESS || handle.is_null() {
        return Outcome::Error {
            reason: format!("pam_start: {}", strerror(std::ptr::null_mut(), rc)),
        };
    }

    let rc = unsafe { pam_authenticate(handle, PAM_DISALLOW_NULL_AUTHTOK) };
    let reason = strerror(handle, rc);
    let info = std::mem::take(&mut data.info);
    let unanswerable = data.unanswerable;
    unsafe { pam_end(handle, rc) };

    if let Some(style) = unanswerable {
        return Outcome::Error {
            reason: format!(
                "PAM sent a message of style {style}, which this locker will not answer"
            ),
        };
    }
    match rc {
        PAM_SUCCESS => Outcome::Success { info },
        PAM_AUTH_ERR => Outcome::Denied { reason, info },
        _ => Outcome::Denied { reason, info },
    }
}

fn strerror(handle: *mut c_void, rc: c_int) -> String {
    let p = unsafe { pam_strerror(handle, rc) };
    if p.is_null() {
        return format!("PAM error {rc}");
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .unwrap_or("unprintable PAM error")
        .to_string()
}

#[cfg(test)]
mod tests {
    /// `PR_SET_DUMPABLE` really is off afterwards, asked of the kernel rather
    /// than assumed from a return code. The test process is dumpable by
    /// default, so this also shows the call did something.
    #[test]
    fn core_dumps_are_actually_refused() {
        // SAFETY: PR_GET_DUMPABLE reads one int and touches nothing of ours.
        let before = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) };
        assert_eq!(before, 1, "the test process was expected to be dumpable");
        super::refuse_core_dumps().expect("prctl refused");
        let after = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) };
        assert_eq!(after, 0, "still dumpable after the call");
    }

    /// The regression `SECRET_CAPACITY` exists for: a `String` from capacity
    /// 0 reallocates as it grows, and a realloc copies the bytes to a new
    /// allocation and frees the old one without clearing it. `zero` reached
    /// the live buffer and left a plaintext prefix of every earlier length
    /// behind in freed heap.
    ///
    /// The property asserted is the one that matters: a password typed one
    /// character at a time never moves.
    #[test]
    fn a_secret_does_not_move_while_it_is_typed() {
        let mut s = Secret::new();
        let before = s.capacity();
        assert!(before >= SECRET_CAPACITY);
        for c in "a-fairly-long-password-typed-one-key-at-a-time".chars() {
            s.push(c);
        }
        assert_eq!(
            s.capacity(),
            before,
            "the buffer reallocated, so an earlier prefix is in freed heap"
        );
    }

    use super::*;

    /// L1, the part of it that is reachable without a PAM stack: the
    /// conversation decides per message style and assumes nothing about how
    /// many arrive or in what order.
    #[test]
    fn every_documented_message_style_has_an_answer() {
        assert_eq!(answer_for(PAM_PROMPT_ECHO_OFF), Some(Answer::Secret));
        assert_eq!(answer_for(PAM_PROMPT_ECHO_ON), Some(Answer::User));
        assert_eq!(answer_for(PAM_ERROR_MSG), Some(Answer::Nothing));
        assert_eq!(answer_for(PAM_TEXT_INFO), Some(Answer::Nothing));
    }

    /// The one that matters. `PAM_BINARY_PROMPT` is 7 on Linux-PAM and asks
    /// for bytes, not text; a conversation that fell through to "it must want
    /// the password" would hand the secret to it. Anything unrecognised is
    /// refused rather than guessed at.
    #[test]
    fn an_unknown_style_is_refused_rather_than_answered() {
        for style in [0, 5, 6, 7, 99, -1, i32::MAX, i32::MIN] {
            assert_eq!(answer_for(style), None, "style {style} was answered");
        }
    }

    /// The secret has exactly one door. If a later edit adds a style that
    /// returns `Secret`, this fails and says so.
    #[test]
    fn only_echo_off_receives_the_secret() {
        let doors: Vec<c_int> = (-64..=64)
            .filter(|s| answer_for(*s) == Some(Answer::Secret))
            .collect();
        assert_eq!(doors, vec![PAM_PROMPT_ECHO_OFF]);
    }

    /// The constants are transcribed from a header rather than bound, so the
    /// transcription is what can be wrong. These are the values at lines
    /// 229-232, 246 and 103 of `_pam_types.h`.
    #[test]
    fn the_transcribed_constants_are_the_headers() {
        assert_eq!(
            (
                PAM_PROMPT_ECHO_OFF,
                PAM_PROMPT_ECHO_ON,
                PAM_ERROR_MSG,
                PAM_TEXT_INFO
            ),
            (1, 2, 3, 4)
        );
        assert_eq!(PAM_MAX_NUM_MSG, 32);
        assert_eq!(PAM_DISALLOW_NULL_AUTHTOK, 0x0001);
        assert_eq!(
            (PAM_SUCCESS, PAM_BUF_ERR, PAM_AUTH_ERR, PAM_CONV_ERR),
            (0, 5, 7, 19)
        );
    }

    /// Not "is it the right name" -- that depends on the machine -- but that
    /// it is the running uid's, which is L3's actual claim.
    #[test]
    fn the_username_is_the_running_uids() {
        let name = own_username().expect("this process has a passwd entry");
        assert!(!name.is_empty());
        let uid = unsafe { libc::getuid() };
        let pw = unsafe { libc::getpwuid(uid) };
        let expected = unsafe { CStr::from_ptr((*pw).pw_name) }.to_str().unwrap();
        assert_eq!(name, expected);
    }

    #[test]
    fn a_secret_is_zeroed_when_it_is_cleared() {
        let mut s = Secret::new();
        for c in "hunter2".chars() {
            s.push(c);
        }
        assert_eq!(s.as_str(), "hunter2");
        // The address of the buffer, so the bytes can be read after `clear`
        // has emptied the String but before the allocation is returned.
        let p = s.as_str().as_ptr();
        let n = s.as_str().len();
        s.clear();
        assert!(s.is_empty());
        let after = unsafe { std::slice::from_raw_parts(p, n) };
        assert_eq!(after, &[0u8; 7], "the buffer still held the secret");
    }

    #[test]
    fn backspace_shortens_the_secret_rather_than_the_view() {
        let mut s = Secret::new();
        for c in "abc".chars() {
            s.push(c);
        }
        s.pop();
        assert_eq!(s.as_str(), "ab");
    }

    /// The structs are `#[repr(C)]` transcriptions, so their layout is also
    /// transcription that can drift. **Field offsets, not sizes**: the first
    /// version of this test added the field sizes up and expected 12 for
    /// `{ int; const char* }`, which is 16 once the pointer's alignment pads
    /// the int. Sizes are a consequence of the layout; the offsets are the
    /// ABI, and they are what a wrong transcription would move.
    #[test]
    fn the_structs_have_the_layout_the_header_gives() {
        assert_eq!(std::mem::offset_of!(PamMessage, msg_style), 0);
        assert_eq!(
            std::mem::offset_of!(PamMessage, msg),
            size_of::<*const c_char>()
        );

        assert_eq!(std::mem::offset_of!(PamResponse, resp), 0);
        assert_eq!(
            std::mem::offset_of!(PamResponse, resp_retcode),
            size_of::<*mut c_char>()
        );

        // `conv` is `Option<fn>`, pointer-sized because a function pointer is
        // never null -- if that ever stopped being true the struct would gain
        // a discriminant and stop matching C.
        assert_eq!(std::mem::offset_of!(PamConv, conv), 0);
        assert_eq!(
            std::mem::offset_of!(PamConv, appdata_ptr),
            size_of::<*const c_void>()
        );
        assert_eq!(size_of::<PamConv>(), 2 * size_of::<*const c_void>());
    }
}
