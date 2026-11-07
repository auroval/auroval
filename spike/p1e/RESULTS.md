# P1/E results — deployment, and R16's real number

Receipts for issue #35. Two of the three scripts here need a TTY login or
root, so this file records both what has been measured and, honestly, what
has not.

## Status

| box | state |
|---|---|
| harness runs the greeter under `cage` on a real VT | **written**, exercised end to end against the real `auroval-greet`; not yet run on a TTY, where `cage` is real |
| `n/total` progress, results written after every step | **done**, verified by running it |
| greetd config touching neither `config.toml` nor SDDM | **done** |
| `/var/lib/auroval` owned by the greeter | **written** as tmpfiles; needs one root command to take effect |
| C15 cache in the greeter's environment | **done** in `deploy/auroval-greet.sh` |
| R16 re-measured at 2560×1440 on a real VT | **not yet measured** — see below |
| C15 cache measured in the greeter's own context | **not yet** — the greeter has no GStreamer in it yet; the 1018/93 ms figures remain P0's |

## Why R16 is not measured here

Two reasons, and only one of them is about scheduling.

First, `auroval-greet` cannot draw anything yet. The Wayland shell is a
separate piece of P1 (#36) and until it lands the binary has no surface, no
frame, and nothing to time. The harness knows this: when no run reports a
`first presented : Nms after start` line it says so in capital letters and
refuses to turn process lifetimes into an R16 number.

Second, the measurement has to be taken on a TTY by the person sitting at the
machine. `cage` takes DRM master through logind from whichever VT you are
logged in on, so no amount of cleverness gets this from inside a desktop
session — and a nested `cage` measuring the desktop is precisely the mistake
that made the current 500 ms budget provisional in the first place. The
harness refuses to run when `WAYLAND_DISPLAY` or `DISPLAY` is set, for that
reason and no other.

So the budget in the PRD stays at 500 ms, still marked provisional, until
someone runs `auroval-vt` on a console after #36 lands.

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
- Every measurement run exits 2, which the harness now says out loud: until
  the Wayland shell lands, `auroval-greet` with no subcommand prints its usage
  and never starts a greeter. "No first-frame line" and "the binary refused to
  start" are different failures and the console now distinguishes them.

`cage` itself was stood in for, because taking DRM master needs a console.
Everything downstream of `cage` is the real thing.

Against stubs, because the real binary cannot yet produce the output:

- The success path: a stub printing P0's `first presented : Nms after start`
  lines gives a median, a worst case, a surface check against 2560×1440, and a
  budget verdict.

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

Then, from a TTY login (Ctrl+Alt+F3), once #36 has landed:

```
auroval-vt
```

Paste the resulting `~/auroval-p1e-first-frame.txt` under a new heading here,
and R16 has its number.
