# P1/E results — deployment, and R16's real number

Receipts for issue #35. Two of the three scripts here need a TTY login or
root, so this file records both what has been measured and, honestly, what
has not.

## Status

| box | state |
|---|---|
| harness runs the greeter under `cage` on a real VT | **done**, run on tty3 on 2026-11-07 |
| `n/total` progress, results written after every step | **done**, verified by running it |
| greetd config touching neither `config.toml` nor SDDM | **done** |
| `/var/lib/auroval` owned by the greeter | **written** as tmpfiles; needs one root command to take effect |
| C15 cache in the greeter's environment | **done** in `deploy/auroval-greet.sh` |
| R16 re-measured at 2560×1440 on a real VT | **done** — 8 ms warm, 9 ms cold, surface 2560×1440. See below |
| C15 cache measured in the greeter's own context | **not yet, and now known to be unmeasurable here** — nothing on the still-background path starts GStreamer, so cold and warm came out 9 ms and 8 ms. The 1018/93 ms figures remain P0's until the media path lands |

## R16's number, measured

Run on **2026-11-07 23:08**, host `games-werkkamer`, kernel 7.3.1-1-cachyos,
nvidia, from a TTY login on `/dev/tty3` with `cage` taking DRM master
directly. No desktop session underneath it, no nesting.

```
cold  first frame 9.0 ms  surface 2560x1440
warm  first frame 8.0 ms  surface 2560x1440
warm  first frame 8.0 ms  surface 2560x1440

median first frame 8.0 ms over 2 warm runs
worst 8.0 ms
surface 2560x1440
R16 budget 500 ms HOLDS — median 8.0 ms, worst 8.0 ms
```

**The 500 ms budget holds, with about sixty times the room.** The PRD's R16
is settled and no longer marked provisional.

The number it replaces was not a slower version of this measurement. P0's
243 ms came from a nested `cage` at 1255×674 inside a running Hyprland
session, and 243 against 8 is the cost of the nesting, not of the greeter.
That is the whole reason this script refuses to run with `WAYLAND_DISPLAY`
set.

### What this number does not cover

Two things, and both matter more than the number itself.

It is the **still background only**. No video wallpaper, no R23 audio —
and both of those pull in GStreamer, which nothing on this path touches.

Which is why **C15 does not appear in it**. Cold 9 ms against warm 8 ms is
not evidence that the registry rescan is cheap; it is evidence that the
rescan never happened, because no GStreamer was initialised in either run.
C15 remains a deployment requirement held on P0's measurement and on
faith. When the media path lands, this number has to be taken again, warm
and cold, and *the cold run is the one that tests C15*.

### Caveat on the sample: two runs, not ten

Eleven runs were started and three drew a frame. The other eight failed
after exactly 10 s with exit 1, and the kernel log says why: at 23:10:12 the
CRTC changed — the VT was switched back to the desktop, between run 3 and
run 4. `cage` can only hold DRM master while its session is the **active**
one, so from that moment every run waited ten seconds for a device it was
never going to get.

Three fixes went in because of this, and the first is the one that matters:

- The greeter takes `AUROVAL_EXIT_AFTER_FIRST_FRAME`, and the harness sets
  it. A run now ends the instant it has printed its line: **303 ms instead
  of 30 s**, measured. Eleven runs is a few seconds rather than five and a
  half minutes, which removes the reason anyone would switch away mid-run.
  The failure was a usability failure of the harness, not of the greeter.
- The harness names the cause the moment a run fails after one has worked,
  instead of leaving a bare `exit 1` in a table for someone to puzzle over
  later.
- The verdict says how many runs the median rests on. A median over two runs
  and a median over ten are not the same claim.

A median of 8.0 with a worst case of 8.0 over two runs is thin, but it is
thin in a way that does not matter: the question was whether a 500 ms budget
survives at native resolution, and the answer is not close enough for the
sample size to decide it.

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
