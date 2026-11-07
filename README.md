# auroval

An opinionated look for Hyprland, as one set of parts that agree with each
other: the login screen, the lock screen, the bar, the window overview, and the
wallpaper-derived palette that ties them together.

**Status: design phase, with the P0 spike done.** The one assumption the whole
project rested on — that hardware video decode works inside a greeter, where
there is no user session and the process is not the logging-in user — has been
measured and holds: 1440p60 in hardware for 2.5% of one core, with 9.2× to
spare. See [PRD C6](docs/prd/greet.md) and the receipts in
[`spike/p0/RESULTS.md`](spike/p0/RESULTS.md). The code so far is that spike plus
the greetd protocol implementation it needed; there is no `greet` yet.

## Why a suite, and why start at the login screen

Hyprland has no shortage of desktop shells — Wayle, Noctalia, Way-Shell,
Ax-Shell, Caelestia, DankMaterialShell, Modus, OkPanel and others all build a
bar with notifications and a launcher, several of them well.

None of them is a greeter. The login screen is where a themed desktop gives
itself away: it is the one surface that belongs to no session, so it falls back
to whatever the display manager ships. auroval starts there and works outward,
which is the opposite order from every project listed above and the reason this
one exists.

The components share one renderer, one configuration language (hyprlang, which
Hyprland and hyprlock already use) and one palette. That sharing is the whole
argument for a mono-repo: a greeter and a bar that merely *look* alike will
drift apart, while two front-ends over one renderer cannot.

## Components

| | |
|---|---|
| `greet` | Login screen (greetd) and lock screen (`ext-session-lock-v1`), one binary, animated background on both. [PRD](docs/prd/greet.md) |
| `bar` | Status bar. Not yet specified. |
| `expose` | Window overview. Not yet specified. |
| `theme` | Wallpaper-derived palette, generated with matugen, consumed by every component. |

## Read this first

[`docs/vision.md`](docs/vision.md) and [`docs/tenets.md`](docs/tenets.md) come
first and win over anything below them. The tenets are tie-breakers with their
costs stated — read those if you want to know what this project refuses to do
and why. [`docs/ideas.md`](docs/ideas.md) holds what is deliberately parked.

[`docs/prd/greet.md`](docs/prd/greet.md) is the requirements document for the
first component. It states the goals and, more usefully, the non-goals, the
constraints with their sources, and the assumptions that are still
**unverified** — one of which decides whether the component is viable at all.

If you are evaluating whether this project should exist, read its "Ruled out"
section. It includes a working alternative that needs no code.

## Licence

**Not yet chosen** — see open question Q3 in the component PRD. Until a licence
is added, default copyright applies and this is not yet open source in the legal
sense.

The dependency that would have forced the choice no longer does. Q3 hung on Q2:
whether the `greetd_ipc` crate is GPL-3.0-only. It is — declared in the crate's
own manifest, which docs.rs does not display — so the protocol is implemented
in-house instead, and no GPL code sits in the authentication path. **The licence
is now a free choice rather than a constrained one**, and it is the one thing
here that cannot be deferred indefinitely: a public repository with no licence
is a repository nobody else may lawfully use.
