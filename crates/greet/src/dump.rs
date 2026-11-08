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
//! **With a video background the two coincide**, and the directory fills at
//! the clip's rate: a 20-second run against a 60 fps loop wrote 1169 PNGs.
//! That is the contract working rather than failing — every one of those is a
//! different picture — but it makes the dump useless for the question it was
//! built to answer, which is what the UI did. Use it with a still or with no
//! background configured.
//!
//! **One frame per change, not per frame.** A 2560x1440 PNG costs on the
//! order of a hundred milliseconds to encode and write, and the loop that
//! would be doing it is the same loop whose timing R6 measures, and a repeat
//! period is that gap plus whatever the callback's turn of the loop spent.
//! Dumping every frame would not record
//! the greeter, it would record a different, slower greeter. So a frame is
//! written only when the thing being drawn has actually changed, which is
//! also what makes the output readable: ten files that each mean something,
//! not nine hundred.
//!
//! **This is a debug flag on a login screen, and it stays one.** It writes
//! nothing unless the variable is set, it refuses any directory inside a
//! home (R15, C10), and the signature it dedupes on — `view::signature`,
//! which is also what the shell decides to draw on — holds a hidden field as
//! a count of dots and never as its contents. Though what the captured frame
//! shows is dots in a password field, the file name should not be the thing
//! that leaks a length either, so it carries a counter and a timestamp and
//! nothing else.

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

    /// Write a frame if anything visible has changed since the last one.
    ///
    /// `sig` is `view::signature` — what the picture depends on, as a string.
    /// This used to compute its own; it does not any more, because the shell
    /// now draws on exactly that signature changing and two lists of "what is
    /// visible" that must agree is the bug this project keeps finding. The
    /// dedupe stays here even so: the shell may draw for reasons that are not
    /// a change of picture (a first frame, a reconfigured surface), and a dump
    /// is a record of what was *said*.
    ///
    /// `pixels` is RGBA, top row first, as `Gl::read_pixels` returns it.
    pub fn maybe(
        &mut self,
        ui: &UiState,
        sig: &str,
        w: u32,
        h: u32,
        pixels: impl FnOnce() -> Vec<u8>,
    ) {
        if self.last.as_deref() == Some(sig) {
            return;
        }
        self.last = Some(sig.to_string());
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
