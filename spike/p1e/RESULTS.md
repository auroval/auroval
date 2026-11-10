# P1/E results — deployment, and R16's real number

Receipts for issue #35. Two of the three scripts here need a TTY login or
root, so this file records both what has been measured and, honestly, what
has not.

## Status

| box | state |
|---|---|
| harness runs the greeter under `cage` on a real VT | **done**, run on tty3 on 2026-11-07; eleven runs, all `exit 0`, about six seconds |
| `n/total` progress, results written after every step | **done**, verified by running it |
| greetd config touching neither `config.toml` nor SDDM | **done** |
| `/var/lib/auroval` owned by the greeter | **written** as tmpfiles; needs one root command to take effect |
| C15 cache in the greeter's environment | **done** in `deploy/auroval-greet.sh` |
| R16 re-measured at 2560×1440 on a real VT | **done** — median 8 ms over ten warm runs, worst 9 ms, cold 8 ms, surface 2560×1440. See below |
| C15 cache measured in the greeter's own context | **done** — the media path landed, so the cold run finally starts GStreamer. Cold 1874 ms on 2026-11-09 and 1319 ms on 2026-11-10, against 19 ms warm both times. See the two later sittings at the end of this file; the warm figure is settled and the cold one is not |

## R16's number, measured

Run on **2026-11-07 23:26**, host `games-werkkamer`, kernel 7.3.1-1-cachyos,
nvidia, from a TTY login on `/dev/tty3` with `cage` taking DRM master
directly. No desktop session underneath it, no nesting. Eleven runs, all
`exit 0`. Receipt: `cage-run-2026-11-07-2326.txt`.

```
cold  first frame 8.0 ms  wall 550.3 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 547.5 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 565.6 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 547.4 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 567.8 ms  surface 2560x1440  exit 0
warm  first frame 9.0 ms  wall 564.2 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 549.9 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 565.7 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 547.6 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 550.0 ms  surface 2560x1440  exit 0
warm  first frame 8.0 ms  wall 550.4 ms  surface 2560x1440  exit 0

median first frame 8.0 ms over 10 warm runs
worst 9.0 ms
surface 2560x1440
R16 budget 500 ms HOLDS — median 8.0 ms, worst 9.0 ms
```

**The 500 ms budget holds, with about sixty times the room.** The PRD's R16
is settled and no longer marked provisional. The spread is 8–9 ms across ten
runs, which is as tight as the millisecond resolution of the print allows —
nine of eleven runs landed on the same integer.

The number it replaces was not a slower version of this measurement. P0's
243 ms came from a nested `cage` at 1255×674 inside a running Hyprland
session, and 243 against 8 is the cost of the nesting, not of the greeter.
That is the whole reason this script refuses to run with `WAYLAND_DISPLAY`
set.

Wall time is ~550 ms per run against a first frame of 8 ms. The difference is
`cage` starting, taking DRM master, setting the mode and handing it back —
the greeter's share of a run is about 1.5% of it. Worth remembering before
anyone optimises the wrong end.

### What this number does not cover

Two things, and both matter more than the number itself.

It is the **still background only**. No video wallpaper, no R23 audio —
and both of those pull in GStreamer, which nothing on this path touches.

Which is why **C15 does not appear in it**. The cold run, with the registry
deleted, came out at 8.0 ms: the same as warm, to the millisecond. That is
not evidence that the rescan is cheap, it is evidence that the rescan never
happened, because no GStreamer was initialised in either run. C15 remains a
deployment requirement held on P0's measurement and on faith. When the media
path lands, this number has to be taken again, warm and cold, and *the cold
run is the one that tests C15*.

### The first attempt, and what it cost

Worth keeping, because the fix is reusable. An earlier run the same evening
(`cage-run-2026-11-07-2308.txt`) got three runs and lost eight. They failed
after exactly 10 s with exit 1, and the kernel log says why: at 23:10:12 the
CRTC changed — the VT was switched back to the desktop, between run 3 and
run 4. `cage` can only hold DRM master while its session is the **active**
one, so from that moment every run waited ten seconds for a device it was
never going to get.

That was the harness wasting its operator's attention, not a bug in the
greeter. `preview` draws forever, so each run had to be killed by the 30 s
timeout: eleven runs was five and a half minutes of black screen on a VT you
have to sit still on, and sitting still was not a reasonable thing to ask.

- The greeter takes `AUROVAL_EXIT_AFTER_FIRST_FRAME`, and the harness sets
  it. A run ends the instant it has printed its line. **550 ms instead of
  30 s**: the full eleven runs now take about six seconds, which is less
  time than it takes to wonder whether it has hung.
- The harness names the inactive-VT cause the moment a run fails after one
  has already worked, instead of leaving a bare `exit 1` to be puzzled over.
- The verdict says how many runs the median rests on.

The lesson generalises past this script. A measurement that requires an
operator to sit still for five minutes will be abandoned halfway, and the
abandonment will look like a bug in whatever was being measured. Making the
run short was worth more than any amount of better error reporting around
the long version.

## What the harness does

`cage-measure.nu`, in order, writing `~/auroval-p1e-first-frame.txt` after
every step:

1. the binary and `cage` exist
2. the C15 cache — its own for the run, and whether the deployed
   `/var/cache/auroval` exists at all
3. `probe` with no compositor
4. `probe` under `cage` — the first step that needs DRM master, so a greeter
   missing from the `video` group fails here with a clear cause instead of
   inside a timing loop
5. one **cold** run with the registry deleted, then ten **warm** runs

Then the median of the warm runs, the worst case, the surface size, and a
verdict: the budget holds, or it does not and here is the number to move it
to. It warns when the surface is not 2560×1440, because a result at any other
size is not the result R16 asked for.

Ten runs, counted `n/total`. Ten near-identical fullscreen runs with no sense
of how many are left reads as an infinite loop, and that has already nearly
caused someone to abort a good measurement.

## What was verified without a TTY

Against the real `auroval-greet`, built from `crates/greet` at `3eea410`:

- `deploy/install.nu` builds, stages, substitutes the VT into the greetd
  config, writes the root script and the `auroval-vt` wrapper, and runs no
  `sudo`. The root script it emits installs four files and runs
  `systemd-tmpfiles --create`, and nothing else.
- The harness runs the real binary through all of its steps. Its `probe`
  output is reproduced below.
- Every measurement run exited 2, which the harness said out loud. The cause
  outlived this section: the harness itself was invoking the binary with no
  mode, so it printed its usage and never started a greeter, and so did the
  installed `auroval-greet.sh`. Both were fixed (#41, #42) and the fix is what
  made the measurement above possible. "No first-frame line" and "the binary
  refused to start" are different failures and the console distinguishes them.

`cage` itself was stood in for, because taking DRM master needs a console.
Everything downstream of `cage` is the real thing.

Against stubs, because the real binary cannot yet produce the output:

- The success path: a stub printing P0's `first presented : Nms after start`
  lines gives a median, a worst case, a surface check against 2560×1440, and a
  budget verdict. Superseded by the real run above, and the stub's spelling of
  the line is not the greeter's — the harness parses both for that reason.

And without either:

- The harness refuses to run with `WAYLAND_DISPLAY` set, exit 2.
- Results are on disk after every step, confirmed by reading the file during a
  run that had not finished.
- `greeter-probe.nu` and `greetd-session.nu` refuse to run as non-root.
- The `greeter` user on this machine is in `video`, so `cage` will get DRM
  master. Checked with `id -nG greeter`: `greeter video`.

## `auroval-greet probe` today, as you

```
problem      configuration is not implemented yet; using defaults (wanted /etc/auroval/greet.lua)
sessions     2
  hyprland               Wayland  /usr/bin/start-hyprland
  hyprland-uwsm          Wayland  uwsm start -e -D Hyprland hyprland.desktop
state file   /var/lib/auroval/last-login
  last user    —
  last session —
  writable     no — Permission denied (os error 13)
preselected  hyprland
geometry     clock 110px  date 22px  user 16px  field 340x56 r12 p4
font         /usr/share/fonts/TTF/MesloLGSNerdFont-Regular.ttf
fonts found  5
```

That `writable no` is the box this piece exists to close, and it is one root
command away. Run as `greeter` it would say the same thing for the same
reason; run as `greeter` *after* the tmpfiles snippet is installed, it should
say `writable yes`. `greeter-probe.nu` is how that gets checked, and nobody
should believe the box is ticked on the strength of the line above, which was
produced by an account that does not own the directory either.

## What needs the maintainer

```
nu deploy/install.nu
sudo sh target/auroval-install-root.sh
sudo nu spike/p1e/greeter-probe.nu
```

The third one is the `/var/lib/auroval` box: it should go from
`writable no — Permission denied (os error 13)` to `writable yes`.

The R16 measurement itself is done; `auroval-vt` from a TTY login
(Ctrl+Alt+F3) is how it gets taken again, and it will have to be taken again
once the media path lands, because that is the run that finally tests C15.
**That has since happened, twice — see the last section of this file.**

## The two later sittings, and what changed about the status above

The closing paragraph above said the R16 measurement was done and would have
to be taken again once the media path landed. It landed, and it has been taken
twice more. Both are in this directory as receipts.

| sitting | warm median | worst | cold first frame |
|---|---|---|---|
| 2026-11-07, still background only | 8 ms | 9 ms | 8 ms — a rescan that never happened |
| 2026-11-09, media path landed | 19 ms | 19 ms | **1874 ms** |
| 2026-11-10, same binary and caches | 19 ms | 20 ms | **1319 ms** |

Two readings, in order of how much they are worth.

**The warm number is settled.** 19 ms twice, a millisecond apart at the worst
case, at 2560×1440 on a real VT against a 500 ms budget. The 11 ms the media
path costs over the still background is the whole price of GStreamer on this
path, and it is cheaper than C15's 93 ms figure suggested.

**The cold number is not settled, and the 2026-11-10 run is why.** It ran with
the shader cache warm and the registry cleared — the same configuration that
gave 835 ms on 2026-11-09, from which C15 derives a ~816 ms plugin scan and a
~1039 ms shader compilation by subtraction. 1319 ms does not fit that split:
it puts the scan alone at about 1300 ms. Nothing in the environment accounts
for the difference. `pacman.log` shows no gstreamer change since 2026-10-13
and no nvidia change since 2026-11-08, and no `nvidia/GLCache` entry was
written during the run, so the shader cache was hit rather than rebuilt.

What that costs us is the two numbers, not the argument. Both caches are
expensive, the registry half is warmed by `deploy/install.nu` and the shader
half cannot be warmed from an install — C15 measures why, and none of that
rests on the subtraction. But anyone quoting ~816 ms or ~1039 ms as a measured
figure is quoting a single sample that another single sample contradicts.
A cold number worth standing on needs the same ten runs the warm one got, and
each of those costs a cache rebuild.
