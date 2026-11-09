//! Which background this launch gets, when the theme offers more than one.
//!
//! R9's `background` list is a **fallback chain**: first entry that works
//! wins, and the ones after it exist for when it does not. That is the wrong
//! shape for "show me these two in turn" — a second entry added to a chain
//! whose first entry works is never seen at all.
//!
//! So a theme that wants rotation writes a list *of chains*, and this module
//! picks one of them per launch. The chain it returns is then handed to
//! `Background::start` unchanged, so R9's degradation is untouched: each
//! candidate still falls back through its own stills to the solid colour.
//!
//! The choice advances rather than being drawn at random, because with two
//! candidates a fair coin shows the same one twice in a row half the time,
//! which does not read as rotation. Advancing needs one integer remembered
//! across launches, and that is the only reason this module touches the disk.
//!
//! Every failure here degrades to showing *a* background, never to showing
//! none: the state file is advisory. Unreadable means "start somewhere", and
//! somewhere is derived from the clock rather than fixed, so a machine whose
//! state is wiped on every boot still alternates instead of pinning to the
//! first entry forever. Unwritable means "this launch does not advance", and
//! it says so on stderr, in R9's spirit of a logged rather than silent
//! degradation.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The remembered index lives beside the remembered login, for the reason
/// `store` gives: `/var/lib/auroval` is outside every home, which is what
/// C10 demands and what lets the greeter read its own state as itself.
pub fn state_beside(state_path: &Path) -> PathBuf {
    match state_path.parent() {
        Some(dir) => dir.join("last-background"),
        None => PathBuf::from("last-background"),
    }
}

/// Which candidate was shown last, without choosing or advancing.
///
/// `probe` needs this and must not use [`choose`]: a diagnostic that advances
/// the thing it reports on is one you cannot run twice, and running it would
/// change what the next real login shows.
pub fn last_shown(state: &Path) -> Option<usize> {
    read_index(state)
}

/// Which candidate the next launch will get, given a pool size. `None` when
/// there is nothing remembered, because then the choice is made from the
/// clock and predicting it here would be a guess dressed as a fact.
pub fn next_after(last: Option<usize>, n: usize) -> Option<usize> {
    match (last, n) {
        (_, 0) | (_, 1) => None,
        (Some(last), n) => Some((last + 1) % n),
        (None, _) => None,
    }
}

/// Whether the rotation memory could be written, asked without writing it.
///
/// A sibling file is created and removed rather than the real one rewritten:
/// on a machine that has never rotated there is no index to write back, and
/// inventing one would answer the question by changing the answer. The
/// directory is what the permission actually hangs on.
pub fn can_write(state: &Path) -> std::io::Result<()> {
    let probe = state.with_extension("probe");
    if let Some(dir) = probe.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::File::create(&probe)?;
    std::fs::remove_file(&probe)
}

/// Which candidate to show, and what to say about it.
pub struct Choice<'a> {
    /// The R9 chain for this launch. Empty when the pool is empty, which is
    /// the solid colour — the same thing an empty `background` has always
    /// meant.
    pub chain: &'a [PathBuf],
    /// Which candidate this is, for the log. `None` when there was nothing to
    /// choose between, so a single-background greeter logs nothing new.
    pub index: Option<usize>,
    /// Degradations, in R9's shape: the reason is stated, not swallowed.
    pub problems: Vec<String>,
}

/// Choose a chain from the pool and remember the choice.
///
/// `state` is written only when the pool can actually rotate. A greeter with
/// one background has nothing to remember, and writing a file anyway would
/// invent a failure mode for no gain.
pub fn choose<'a>(pool: &'a [Vec<PathBuf>], state: &Path) -> Choice<'a> {
    match pool.len() {
        0 => Choice {
            chain: &[],
            index: None,
            problems: Vec::new(),
        },
        1 => Choice {
            chain: &pool[0],
            index: None,
            problems: Vec::new(),
        },
        n => {
            let mut problems = Vec::new();
            let next = match read_index(state) {
                Some(last) => (last + 1) % n,
                // No memory yet. Not index 0: a machine that cannot keep this
                // file would then show the first candidate on every single
                // boot, which is the behaviour rotation exists to avoid.
                None => scatter(n),
            };
            if let Err(e) = write_index(state, next) {
                problems.push(format!(
                    "background rotation cannot remember its place in {}: {e} \
                     — the next launch will choose afresh",
                    state.display()
                ));
            }
            Choice {
                chain: &pool[next],
                index: Some(next),
                problems,
            }
        }
    }
}

fn read_index(path: &Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(v) = line.strip_prefix("index=") {
            return v.trim().parse::<usize>().ok();
        }
    }
    None
}

/// Written atomically, for `store`'s reason: a greeter killed mid-write must
/// not leave a file that makes the next launch worse than no memory at all.
fn write_index(path: &Path, index: usize) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        writeln!(
            f,
            "# auroval greeter. Which background was shown last; advisory."
        )?;
        writeln!(f, "index={index}")?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// A starting point when there is no memory. The clock, not a random number
/// generator: the greeter has no rng dependency, and this does not need to be
/// unpredictable — it needs only to not be constant.
fn scatter(n: usize) -> usize {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    nanos % n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(n: usize) -> Vec<Vec<PathBuf>> {
        (0..n)
            .map(|i| vec![PathBuf::from(format!("/var/lib/auroval/{i}.mp4"))])
            .collect()
    }

    fn tmp(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "auroval-rotation-{name}-{}",
            std::process::id() as usize
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn an_empty_pool_is_the_solid_colour() {
        let c = choose(&[], &tmp("empty"));
        assert!(c.chain.is_empty());
        assert_eq!(c.index, None);
    }

    #[test]
    fn one_candidate_rotates_to_itself_and_writes_nothing() {
        let state = tmp("single");
        let p = pool(1);
        let c = choose(&p, &state);
        assert_eq!(c.chain, p[0].as_slice());
        assert_eq!(c.index, None, "nothing was chosen, so nothing is logged");
        assert!(
            !state.exists(),
            "a greeter with one background has nothing to remember"
        );
    }

    #[test]
    fn two_candidates_alternate_across_launches() {
        let state = tmp("alternate");
        let p = pool(2);
        let first = choose(&p, &state).index.unwrap();
        let second = choose(&p, &state).index.unwrap();
        let third = choose(&p, &state).index.unwrap();
        assert_ne!(first, second, "a launch must not repeat the one before it");
        assert_eq!(first, third, "two candidates means a period of two");
        let _ = std::fs::remove_file(&state);
    }

    #[test]
    fn the_whole_pool_is_reached_before_any_candidate_repeats() {
        let state = tmp("cycle");
        let p = pool(4);
        let seen: Vec<usize> = (0..4).map(|_| choose(&p, &state).index.unwrap()).collect();
        let mut sorted = seen.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![0, 1, 2, 3], "saw {seen:?}");
        let _ = std::fs::remove_file(&state);
    }

    #[test]
    fn an_index_past_the_end_wraps_rather_than_panicking() {
        // The pool shrank since the file was written -- a theme edit is all it
        // takes. Indexing straight into the pool here would be a panic in the
        // one process that must never have one.
        let state = tmp("shrunk");
        std::fs::write(&state, "index=97\n").unwrap();
        let p = pool(2);
        let c = choose(&p, &state);
        assert_eq!(c.index, Some(0), "98 % 2");
        let _ = std::fs::remove_file(&state);
    }

    #[test]
    fn unreadable_state_still_chooses_and_does_not_complain() {
        // No memory is the ordinary first-boot case, not a degradation, so it
        // produces no problem line. It must still land inside the pool.
        let p = pool(3);
        let c = choose(&p, Path::new("/var/empty/auroval-does-not-exist/x"));
        assert!(c.index.unwrap() < 3);
    }

    #[test]
    fn unwritable_state_shows_a_background_anyway_and_says_why() {
        let p = pool(3);
        // A directory where the greeter cannot create files. `/proc` refuses
        // writes to every uid, root included, so this is not a test that
        // passes only for an unprivileged runner.
        let c = choose(&p, Path::new("/proc/auroval-rotation/last-background"));
        assert!(c.index.unwrap() < 3, "it still chose one");
        assert_eq!(c.problems.len(), 1, "and it said so: {:?}", c.problems);
        assert!(
            c.problems[0].contains("cannot remember"),
            "{:?}",
            c.problems
        );
    }

    #[test]
    fn a_garbled_state_file_is_no_memory_rather_than_a_failure() {
        let state = tmp("garbled");
        std::fs::write(&state, "# comment only\nnonsense\nindex=not-a-number\n").unwrap();
        let p = pool(3);
        assert!(choose(&p, &state).index.unwrap() < 3);
        let _ = std::fs::remove_file(&state);
    }

    #[test]
    fn the_probe_predicts_the_next_launch_only_when_there_is_memory() {
        // With nothing remembered the choice comes from the clock, so a
        // predicted index would be a guess printed as a fact.
        assert_eq!(next_after(None, 3), None);
        assert_eq!(next_after(Some(0), 3), Some(1));
        assert_eq!(next_after(Some(2), 3), Some(0));
        // A pool that cannot rotate has no next, remembered or not. This is
        // the same silence `choose` keeps for a single candidate.
        assert_eq!(next_after(Some(0), 1), None);
        assert_eq!(next_after(Some(0), 0), None);
        // The stale index `choose` wraps, the probe must wrap too, or the two
        // disagree about what is coming.
        assert_eq!(next_after(Some(97), 2), Some(0));
    }

    #[test]
    fn reporting_the_last_shown_candidate_does_not_advance_it() {
        let state = tmp("readonly");
        let p = pool(3);
        let first = choose(&p, &state).index.unwrap();
        for _ in 0..5 {
            assert_eq!(
                last_shown(&state),
                Some(first),
                "a diagnostic that moves what it reports cannot be run twice"
            );
        }
        let _ = std::fs::remove_file(&state);
    }

    #[test]
    fn writability_is_asked_without_inventing_a_memory() {
        let state = tmp("canwrite");
        assert!(can_write(&state).is_ok());
        assert!(
            !state.exists(),
            "asking the question must not answer it by changing it"
        );
        assert!(
            !state.with_extension("probe").exists(),
            "and must leave nothing of its own behind"
        );
        assert!(can_write(Path::new("/proc/auroval-rotation/last-background")).is_err());
    }

    #[test]
    fn the_state_file_sits_beside_the_login_memory() {
        assert_eq!(
            state_beside(Path::new("/var/lib/auroval/last-login")),
            PathBuf::from("/var/lib/auroval/last-background"),
            "C10's directory, which the greeter owns and can read as itself"
        );
    }
}
