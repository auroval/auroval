//! auroval's login screen.
//!
//! P1 is being built in pieces that do not touch each other's files; see the
//! issues linked from #18. What is here now is everything that can be decided
//! without a screen — the sessions on offer, the memory of the last login,
//! the palette and geometry, and the greetd conversation — plus `probe`,
//! which prints all of it so that each piece can be checked on its own.
//!
//! The crate-wide `dead_code` allow is deliberate and temporary. P1 is being
//! landed as pieces that do not touch each other's files, so modules arrive
//! complete and tested before the shell that calls them exists. Without this,
//! clippy's `-D warnings` would make a deliberately staged crate look broken
//! and would push everyone towards one unreviewable commit instead. **It goes
//! when piece A lands**, and anything still unused then is genuinely unused.
#![allow(dead_code)]

mod auth;
mod config;
mod render;
mod sessions;
mod store;
mod theme;
mod ui;

use config::Config;
use store::Store;

const USAGE: &str = "usage: auroval-greet probe";

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("probe") => probe(),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// Say what the greeter can see, in the context it is run in. The P0 spike
/// had the same mode and it earned its keep: nearly everything surprising
/// about a greeter is about what it can and cannot reach.
fn probe() {
    let cfg = Config::load(std::path::Path::new("/etc/auroval/greet.lua"));
    for p in &cfg.problems {
        println!("problem      {p}");
    }

    let found = sessions::discover();
    println!("sessions     {}", found.len());
    for s in &found {
        println!("  {:<22} {:?}  {}", s.id, s.kind, s.command.join(" "));
    }

    let store = Store::at(&cfg.state_path);
    let last = store.load();
    println!("state file   {}", cfg.state_path.display());
    println!("  last user    {}", last.user.as_deref().unwrap_or("—"));
    println!("  last session {}", last.session.as_deref().unwrap_or("—"));
    println!(
        "  writable     {}",
        match store.save(&last) {
            Ok(()) => "yes".to_string(),
            Err(e) => format!("no — {e}"),
        }
    );

    let state = ui::UiState::new(found, &last);
    println!(
        "preselected  {}",
        state.session().map(|s| s.id.as_str()).unwrap_or("—")
    );

    let g = cfg.geometry;
    println!(
        "geometry     clock {}px  date {}px  user {}px  field {}x{} r{} p{}",
        g.clock_px, g.date_px, g.user_px, g.field_w, g.field_h, g.field_rounding, g.rounding_power
    );
    println!("font         {}", render::atlas::FONT_CANDIDATES[0]);
    println!(
        "fonts found  {}",
        render::atlas::FONT_CANDIDATES
            .iter()
            .filter(|p| std::path::Path::new(p).is_file())
            .count()
    );
}
