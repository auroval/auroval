# P3 — the lock surface, measured. No authentication was written.

Run on 2026-11-08 on the target machine: CachyOS, kernel 7.3.3-1-cachyos,
Hyprland 0.57.1 (`v0.57.1`, commit `acc1aa0`), NVIDIA, Dell S3220DGF.
Reproduce with [`lock-measure.nu`](lock-measure.nu); nested compositor config
in [`nested.lua`](nested.lua).

> **The CPU figures below were taken against a compositor that was running on
> its defaults, not on the config this file cites.** The nested config was
> `nested.conf`, in hyprlang, and Hyprland 0.57.1 reads a `-c` file as Lua:
> every `#` comment was a syntax error, the config was abandoned whole, and
> the compositor came up in emergency mode with animations and blur ON. The
> error is drawn as an overlay on the nested window and does not reach the
> log, so nothing here showed it; it was found on 2026-11-09 by the operator
> reading his own screen. The file is Lua now and parses.
>
> How much this moves the numbers is **not known and is not assumed**. The
> figures are each process's own CPU out of `/proc`, so a compositor
> animating its own windows is not directly in them — but it shares a GPU and
> it sets the frame pacing both front ends answer to. The pairing in §4 is
> the part that survives regardless, because both front ends ran against the
> same broken compositor. **Any absolute figure below should be re-measured
> before it is quoted.**

**What this answers.** Three of issue #67's premises, and no more than three:
that `ext-session-lock-v1` is available here, that `crates/auroval-ui`'s
renderer draws on a lock surface, and that the video still lands in a GL
texture there. **What it deliberately does not answer** is anything about
unlocking, because `auroval-lock` cannot — there is no PAM code in the crate.
See [`docs/prd/lock.md`](../../docs/prd/lock.md) for the requirements, L9 for
the tripwire, and the PR for the boundary.

## 1. The protocol is there, at version 1

`auroval-lock probe` binds the manager global and locks nothing, so this was
taken on the live desktop:

| | |
|---|---|
| Protocol | `ext-session-lock-v1`, staging, in `/usr/share/wayland-protocols/staging/ext-session-lock/` |
| Manager global | **`ext_session_lock_manager_v1` version 1** |
| Hyprland | 0.57.1 — all three interfaces present in the binary and advertised on the registry |
| smithay-client-toolkit | 0.20 ships `session_lock`, binding `1..=1`, so no protocol code was written here at all |
| Outputs | 1 — `HDMI-A-1 2560x1440 scale 1`, Dell S3220DGF |
| Font | `/usr/share/fonts/TTF/MesloLGSNerdFont-Regular.ttf` — R11's face, loaded by path |

Version 1 is the only version the protocol defines, so "version 1" is not a
caveat; it is the whole protocol. Its own XML calls the staging phase out —
*"Backward compatible changes may be added together with the corresponding
interface version bump"* — so a future version 2 is possible and nothing here
would break, since sctk pins the range it asks for.

**That this is policy and not a guarantee is in the spec, not an inference.**
*"The compositor may choose to restrict this protocol to a special client
launched by the compositor itself or expose it to all privileged clients, this
is compositor policy."* Hyprland exposes it to any client of the session; a
compositor that did not would make this project's lock screen unbuildable on
it, which is why `probe` exists as its own mode and prints the answer.

## 2. The renderer draws on it, and the video stays on the GPU

| | |
|---|---|
| Surfaces created | 1, configured at **630x339 scale 2** (the output's logical size; 1259x678 physical) |
| Frame memory | **`memory:GLMemory`** — the decoded frame was already a texture in this renderer's context |
| Draw rate | 30/s against a 30 fps clip, i.e. one present per delivered frame |
| Clip | `/usr/local/share/auroval/nightshade-01.mp4`, R9 chain with `nightshade-01.png` behind it |

**Re-checked after the glib/gstreamer stack moved to 0.25** (#75, merged into
this branch): `greet videoprobe` through `auroval-ui`'s renderer still reports
`video/x-raw(memory:GLMemory)`, byte-for-byte the same caps string, with the
first frame at 240 ms warm and 28.8-29.0 frames/s over five runs. The first
run of a freshly linked binary read 256 ms and is dynamic-linker warm-up, not
a regression -- warm the binary before trusting a number here.

`memory:GLMemory` on a lock surface is the result worth having. Q5 settled
that `nvh264dec` negotiates it against a shared GL context and
`greet videoprobe` confirmed it against the greeter's own EGL context — but
both of those were an xdg surface or none at all. A lock surface is created by
a different protocol, and the question of whether the no-readback path
survives that was open until this run. It does, and nothing in
`crates/auroval-ui` needed changing for it: `render::Gl::target` takes a
`wl_surface` and has never known where it came from.

## 3. The present costs what the greeter's does — the protocol adds nothing

Both front ends, one after the other, on the same nested compositor, the same
clip, and — verified, not assumed — the same surface size. CPU is this
process's `utime + stime` out of `/proc/<pid>/stat`, sampled over a steady
window that starts 8 s in so that GStreamer's startup and the lock handshake
are outside it.

| surface | protocol | CPU, pair 1 | CPU, pair 2 |
|---|---|---|---|
| `auroval-greet preview` | `xdg_shell` | 22.7% | **33.2%** |
| `auroval-lock rehearse` | `ext-session-lock-v1` | 19.4% | **32.9%** |

Within each pair the two surfaces are **0.3 and 3.3 points apart, with the
lock surface the cheaper one both times**. So the claim this run supports is
the narrow one: *`ext-session-lock-v1` costs nothing the greeter's surface
does not.* That is the question P3 had, and it is answered.

The run-to-run spread between the pairs — 22.7 against 33.2 for the *same
binary on the same clip* — is large and is not new. Q5 recorded exactly this:
*"the GL path's run-to-run spread (26–42%, against the readback path's 2
points) is unexplained."* It is unexplained here too. Pairing is the defence
against it, which is why the figures are only ever read within a pair.

### What this is not, and the measurement that is still missing

**These are not R7's numbers.** R7's ceiling — 50% of one core for the whole
present path — is stated at 1440p60 under `cage` on a free VT, and R16's note
in the greeter's PRD records why that mattered: an earlier 1440p figure *"was
measuring the nesting"* of a nested `cage` at 1255x674 inside a live Hyprland
session, and was withdrawn. This run is nested, at 630x339 logical, against a
30 fps clip rather than 60. Every one of those differences moves the number.

Both figures sit under 50%, and that is worth nothing as evidence about R7.
**The 1440p60-on-a-free-VT measurement of the lock surface has not been
taken**, because taking it means a real Hyprland on a VT with the maintainer
present and a second TTY open — see L14 in the PRD. It is listed unticked on
issue #67 rather than quietly inferred from the figures above.

`cage` cannot stand in for it: it is a kiosk compositor and advertises no
`ext_session_lock_manager_v1`, which is why the greeter's harnesses could use
it and this one cannot.

## 4. Two findings nobody was looking for

**Hyprland takes 5.00 s to send `locked`.** Measured twice, at 5000 ms and
5000 ms from process start, with the first locked frame 23–29 ms after it.
The flatness of that figure says it is a deliberate delay in the compositor
and not work: the client is ready in tens of milliseconds and waits for
five seconds. The protocol permits it — the compositor may not send `locked`
*"until a new 'locked' frame has been presented on all outputs and no security
sensitive normal/unlocked content is possibly visible"*, and when that happens
is its business. But it means **a locker's own first-frame budget is not the
number a user experiences**, which is the shape R16 has for the greeter and
would be a misleading requirement here. Recorded as **N4** in the PRD rather
than written as a budget. Whether it is configurable in Hyprland, or specific
to the nested backend, was not investigated.

**Fail-closed was demonstrated by accident, which is the best way to learn
it.** An early version of `lock-measure.nu` killed the locker before its own
timer fired and then ran the greeter. The greeter read **0.3% of one core**
and looked like a triumph; it was drawing into a session that was still
locked, where the protocol says its surface is not displayed. The next lock
attempt then got `finished` immediately — *"This might be sent because there
is already another `ext_session_lock_v1` object held by a client"* — and
exited. Both behaviours are correct, and together they are C5 and R14 observed
rather than asserted: killing the process left the session locked, and nothing
could draw on it afterwards.

This is also the argument for the skeleton having no signal handler at all.
`SIGTERM`, `SIGINT` and `SIGKILL` all leave the session locked, because sctk's
own `Drop` sends `destroy` rather than `unlock_and_destroy` — its comment
reads *"choosing not to unlock here results in us failing secure."* R14's
property is therefore already true of `auroval-lock`, before any
authentication exists to go wrong.

## 5. How to reproduce, and the safety that is built in

From the repository root, with the release binaries built:

```nu
cargo build --workspace --release
```

```nu
Hyprland -c spike/p3/nested.lua
```

```nu
nu spike/p3/lock-measure.nu wayland-2 40
```

The display argument is the nested compositor's socket, which is the one in
`$env.XDG_RUNTIME_DIR` that was not there before. **The script refuses to run
against `$env.WAYLAND_DISPLAY`**, and `auroval-lock rehearse` refuses to start
at all without `AUROVAL_LOCK_REHEARSAL=1`, and the harness always passes
`AUROVAL_LOCK_SECONDS` so that a mistake releases itself. Those three are
there because `ext-session-lock-v1` fails closed and the person most likely to
lock their own screen with this is whoever is least expecting it.

`auroval-lock probe` is safe anywhere: it binds the manager global and never
sends `lock`.
