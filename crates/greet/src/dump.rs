//! `AUROVAL_FRAME_DUMP`: the greeter recording what it put on the screen.
//!
//! The P1/A+B console sitting ended with three questions nobody could
//! answer: was the extra PAM line visible for its four seconds or did it
//! flash at the end, did the one-time-password prompt replace the first
//! prompt's text or get appended to it, and what exactly did the error say.
//! All three are about pixels that existed for a moment on a VT, and the
//! only record was somebody's memory of a screen they were busy using.
//!
//! The greeter can answer those itself. It already reads its framebuffer
//! back for the render tests, so this is the same thing pointed at a
//! directory.
//!
//! **One frame per change, not per frame.** A 2560x1440 PNG costs on the
//! order of a hundred milliseconds to encode and write, and the loop that
//! would be doing it is the same loop whose timing R6 measures — the repeat
//! rate is already 40 ms plus a redraw. Dumping every frame would not record
//! the greeter, it would record a different, slower greeter. So a frame is
//! written only when the thing being drawn has actually changed, which is
//! also what makes the output readable: ten files that each mean something,
//! not nine hundred.
//!
//! **This is a debug flag on a login screen, and it stays one.** It writes
//! nothing unless the variable is set, it refuses any directory inside a
//! home (R15, C10), and the signature below deliberately holds the *length*
//! of the field and never its contents — though what is on the screen it
//! captures is dots in a password field, the file name should not be the
//! thing that leaks a length either, so it carries a counter and a
//! timestamp and nothing else.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::store::refuse_a_home;
use crate::ui::UiState;

pub struct Dump {
    dir: PathBuf,
    started: Instant,
    last: Option<String>,
    n: u32,
}

impl Dump {
    /// Returns `None` when the variable is unset, which is the normal case,
    /// and prints why when it is set but unusable. A debug flag that fails
    /// silently is worse than no debug flag: you sit on a VT doing the thing
    /// you meant to capture and find out afterwards that nothing was.
    pub fn from_env() -> Option<Dump> {
        let dir = PathBuf::from(std::env::var_os("AUROVAL_FRAME_DUMP")?);
        if let Err(e) = refuse_a_home(&dir, "AUROVAL_FRAME_DUMP") {
            eprintln!("auroval-greet: {e}");
            return None;
        }
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!(
                "auroval-greet: AUROVAL_FRAME_DUMP {}: {e}; not dumping",
                dir.display()
            );
            return None;
        }
        eprintln!(
            "auroval-greet: dumping a frame per change to {}",
            dir.display()
        );
        Some(Dump {
            dir,
            started: Instant::now(),
            last: None,
            n: 0,
        })
    }

    /// What is on the screen, as a string, for deciding whether it changed.
    ///
    /// The field is its character count and not its text. At the username
    /// stage that is a name rather than a secret, but a signature that is
    /// sometimes the content and sometimes a count is a signature somebody
    /// will misread later.
    ///
    /// The surface size is in it because a resize is a different picture of
    /// the same state, and on a VT it is also how a mode change would show
    /// up — which is worth a frame.
    fn signature(ui: &UiState, w: u32, h: u32) -> String {
        format!(
            "{w}x{h}|{:?}|{}|{}|{}|{}|{}|{}|{}",
            ui.stage,
            ui.prompt,
            ui.field().chars().count(),
            ui.secret,
            ui.caps_lock,
            ui.notice.as_ref().map(|n| n.text.as_str()).unwrap_or(""),
            ui.session().map(|s| s.id.as_str()).unwrap_or(""),
            // The clock, so a minute rolling over is a frame. It is one file
            // a minute and it dates every other frame in the directory.
            ui.clock,
        )
    }

    /// Write a frame if anything visible has changed since the last one.
    ///
    /// `pixels` is RGBA, top row first, as `Gl::read_pixels` returns it.
    pub fn maybe(&mut self, ui: &UiState, w: u32, h: u32, pixels: impl FnOnce() -> Vec<u8>) {
        let sig = Self::signature(ui, w, h);
        if self.last.as_deref() == Some(sig.as_str()) {
            return;
        }
        self.last = Some(sig);
        let ms = self.started.elapsed().as_millis();
        let path = self.dir.join(format!("{:04}-{:07}ms.png", self.n, ms));
        self.n += 1;
        match write_png(&path, w, h, &pixels()) {
            // The line is the index: it pairs a file with the stage it
            // caught, so the directory can be read without opening it.
            Ok(()) => eprintln!(
                "auroval-greet: frame {} stage {:?}{}",
                path.display(),
                ui.stage,
                match &ui.notice {
                    Some(n) => format!(" notice {:?}", n.text),
                    None => String::new(),
                }
            ),
            Err(e) => eprintln!("auroval-greet: frame {}: {e}", path.display()),
        }
    }
}

fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let buf = image::RgbaImage::from_raw(w, h, rgba.to_vec())
        .ok_or_else(|| format!("{} bytes is not {w}x{h} RGBA", rgba.len()))?;
    buf.save(path).map_err(|e| e.to_string())
}
