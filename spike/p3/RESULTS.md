# P3 — the lock surface, measured. No authentication was written.

Run on 2026-11-08 on the target machine: CachyOS, kernel 7.3.3-1-cachyos,
Hyprland 0.57.1 (`v0.57.1`, commit `acc1aa0`), NVIDIA, Dell S3220DGF.
Reproduce with [`lock-measure.nu`](lock-measure.nu); nested compositor config
in [`nested.lua`](nested.lua).

> **Pairs 1 and 2 in §3 were taken against a compositor that was running on
> its defaults, not on the config this file cites.** The nested config was
> `nested.conf`, in hyprlang, and Hyprland 0.57.1 reads a `-c` file as Lua:
> every `#` comment was a syntax error, the config was abandoned whole, and
> the compositor came up in emergency mode with animations and blur ON. The
> error is drawn as an overlay on the nested window and does not reach the
> log, so nothing here showed it; it was found on 2026-11-09 by the operator
> reading his own screen. The file is Lua now and parses.
>
> It has since been re-measured. Pairs 3 and 4 in §3 were taken against the
> compositor reading `nested.lua`, and they land inside the spread the first
> two already had — so the broken config is not what moved these numbers, and
> nothing here needed withdrawing. What it did cost is the *direction* of the
> §3 claim: see pair 4.

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

Pairs 1 and 2 were taken on 2026-11-08, against the compositor that was
silently running on its defaults. Pairs 3 and 4 were taken on 2026-11-09
against the same compositor reading `nested.lua`, at 1259x679 and 1259x678
logical with the scale pinned to 1.

| surface | protocol | pair 1 | pair 2 | pair 3 | pair 4 |
|---|---|---|---|---|---|
| `auroval-greet preview` | `xdg_shell` | 22.7% | 33.2% | 19.6% | 14.6% |
| `auroval-lock rehearse` | `ext-session-lock-v1` | 19.4% | 32.9% | 17.7% | 17.3% |
| difference, lock − greet | | −3.3 | −0.3 | −1.9 | **+2.7** |

**The fourth pair reverses the sign, and that is the useful result.** After
three pairs with the lock surface cheaper, the first phrasing of this section
read *"the cheaper one both times"*, which was a pattern in three samples and
not a property. Pair 4 has the lock surface 2.7 points dearer. What survives
all four is the magnitude: **the two surfaces are within ±3.3 points of each
other, in both directions.**

That band is smaller than the spread of one binary against itself. The
greeter alone covers 14.6% to 33.2% on the same clip at the same size — so
the difference the pairing is trying to resolve is below the noise it is
measured in. The honest claim is therefore bounded rather than directional:
*nothing here shows `ext-session-lock-v1` costing more than the greeter's
surface, and the resolution of this instrument is about three points.* A
difference smaller than that would not be visible; a difference large enough
to threaten R7 would be.

The spread itself is not new and is still unexplained. Q5 recorded it:
*"the GL path's run-to-run spread (26–42%, against the readback path's 2
points) is unexplained."* Pairing is the defence against it, which is why the
figures are only ever read within a pair — and pair 4 is why a *direction*
read across pairs is not safe either.

### What this is not, and the measurement that is still missing

**These are not R7's numbers.** R7's ceiling — 50% of one core for the whole
present path — is stated at 1440p60 under `cage` on a free VT, and R16's note
in the greeter's PRD records why that mattered: an earlier 1440p figure *"was
measuring the nesting"* of a nested `cage` at 1255x674 inside a live Hyprland
session, and was withdrawn. These runs are nested, at 1259x679
logical with the scale pinned to 1 — pairs 1 and 2 were at 630x339 logical,
scale 2, the same pixels through a different reported geometry — and against
a 30 fps clip rather than 60. Every one of those differences moves the
number.

Both figures sit under 50%, and that is worth nothing as evidence about R7.
**The 1440p60-on-a-free-VT measurement of the lock surface has not been
taken**, because taking it means a real Hyprland on a VT with the maintainer
present and a second TTY open — see L14 in the PRD. It is listed unticked on
issue #67 rather than quietly inferred from the figures above.

`cage` cannot stand in for it: it is a kiosk compositor and advertises no
`ext_session_lock_manager_v1`, which is why the greeter's harnesses could use
it and this one cannot.

## 4. Two findings nobody was looking for

**~~Hyprland takes 5.00 s to send `locked`.~~ It was waiting for us.**
Measured twice at 5000 ms from process start, with the first locked frame
23–29 ms *after* it. The flatness said deliberate delay rather than work, and
the conclusion drawn here was that the delay was the compositor's business.

**That was wrong, and the sentence that disproves it is quoted in the
paragraph it was written in.** The compositor may not send `locked` *"until a
new 'locked' frame has been presented on all outputs"* — so it was waiting for
a frame from this client. And this client created no lock surface until the
`locked` event arrived, because `get_lock_surface`'s *"These won't be
displayed by the compositor unless the lock is successful and the locked event
is sent"* had been read as a prohibition instead of as the reassurance it is.
Each side waited for the other; Hyprland broke the deadlock with a timeout and
showed its own fallback in the meantime.

Found on 2026-11-10 by the maintainer, who locked his own screen with the new
keybind and saw *"a black rectangle with large blown-up text in it"* smearing
under the mouse before the lock screen appeared — the compositor's fallback,
unrepainted, for five seconds. Nobody had seen it before because every
measurement until then was a nested window on a desktop that was repainting
it anyway.

Fixed by creating the surfaces immediately after `lock()`:

| | before | after |
|---|---|---|
| `locked` | 5000 ms | **34 ms** with a video background, **6 ms** with a still |
| first locked frame | 5034 ms | **27 ms** / **5 ms** |

Receipt: `failure-round-2026-11-10-handshake.txt`, the whole round re-run
against the fixed handshake — six of six, and the recovery sequence with it,
so nothing in L5–L8 or L11 depended on the inverted order.

**And confirmed on the real output, which is the only place it could be.**
The maintainer installed the fixed build and locked his own screen with
`SUPER + SHIFT + L`: no black rectangle, no smear, the lock screen straight
away. That confirmation cannot come from this harness — every nested
measurement in this file was a window on a desktop that repainted it anyway,
which is exactly why the symptom survived five days of green rounds.

And the order flipped: the frame now precedes `locked`, which is what the
protocol describes. The log had been printing the two the wrong way round for
five days, which was visible evidence of an inverted handshake that nobody
read — because the 5 s had a name, **N4**, and a thing with a name looks
handled.

So a locker's first-frame budget *is* the number a user experiences after all,
and N4 is answered rather than open.

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
`AUROVAL_LOCK_SECONDS` so that a run is bounded with nobody at the keyboard.
Since L9 that deadline **exits without unlocking**, so what bounds is the run
and not the lock: the nested compositor is left locked and §6 is how it is
handed back. Those three are there because `ext-session-lock-v1` fails closed
and the person most likely to lock their own screen with this is whoever is
least expecting it.

`auroval-lock probe` is safe anywhere: it binds the manager global and never
sends `lock`.

## 6. The failure round — what happens when the things around it break

```nu
nu spike/p3/failure-round.nu wayland-2
```

P3c's unattended half, run 2026-11-09 against a nested Hyprland 0.57.1 at
1259x678 scale 1. Five cases, each asserting that the locker **says what
broke, falls back, reaches `locked` and presents a frame**:

| case | requirement | what it did |
|---|---|---|
| config with a syntax error | L6 | named the file and the line, used defaults, locked at 5229 ms |
| config absent | L6 | said `No such file or directory`, used defaults, locked at 5000 ms |
| config unreadable, mode 000 | L6 | said `Permission denied`, used defaults, locked at 5000 ms |
| no GStreamer plugins | L5 | `no glsinkbin`, fell to the still, decoded it in 58 ms, locked |
| clip truncated mid-playback | L5 | played, then `Stream contains no data.`, fell to the still **while holding the lock**, did not panic |

The clip case **truncates rather than deletes**, and the difference is the
whole test: unlinking an open file changes nothing a reader can see, so a
round that deleted it would have proved only that Linux works.

**What this round cannot show is that any of these screens unlocks**, because
unlocking needs a password typed by a person. It does not type one: L10
forbids retrying, `pam_faillock` counts to three, and a wrong password left
behind by a harness is a tally nobody asked for. The script prints the
attended step instead.

### The recovery path, which is the compositor's and not ours

The question the round was really run to answer: **what gets you back in when
the locker dies holding the screen?** Asked by killing it with `SIGKILL` on a
held nested lock.

- Hyprland does **not** leave a black screen. It draws its own page — *"it
  looks like you locked your screen but the lockscreen app died"* — and names
  the way out: `hyprctl --instance 0 eval 'hl.clear_crashed_lockscreen()'`
  from another tty.
- That call **works**. The lock is released and the desktop comes back.
- A second `auroval-lock` **cannot take over**. Started against a compositor
  still holding the dead client's lock, it is sent `finished` and locks
  nothing.

That last one settles a design question before anything was built for it: **a
supervisor that restarts the binary on a crash is not a way out.** It would
restart a process the compositor refuses. The way out is the compositor's, it
is one command, and it needs a second tty — which is the same shape as the
greeter's rollback in `deploy/TRIAL.md`, and the reason the locker can be
dogfooded at all.

### One bug, found by asserting the obvious

L8 asks that a withdrawn or refused lock be *"reported, and exits non-zero"*.
It reported, and exited **0**. The same run printed `cpu 130.3% of one core`
and `R7 ceiling — OVER BUDGET` for a run that had been declined the lock and
presented no frames at all: 0.03 s of startup over a wall clock that had
barely started. Both are fixed — `finished` now exits **3**, and a run with no
frames in it gets no budget verdict — and both are the shape of the summary
that denied its own unlock a day earlier: a line that was true once, stated as
a constant, with nothing reading it back.

## 7. L7, and the two things that made it look broken

**2026-11-10.** The 2026-11-09 round could ask four of L5–L8 and not this
one: a broken config, a missing file, a truncated clip and a second locker
can all be arranged from outside the process, and taking a GL context away
cannot. So the binary carries the way to do it —
`Gl::break_context_on_purpose`, asked for with
`AUROVAL_LOCK_BREAK_GL=<frames>` — and the round has a sixth case. Receipt:
`failure-round-2026-11-10.txt`, six of six.

With the context destroyed three frames into a working lock, the locker
reports the loss **once** rather than per turn, keeps turning the loop at 63
turns a second drawing nothing, still reaches its own deadline afterwards,
and does not panic. The two ordering checks are the point: presence proves
nothing when the claim is *after*.

### The silence that was there before the hook was

`draw_all` had `if gl.make_current(target).is_err() { continue; }`. That is
the right thing to **do** and it says nothing — after a context is lost every
turn takes that branch, so a screen frozen on its last frame and a screen
with nothing to redraw are the same log. It reports once now.

### Why a FAIL in the middle of this was not a failure

The second run of the round printed six crosses against L7. Nothing was
wrong with L7. Two causes, both the operator's and neither the locker's:

1. **The nested window was not on the visible workspace.** Hyprland does not
   render a window you are not looking at, and a nested compositor that is
   not being rendered never finishes the `ext-session-lock-v1` handshake. The
   locker sat at `waiting for the compositor's locked` for ever and every
   assertion about it came out false. Diagnosed by the maintainer, from his
   own screen, which is the second time that has been the thing that caught
   it.
2. **A hand-run had left the lock held.** `AUROVAL_LOCK_SECONDS` exits
   *without* unlocking (L9), so every later run was sent `finished` and
   exited 3. That is L8 working, and it reads like a fault.

The round now reports **VOID** rather than FAIL for a case that never reached
`locked`, and names which of the two it was. A requirement that was never
asked has not been failed, and a harness that cannot tell the difference will
eventually send somebody to fix code that is fine.

### What the hook costs, and what it cannot reach

Measured rather than assumed, and the first version of this section had it
wrong. After the loss the process spends **11–18% of one core** with a video
background and **0–1%** with a still — unchanged by skipping the draw path,
because the cost is GStreamer decoding on its own threads whether or not
anyone pulls the samples.

Stopping the pipeline would reclaim that and is deliberately not done.
`Video` wraps **this** context — `GLContext::new_wrapped` over our own handle,
not a context of its own — so taking the pipeline to `Null` frees GLMemory
against the context that was just destroyed. That is the hazard, not the way
out of it.

The same wrapping is the honest limit of the hook. A real driver reset takes
GStreamer's view of the context with it, and GStreamer would then make GL
calls on a dead context from its own threads with none of our code in
between. Destroying a context between two frames does not reproduce that, and
L7 says so rather than claiming the stronger thing.
