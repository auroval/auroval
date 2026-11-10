# lock — Product Requirements

Status: **draft for review** · Org: `auroval` · 2026-11-08

## Summary

`auroval-lock` is auroval's lock screen: an `ext-session-lock-v1` client that
plays the same nightshade loop as the login screen and looks very nearly like
it. It replaces `hyprlock`, which cannot show a video at all (C4) and is still
on Catppuccin Mocha rather than the scene-derived palette, making it the one
surface on this machine that is off-identity twice over. Issue #67 is the
request; the suite's [vision](../vision.md) and [tenets](../tenets.md) are
upstream of this document and win where they disagree.

Its parent document is [greet.md](greet.md), and this one is deliberately
short where that one is long: the two surfaces share a renderer, a config
language, a geometry and a background chain, and restating those here would
let them drift. **This document is about the three things a locker must get
right that a greeter does not**, and nothing else:

1. It holds the screen with a protocol whose guarantee is the security model
   (L2), not with a window that happens to be on top.
2. It authenticates *itself*, against PAM, for a user who is already logged
   in — where the greeter never authenticates anything and lets greetd own
   PAM entirely (L1, C20).
3. **Its failure policy is inverted.** `auroval-greet`'s worst case is a
   solid colour (R9). `auroval-lock`'s worst case is a screen nobody can
   unlock, so every failure has to degrade towards *still locked, still
   unlockable* (L5–L10).

## The decision this document should enable

1. **Is Q6 answered?** greet.md left open whether lock mode is the same
   binary or its own. The crate split done for this document is the evidence:
   the renderer moved into `crates/auroval-ui/` without a single behavioural
   change, and the surface and event-loop code did not move at all because
   none of it transfers. **Recommendation: separate binaries, shared
   library.** See §"Q6, answered".
2. **Does the inverted failure policy change the architecture, or only the
   tests?** L5–L10 are written as requirements rather than as a note, because
   the answer determines whether the PAM conversation can live on a worker
   thread the way greetd's does (L7).
3. **When is hyprlock removed?** Not with this document, and not with the PR
   that accompanies it. §"Phasing" says what has to be true first.

## Goals

- Unlocking looks and feels like the same machine as logging in and as the
  desktop — the same loop, the same clock, the same field, the same palette.
- A failure in decode, theming, config or the GPU costs appearance and never
  costs the ability to unlock.
- A failure in the locker itself leaves the session **locked**, not exposed.
  These two goals pull in opposite directions and L5–L10 are where the line
  is drawn between them.
- hyprlock can be removed from the machine without anything being missed.

## Non-goals

Refusals, not "later". Each would arrive looking like an obvious extension.

- **Not a screensaver, and no idle management.** What decides when to lock is
  `hypridle`, logind, or whatever the user already has. A locker that owns its
  own idle timer owns a policy that belongs to the session.
- **No password changing, no account unlocking, no faillock reset.** A locker
  that can clear `pam_faillock`'s counter is a locker that can be used to
  brute-force (C19).
- **No "switch user", no session list.** The greeter offers sessions because
  greetd starts them. A locker is locking one session that already exists;
  `ui::UiState` carries a session list only because it is shared code, and a
  locker leaves it empty (N3).
- **No media keys, notifications, or anything readable through the lock.**
  A lock screen that shows the content of a notification is a lock screen
  that leaks. If this is ever wanted it is a separate requirement with its own
  argument, not a convenience.
- **No "emergency unlock", no escape key, no debug bypass in a shipped
  build.** The skeleton has one and it is the single most dangerous thing in
  the crate; **L9** is the tripwire that removes it.
- **No X11, no remote, no OIDC implementation.** Same refusals as greet.md,
  for the same reasons. Cloud identity, if it ever works for login (Q7),
  arrives here through PAM and needs no code (C20).

## Constraints, with receipts

Numbered onward from greet.md's C-series so a C-number means one thing across
both documents. Everything here was checked on the target machine on
2026-11-08 unless marked otherwise.

**C17 — `ext-session-lock-v1` is available here, at version 1.** The protocol
XML is installed at
`/usr/share/wayland-protocols/staging/ext-session-lock/ext-session-lock-v1.xml`
and defines exactly one version of each of its three interfaces. Hyprland
0.57.1 (`v0.57.1`, commit `acc1aa0`) advertises all three on the registry —
confirmed by binding the manager global on the live session with
`auroval-lock probe`, which locks nothing. `smithay-client-toolkit` 0.20 —
already this project's toolkit — ships a `session_lock` module that binds
`1..=1`, so **no protocol code had to be written.**

Its availability is **policy, and the spec says so**: *"The compositor may
choose to restrict this protocol to a special client launched by the
compositor itself or expose it to all privileged clients, this is compositor
policy."* Hyprland exposes it. A compositor that did not would make this
unbuildable on it, which is why `probe` is a mode and prints the answer rather
than the code assuming it.

Receipt: [`spike/p3/RESULTS.md`](../../spike/p3/RESULTS.md) §1.

**C18 — the guarantee layer-shell does not give, which is the whole security
argument.** Three sentences from the protocol's own description and events,
and together they are the reason this is not an overlay surface:

| | |
|---|---|
| On a crash | *"If the client dies while the session is locked the session remains locked, possibly permanently depending on compositor policy."* |
| On going up | the `locked` event *"must not be sent until a new 'locked' frame has been presented on all outputs and no security sensitive normal/unlocked content is possibly visible."* |
| On coming down | `unlock_and_destroy` is *"a protocol error to make ... if the locked event has not been sent"*, and a client exiting *"must use the wl_display.sync request"* or there is *"no guarantee that the server has unlocked"* |

A `wlr-layer-shell` overlay has none of this. It is a window that is drawn on
top, so: a crash removes it and the desktop is simply there; there is no event
that says the desktop stopped being visible, so the gap between asking for the
surface and getting it is a gap in which the desktop is on screen; and input
exclusivity is a separate protocol that can be absent or refused. **The
compositor, not the client, decides when the session is locked** — that
sentence is what this protocol buys and it cannot be reproduced from the
client side. C5 in greet.md already recorded the crash half; C18 is the
complete version.

This was also demonstrated by accident, which is worth more than the quote:
an early measurement harness killed the locker, and the greeter run after it
read 0.3% of one core because it was drawing into a still-locked session where
its surface was not displayed. `RESULTS.md` §4.

**C19 — the PAM stack is `/etc/pam.d/hyprlock`'s, and it has a lockout in
it.** Read on the machine. The relevant chain:

```
/etc/pam.d/hyprlock   auth include login
/etc/pam.d/login      auth requisite pam_nologin.so; auth include system-local-login
                      -> system-login -> system-auth
/etc/pam.d/system-auth
                      auth required    pam_faillock.so preauth
                      auth [success=2 default=ignore] pam_systemd_home.so
                      auth [success=1 default=bad]    pam_unix.so try_first_pass nullok
                      auth [default=die] pam_faillock.so authfail
                      auth required    pam_faillock.so authsucc
```

Three things follow, and the third is the one that bites.

*The service name to use is `hyprlock`'s, or a new one that is a copy of it.*
`/etc/pam.d/greetd` is **not** usable: it opens with
`auth required pam_securetty.so`, which is there because greetd runs on a VT
and would refuse a locker that is not on one.

*`/etc/security/faillock.conf` is entirely commented out*, so the defaults
apply: **three failures, then the account is locked for ten minutes.** This is
the same mechanism `AGENTS.md` warns about for `sudo`, and on a lock screen it
is far worse than on a shell prompt: a cat on a keyboard, a key repeat, or a
retry loop in the locker produces three failures in under a second and then
the machine cannot be unlocked by its owner for ten minutes. **A locker that
retries automatically, or that resubmits on its own, is a lockout generator.**
L6 and L10 exist for this.

*A locker must not reset the counter*, which is why "no faillock reset" is a
non-goal rather than a convenience: `faillock --reset` is how you would make
the ten minutes go away, and a process that can do it from a locked screen has
removed the protection for everyone.

**C20 — a locker authenticates in its own process, and this is the single
largest difference from the greeter.** greet.md's whole security posture rests
on C1 and C11: the greeter *"is a client"*, greetd *"owns PAM, seats, TTYs and
session launching"*, and *"a design in which the greeter talks to
`accounts.google.com` is wrong"*. None of that transfers. There is no greetd
in a locked session and no socket to delegate to: `pam_start` /
`pam_authenticate` happen **in this process**, as the logged-in user, holding
whatever the person typed.

So the things greet.md could refuse on structural grounds have to be refused
here on discipline instead, and that is a real loss of safety which L1's fit
criterion is written to notice. What does carry over unchanged is C2's shape —
*"there are no limits on the number and type of messages that may be required
for authentication to succeed"* — because that is a property of PAM, not of
greetd. A locker hard-wired to one password field is wrong for the same reason
a greeter is.

`libpam` is already linked by `hyprlock` (`ldd`), so nothing new is being
introduced to the machine.

**C21 — hyprlock stays installed and working, and is not configured by this
repository.** `~/.config/hypr/hyprlock.conf` is generated by `playbook.yml` in
the separate `pc-setup` repository. Nothing in auroval writes it, and nothing
should: a lock screen with two owners is a lock screen that breaks on a
rebuild. The changeover is a `pc-setup` change made deliberately, after L14,
and hyprlock remains the fallback until then.

**C22 — `hyprlock` 0.9.7 cannot show a video, restated because it is this
document's premise rather than the greeter's.** `ldd /usr/bin/hyprlock` links
`libpng`, `libjpeg`, `libwebp` and `libjxl` and **no `libavcodec` or
`libavformat`**. greet.md's C4 reached the same conclusion from `strings` and
from hyprgraphics' format list; the link map is the stronger form of it. Its
current config sidesteps the question with `path = screenshot`, which inherits
the wallpaper rotation for free and shows your windows instead of the scene.

**C23 — `cage` cannot host a rehearsal, so the greeter's measurement harnesses
do not transfer.** `cage` is a kiosk compositor and advertises no
`ext_session_lock_manager_v1`. Every figure in greet.md that matters — R7's
33.7%, R16's 8 ms — was taken under `cage` on a free VT precisely because that
is the greeter's own context. A locker's context is a full session compositor,
so a rehearsal needs either a nested Hyprland (which measures the nesting; see
L14) or a real Hyprland on a free VT with someone present.

**C24 — `/etc/pam.d/auroval-lock` has no `account` stanza, so the
implementation must not call `pam_acct_mgmt()`.** The service
[`pc-setup` installs](https://github.com/dlouwers/pc-setup/pull/3) has one
effective line. It was `auth include login`, the same shape as `hyprlock`'s,
until 2026-11-10; it is now `auth include system-auth`, and L1 says why
`login` is the wrong stack for a locker. A PAM service with no `account` lines does not default to
permitting: it falls through to `/etc/pam.d/other`, which denies. So a locker
that conscientiously checked the account after a correct password would refuse
every unlock — and would do it **only on the deployed machine**, never in a
test that stubs PAM or on a developer box with a fuller stack. That is the
exact shape of failure L9's tripwire exists for, which is why it is written
down as a constraint before the code that could trip over it exists.

## Requirements

Priority is MoSCoW, as in greet.md. L-numbers, so an L is a locker
requirement and an R is a greeter one; where a requirement is inherited the
R-number it comes from is named.

### Authentication

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| L1 | MUST | Authenticate the session's own user against PAM, in this process, driving the conversation without assuming the number, order or type of messages (C20). | A PAM stack carrying an extra `pam_exec` prompt unlocks with no code change, and a stack asking only for a password also unlocks. A fixed one-field implementation fails this. **Met 2026-11-09.** `crates/lock/src/pam.rs` drives the conversation over every `num_msg` message and dispatches on `msg_style`; `answer_for` is a free function so that is testable without a stack, and an unrecognised style is refused rather than answered with the password — `PAM_BINARY_PROMPT` asks for bytes. `PAM_DISALLOW_NULL_AUTHTOK` is passed on every call, because the shared stack carries `pam_unix.so ... nullok` and an empty password on a lock screen is a hole you walk through. **Which stack is no longer a question, 2026-11-09.** `AUROVAL_LOCK_PAM_SERVICE` used to choose it, guarded only by the file existing, and `main.rs` claimed the override *"cannot make authentication weaker than that file is"*. An adversarial review traced that to the opposite: this machine ships `sddm-greeter`, `sddm-autologin` and `system-services`, each with an `auth ... pam_permit.so` line, plus a leftover spike stack with a fixed password. One `env =` line in a user-writable `hyprland.conf` and the next lock opened on one keystroke. The service is now the constant `pam::SERVICE`. **And which system stack, decided 2026-11-10.** It was `auth include login`, copied from hyprlock. `login` pulls in `pam_nologin` (twice, at `login:3` and `system-login:4`) and `pam_shells` (`system-login:3`), neither of which belongs to re-authenticating a user who is already logged in. `man 8 shutdown`: five minutes before a scheduled shutdown `/run/nologin` is created, and while it exists `pam_nologin` refuses the **correct** password before `pam_unix` is reached — on a login screen that is the point, on a lock screen it is the owner shut out of a running session five minutes before a shutdown he may want to cancel. It is now `auth include system-auth`, which keeps `pam_unix`, `pam_env` and all three `pam_faillock` calls including `authsucc`. Both stacks end at `pam_unix` with the same password, so this narrows the grounds for refusal and does **not** weaken authentication. The `account` stanza was reconsidered in the same pass and deliberately still absent; `pc-setup`'s playbook carries the argument and the strict order a reversal would need. |
| L2 | MUST | Hold the screen with `ext-session-lock-v1`, creating a lock surface for every output present and every output that appears while locked. | `probe` reports the manager global; a rehearsal reports one surface per output; an output hotplugged while locked gets a surface rather than showing the desktop. **Protocol and surfaces met and measured; the hotplug half is untested for want of a second output.** |
| L3 | MUST | Never authenticate anyone but the session's own user, and offer no way to reach another account, a shell, or a session switcher. | Reading the crate finds no `getpwnam` of any name but the running uid's, and no process spawn at all. |
| L4 | MUST | Hold the typed secret for no longer than the conversation needs it, and never write it anywhere. | **Amended 2026-11-09, because the original was not achievable and said so with confidence.** It asked for the secret to live in *one* buffer, zeroed after the response. `pam_conv(3)` makes that impossible: of the array the conversation hands back it says *"it is the caller's responsibility to release both, this array and the responses themselves, using free(3)"*, and the caller there is PAM. So a `malloc`'d copy must be handed over, PAM frees it, and `free` does not clear. Linux-PAM overwrites the authtok on some paths; that is an implementation detail and this project does not claim it. **What is required instead:** the one copy *this process* owns lives in `pam::Secret`, which zeroes itself with volatile writes on `clear` and on `Drop`, and it is **never copied into `UiState`** — the view gets a placeholder per keystroke, because `field_display` needs only the count. That is stronger than the original on the half that is in our hands: no frame dump, no debug print of the view and no later change to the view can reach a secret that was never there. `AUROVAL_FRAME_DUMP` must refuse to run while locked, or must render the field masked — whichever, it is asserted by a test, because it writes PNGs of the screen. **That test does not exist, 2026-11-09.** An adversarial review went looking for it: the locker never constructs a `Dump` at all, so nothing writes a PNG and the requirement is met in practice — but the sentence above claims a test is what holds it, and what holds it is an absence. The same review found what the placeholder really rests on: the locker pushes `'\u{2022}'` into `UiState` itself, and its stage never leaves `AskingUser`, so the field is not in hidden mode at all. The secret is genuinely never in the view, and the reason is one character literal rather than a type. Both are now stated rather than implied, and the field at least clears between attempts — it did not, so the screen showed the running length of every password tried that sitting. **And the character literal became an API, 2026-11-10.** `push('\u{2022}')` in the locker was correct and invisible: nothing stopped the next edit from writing `push(c)` there, and nothing would have failed. `UiState::push_placeholder` advances the field with no way to say what was typed, so a caller holding a secret cannot leak it into the view even by accident, and the masking in `field_display` stays as a second, independent reason — which for this requirement is the right number, not one. The same pass found that the masking had **never been on** for this screen: see N8. |

### Failing towards "still locked, still unlockable"

This is the inverted half, and the requirements are written so each failure
mode is a separate testable thing rather than one paragraph about robustness.
**The shape of every one of them is the same: the appearance may fail, the
lock may not lift, and the unlock path must survive.**

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| L5 | MUST | A video that fails to decode, at any point including mid-playback, falls back per R9's chain and does not affect the unlock path. | With GStreamer's plugins made unavailable, the lock screen renders the still, then the solid colour, and still unlocks. With the clip deleted while playing, the same. The fallback is logged, not silent. **Met for the render half, 2026-11-09**, by `spike/p3/failure-round.nu`: with the plugin path emptied it said `no glsinkbin`, fell to the still and locked; with the clip **truncated** under a playing pipeline it said `Stream contains no data.`, fell to the still *while holding the lock*, and did not panic. Truncated rather than deleted on purpose — unlinking an open file changes nothing a reader can see. **The unlock half is not shown by that round and cannot be**: it needs a password typed by a person, and a harness that types a wrong one leaves a `pam_faillock` tally nobody asked for. |
| L6 | MUST | A config that fails to parse, is missing, or is unreadable must not prevent unlocking. The error is reported with file and line and the locker runs on defaults. | A deliberately broken config, an absent config, and a config at mode `000` each yield a screen that unlocks. Inherited from R13 and **restated rather than referenced**, because R13's fit criterion is "still yields a usable login" and the consequence of failing it here is different in kind. **Met for the render half, 2026-11-09**, by `spike/p3/failure-round.nu`: all three configs — a syntax error, an absent file and one at mode `000` — were named in the report with their reason, the locker ran on defaults, reached `locked` and presented a frame. The unlock half carries L5's caveat, for L5's reason. |
| L7 | MUST | Losing the GL context — a GPU reset, a driver reload, a `GL_CONTEXT_LOST` — must not prevent unlocking. | With the context deliberately destroyed, the screen either recovers or degrades to a solid colour, and in both cases the field still accepts input and the session still unlocks. **Asked and met 2026-11-10**, by the hook the 2026-11-09 round said was missing. There is no honest way to destroy a GL context from outside a process, so `Gl::break_context_on_purpose` does it from inside and `AUROVAL_LOCK_BREAK_GL=<frames>` asks for it; `spike/p3/failure-round.nu` is the sixth case and `spike/p3/failure-round-2026-11-10.txt` is the receipt. Nine checks, including two **orderings** — presence proves nothing here, since two greps that both say yes can describe a log where the second line came first. With the context destroyed three frames into a working lock: the loss is reported once rather than per turn, the loop keeps turning at 63 turns a second with `draws 0`, the deadline still fires afterwards, and nothing panics. **The threading design this requirement was meant to decide was already right:** PAM runs on a worker thread polled with `try_recv`, so a render path that can no longer draw does not hold the unlock. **And typed, the same day.** The unattended round cannot type a password — L10 forbids retrying and `pam_faillock` counts to three — so the maintainer ran the attended half: a rehearsal in the nested compositor with `AUROVAL_LOCK_BREAK_GL=3`, and the password typed **blind**, into a screen frozen on its last frame with no character appearing and no clock moving. **It unlocked.** That is the clause this row could not otherwise claim — *the field still accepts input and the session still unlocks* — established with the context genuinely gone rather than with an argument about threads. **One thing in the criterion stays weaker than the row might read.** The screen does **not** degrade to a solid colour — it keeps its last frame, because clearing to a colour needs the context that was lost — so the criterion's "or degrades to a solid colour" is unreachable by construction and this is the "recovers or" branch failing gracefully rather than either of the two the requirement named. And the unlock itself is still untyped: the round never types a password (L10, `pam_faillock`), so what is established is that the loop which carries the keyboard and the PAM answer survives, not that a person got in. **What the hook cannot reach**, and it is the honest limit: `Video` wraps *this* context rather than holding one of its own, so a real driver reset would take GStreamer's view of it too and GStreamer would make GL calls on a dead context from its own threads with none of our code in between. Destroying the context between two frames does not reproduce that. **Measured cost of the failure, since it is not free:** after the loss this process spends 11–18% of one core with a video background and 0–1% with a still, because GStreamer keeps decoding whether or not anyone pulls the samples. Stopping the pipeline would reclaim it and is deliberately not done — taking it to `Null` frees GLMemory against the context that was just destroyed, which is the hazard rather than the way out of it. |
| L8 | MUST | The compositor restarting, or withdrawing the lock, must leave the session in a state the user can get out of. | On `finished`, the locker destroys the lock object with `destroy` — never `unlock_and_destroy`, which is a protocol error before `locked` — reports why, and exits non-zero. **Met 2026-11-09, after the criterion was asserted and failed.** `finished` was handled, reported and exited **0** — `spike/p3/failure-round.nu` asked, by starting a second locker against a compositor still holding a dead client's lock. It now exits **3**, its own code rather than 1, because to anything that starts this they mean different things: 1 is a locker that broke, 3 is a compositor that would not have it. That round also settled what the second client *can* do, which is nothing: **a supervisor that restarts the binary after a crash is not a way out of a held lock**, because the compositor refuses it. The way out is Hyprland's own — it draws a page saying the lockscreen app died, and `hyprctl --instance 0 eval 'hl.clear_crashed_lockscreen()'` from another tty releases it, verified. What is *not* established is what a Hyprland restart does to a held lock; Hyprland has no in-place restart, so the honest position is that a compositor crash ends the session and the lock with it. **N5.** |
| L9 | MUST | A shipped build contains no unconditional unlock. | `rg` over the crate finds no code path from a keysym or a signal to `unlock_and_destroy` that does not pass through a successful PAM result. **Met 2026-11-09, in the commit that introduced PAM, as this required.** Escape is matched and dropped, so the absence is visible rather than an omission. The `AUROVAL_LOCK_SECONDS` deadline now **exits without unlocking** — it still bounds a run, so `spike/p3/lock-measure.nu` works unchanged, but the operator closes a nested compositor instead of it releasing itself. The one caller of `release` is `poll_auth`'s success branch. The compiler enforced the coupling rather than anyone's memory: the binding cannot be committed without a caller, because `clippy -D warnings` refuses dead code. |
| L10 | MUST | Never submit an authentication attempt the user did not ask for, and never retry one automatically (C19). | A key held down, a window of a thousand synthetic key events, and a PAM failure followed by a reconfigure each produce **at most one** `pam_authenticate` call per deliberate submission. The counter in `pam_faillock` is at three; an automatic retry empties it in under a second and locks the owner out of their own machine for ten minutes. **Met 2026-11-09**, and by a state rather than by care: while an attempt is in flight every key is dropped — not buffered, because a keystroke typed during the two-second failure delay would otherwise land in an attempt the user did not compose. `may_submit` is a free function so the claim has tests, including the thousand-Return burst this criterion names. An empty field is not an attempt and never reaches PAM. |
| L11 | MUST | Being killed leaves the session locked. | `SIGKILL` while locked leaves the session locked, verified deliberately with a second TTY open. Inherited from R14, and **already true of the skeleton**: no signal is handled, and sctk's own lock `Drop` sends `destroy`, with a comment reading *"choosing not to unlock here results in us failing secure."* Observed by accident — `RESULTS.md` §4. |

### Appearance, almost all of it inherited

The point of issue #67 is that this half is not new work. What follows is the
audit the task asked for: for each inherited requirement, whether it carries
over as-is or needs restating, and why.

| greeter | carries over | why |
|---|---|---|
| **R7** video budget (50% of one core, whole present path, 1440p60) | **As-is.** | The budget is a property of the renderer and the output, not of the shell above it. Measured at parity: on one compositor, one clip and one surface size, `xdg_shell` cost 33.2% and the lock surface 32.9%. The protocol adds nothing. The 1440p60-on-a-VT figure is still untaken (L14). |
| **R8** no visible loop seam | **As-is.** | A property of the clip and of `tools/nightshade-loop/`, reached through the same decode path. Nothing about locking touches it. |
| **R9** fallback chain: video, then still, then solid colour | **Restated as L5.** | The chain is identical and the code is literally the same `background` module. What differs is the *consequence of the last step*: for the greeter a solid colour is a cosmetic outcome, and R9's fit criterion says "still renders and still logs in". For a locker the same code path has to be shown not to take the unlock with it, and that is a different test. |
| **background pool and per-launch rotation** | **As-is, and not yet on `main`.** | Landed on `feat/nightshade-pack` and still unmerged (issue #5). A locker launching per lock is exactly the rotation's unit, so it inherits the behaviour with no change — a locked screen shows the next scene, which is the intended effect and arguably better than the greeter's, since locking happens more often than booting. **Expect `config.rs` to conflict when that branch merges:** `main` parses `background` as a flat list of paths and the deployed `/etc/auroval/greet.lua` already holds the pool-of-chains format, which this base refuses with *"wanted a path or a list of paths"*. |
| **R10** one surface per output at its scale | **Restated as part of L2.** | Same requirement, stronger consequence. A greeter that misses an output leaves a monitor black; a locker that misses one leaves a monitor **showing the desktop**. The protocol says so explicitly: *"The client is expected to create lock surfaces for all outputs currently present and any new outputs as they are advertised."* |
| **R11** hyprlock's geometry and typography; palette from the theme | **As-is, and it is now the literal requirement.** | R11 was written to make the greeter look like hyprlock. Here the thing being replaced *is* hyprlock, so the measured match — field 340x56 at the half-pixel, clock cap height 109 against 110 — is a direct statement about the replacement. The palette half is still unmeasured in both. |
| **R12** time-of-day variants | **As-is, SHOULD.** | No locker-specific argument either way. |
| **R16** first frame within 500 ms | **Does not carry over. See N4.** | Hyprland does not send `locked` for 5.00 s on this machine, measured twice, with the client ready in under 30 ms. A client-side first-frame budget would therefore be a number the user never experiences, and writing it as a requirement would make the locker look fast while the screen sat black. |
| **R23** ambient sound | **WON'T.** | greet.md already argued this: *"Login happens when you sit down at a machine you have just turned on. Unlocking happens in a meeting, next to someone sleeping, in an office. The same sound in those two places is not the same sound."* Settled rather than deferred. |
| **C10 / R15** no media path inside a home | **Restated as L12, with a weaker justification.** | See below — this is the one inheritance where the original reasoning does *not* apply. |

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| L12 | SHOULD | No media path reads from inside a user's home directory. | With `/home/<user>` at mode 700, the configured background still renders on the lock screen. **This is a SHOULD here where R15 is a MUST, and the reason matters.** C10's argument is that *the greeter runs as a different user* and cannot traverse a home at mode 700. A locker runs as the logged-in user and can read their home perfectly well, so the original justification is simply absent. What remains is weaker but real: the two surfaces share one `background` chain and one config, and a chain that renders on the lock screen and falls back to a colour on the login screen is a worse failure than one refused in both places. The skeleton therefore **reports** the check and does not enforce it. |
| L13 | SHOULD | Show who is locked out — the session's username, and an avatar from the system path R19 already uses. | With a PNG at `/var/lib/AccountsService/icons/<user>`, it renders; without, a placeholder does and nothing is logged as an error. Cheap, since R19's code is in the shared crate. **The username half is met 2026-11-10, and it was not met before in a way worth recording.** `view` has always drawn `ui.username` above the field, so this looked free — and the locker never set it. The line read `—` until the first keystroke and then filled with the *placeholders for the password*, because `push` routes to `username` in the stage the locker was in (N8). So the requirement that asks the screen to say who is locked out was showing a row of dots where the name goes. The locker now sets it once from `getpwuid`, before the surface exists, and `typing_to_unlock_leaves_the_name_alone` asserts the keyboard cannot reach it. **Seen on 2026-11-10, not only asserted.** The maintainer ran a rehearsal in the nested compositor and read the screen: his name above the field, the prompt beside it, dots as he typed. That sentence is the whole difference between this row and the version of it that was true in a test while the screen showed a row of dots where the name goes — which is what it had been doing. **The avatar half is untouched**, and R19's loader is still the cheap way in. |

### WON'T (v1)

Idle management; "switch user"; media controls; notifications of any kind;
password changes; faillock reset; fingerprint or smartcard; an emergency
bypass; ambient sound; anything that spawns a process.

## Q6, answered: separate binaries over a shared library

greet.md's Q6 asked *"Does the two-mode design hold, or does lock mode become
its own binary?"*, with the test being *"if the renderer shares cleanly
through P1 and P2, one binary; if the lock path needs materially different
plumbing, split."* Both halves of that test now have answers, and they point
opposite ways, so the resolution is neither of the two options as written.

**The renderer shares perfectly.** `crates/auroval-ui/` was extracted from
`crates/greet/` as a pure move — git recorded every shared module as a rename
with a zero-byte diff — and the test count did not change. `render::Gl::target`
takes a `wl_surface` and nothing else, so it never knew which protocol handed
that surface out. Eleven modules moved: renderer, video, background, view,
theme, config, clock, ui, store, dump, sessions.

**The plumbing is materially different.** Nothing in `shell.rs` moved, because
none of it applies. The greeter runs on **`xdg_shell`** — a fullscreen
`xdg_toplevel`, because it runs under `cage` which owns the output and the
keyboard for it. The locker runs on `ext-session-lock-v1`, where surfaces are
output-sized by the compositor, nothing may be committed before the first
`configure`, and the sizes in that `configure` are *"exact requirements"*. On
top of that, the greeter's auth is a socket conversation on a worker thread
and the locker's is `pam_authenticate` in-process (C20).

So: **one library, two binaries.** The shared half is 85% of the code by line
count and all of the measurements; the unshared half is the part where getting
it wrong has entirely different consequences in each binary. A single binary
would have had to carry greetd and PAM in one address space and decide between
two protocols at runtime, which is the arrangement where a greeter bug becomes
a lockout.

**Recommendation: close Q6 as "separate binaries over a shared library".**

## Phasing

- **P3a — design and skeleton. This document, and the PR it comes with.**
  The library split, the protocol proved, the video measured on a lock
  surface, no authentication written. Done.
- **P3b — the unlock path.** L1, L3, L4, L9, L10. Written with the maintainer
  awake and a second TTY open, because this is where a mistake locks someone
  out of their own machine. L9 deletes the skeleton's escape hatch in the same
  commit.
- **P3c — the failure policy, tested.** L5–L8 and L11, each as a deliberate
  test rather than an argument. L11 needs a second TTY; L7 needed a way to
  destroy a GL context on purpose. **Unattended half done 2026-11-09** —
  `spike/p3/failure-round.nu`, five cases green and one bug found (L8's exit
  code). **L7 added and green 2026-11-10**, six of six, with the hook the
  round had been missing. **And the attended run done the same day**, which
  nothing unattended can replace: the round never types a password, because
  L10 forbids retrying and `pam_faillock` counts to three. Two rehearsals in
  the nested compositor, password typed by hand — the second blind, with the
  GL context destroyed — and both unlocked. **P3c is complete.** The sitting
  also produced the finding in **N8**: the screen asks *"Who are you?"*.
- **P3d — the 1440p rehearsal.** L14 below. A real Hyprland on a free VT.
- **P3e — the changeover.** A `pc-setup` change that points the idle manager
  at `auroval-lock`, with hyprlock still installed. **Deliberately last, and
  the order inside it matters**: an install step and a written rollback
  first — **done 2026-11-10**: `deploy/install.nu` builds, stages, digests
  and installs `/usr/local/lib/auroval/auroval-lock`, and
  `deploy/LOCK-TRIAL.md` is the rollback, written before the install line
  existed. Installing it enables nothing: no keybind is written, `hypridle`
  and `SUPER + L` still call hyprlock, hyprlock is not even disabled, and a
  bare `auroval-lock` exits 2 because the only mode that locks is `rehearse`
  behind `AUROVAL_LOCK_REHEARSAL=1`. **A production mode is the next step and
  a decision of its own** — it is the commit that makes a bare invocation
  able to lock a screen, and nothing should get a keybind before it exists.
  Then a second deliberate keybind, so the locker is used by choice while `SUPER + L` and the idle timeout stay on hyprlock, and only then the
  timeout itself — a lock you did not ask for is the one you are least ready
  for. No supervisor: P3c established that nothing can restart into a held
  lock, so the recovery story is the compositor's one command from another
  tty, written down before it is needed exactly as `deploy/TRIAL.md` does. hyprlock comes off only
  after greet.md's own metric is met: *"used daily for two weeks and no
  lockout incident."*

## Success metrics

- Unlocking shows the scene, not a blurred screenshot of your own windows.
- The lock screen's palette matches the desktop's and the greeter's, ending
  the last Catppuccin pin on the machine.
- `auroval-lock` is used daily for two weeks with no lockout incident, and
  only then is hyprlock removed.
- Nobody has had to reach a second TTY to get back into their own session.

## Open questions

| # | Question | What would settle it | Owner |
|---|---|---|---|
| N1 | Does the lock screen get its own config file, and what may it override? | The skeleton reads `/etc/auroval/greet.lua`, because inventing `/etc/auroval/lock.lua` in a commit that installs nothing would create a path that exists in code and nowhere on disk. **Recommendation: one `/etc/auroval/auroval.lua` with an optional `lock = { … }` table overriding named keys**, rather than two files that drift — which is C21's argument about two owners, applied one level down. The sharpest sub-question is the background: identical loops are the point, so the default must be shared, and only a theme that deliberately differs should say so. **Now partly answered by a file mode, not by taste** — see N7. | Maintainer |
| N2 | ~~What writes the `auroval-lock` PAM stack, and is it a copy of `hyprlock`'s or a symlink to `login`?~~ **Answered, and the prediction held: `pc-setup` writes it as a copy under its own service name.** | [dlouwers/pc-setup#3](https://github.com/dlouwers/pc-setup/pull/3) writes `/etc/pam.d/auroval-lock` with one effective line — `auth include login` then, `auth include system-auth` since [#4](https://github.com/dlouwers/pc-setup/pull/4) — under a service name of its own so `faillock` can be tuned for a lock screen without touching `sudo` or `login` (C19). It carries a consequence the implementation must obey: **C24**. | Closed |
| N3 | Should `ui::UiState` stop carrying a session list? | It carries one because the greeter preselects the last session, so `ui` depends on `sessions` and both had to move into the shared crate together; a locker constructs it with an empty list. Breaking that means making `UiState` generic over its session type, which is a design change and was deliberately not made in a refactor whose whole claim was that it changed no behaviour. **Recommendation: leave it until a second thing wants it.** One unused field is cheaper than a type parameter threaded through the view. | Maintainer |
| N4 | Why does Hyprland wait 5.00 s before sending `locked`, and is it configurable? | Measured twice at exactly 5000 ms from process start, with the first locked frame 23–29 ms later — the flatness says deliberate delay, not work. Not investigated further: it may be a Hyprland setting, it may be specific to the nested backend. **It is why R16 does not carry over**, and until it is understood a first-frame budget for the locker would be measuring the wrong thing. Reading Hyprland's source for its session-lock handler would settle it. | Maintainer |
| N5 | What actually happens to a held lock when the compositor restarts? | L8 is written for it and only half-verified. Hyprland has no in-place restart, so the honest position today is that a compositor crash ends the session and takes the lock with it — which is not a lockout but is also not "survives the compositor restarting under it", which is what issue #67 asks for. A config reload, which does not restart the compositor, is the case that can be tested cheaply and has not been. | Maintainer |
| N6 | Does the skeleton's `AUROVAL_FRAME_DUMP` have to be refused while locked? | It is in the shared crate and writes PNGs of the screen, which on a locked screen includes a field the user is typing into. L4 says masked or refused; **recommendation: refused outright while locked**, because "masked" is a claim that has to stay true through every later change to the view. | Maintainer |
| N7 | Should a locker rotate between R9's background candidates, and against what state? | **No, in the skeleton, and the reason is a permission rather than a preference.** #66 made `background` a pool of chains and `greet` picks between them with `rotation`, which remembers its place in `/var/lib/auroval/last-background`. That directory is `drwxr-x--- greeter greeter`: a process running as the logged-in user cannot read it, let alone advance it, so a locker cannot participate in the greeter's rotation at all without either loosening that mode or keeping a second index somewhere the user can write. Both are real changes with their own arguments, and neither belongs in a merge resolution. So `crates/lock` takes **candidate 0, every time**, and says so in `probe`. The consequence worth stating plainly: **the two screens agree only when the greeter happens to be showing candidate 0**, which undercuts N1's "identical loops are the point" exactly as often as the pool is long. If that matters more than the rotation does, the answer is for a locker to read a shared *chosen* value rather than a private *index* — but that is a design change to `rotation`, and it is the maintainer's. | Maintainer |
| N8 | ~~The lock screen asks **"Who are you?"**, and the field it types into cannot hide anything. What stage should a locker be in?~~ **Answered: its own.** | Found by the maintainer on 2026-11-10, in the first attended rehearsal, as *"heel vreemd dat er 'Wie ben je?' staat en dat er een password wordt verwacht"* — and it was not a wording problem. `UiState` was built in `Stage::AskingUser` and never left it, which gave three consequences from one shortcut: the hint was the greeter's identity placeholder; `field_display` computed `hidden = stage != AskingUser && secret`, so **hidden was always false** on the one screen that is entirely a password; and `push` routes to `username` in that stage, which `view` draws above the field — so the placeholders appeared twice and before the first keystroke the line read `—` where the owner's name belongs. `Stage::Unlocking` now holds the secret in `typed`, hides it by naming the stages that hide rather than the one that does not, and takes its hint from the conversation. The locker sets `username` once, from `getpwuid`, which is **L13's first half**. **Confirmed on the screen the same day**, by the person who found it: the hint reads the prompt and the line reads his own name. Nothing here may call a screen fixed on the strength of a test about it — this question exists because six merged PRs did. | Closed |

## What would change this plan

- **hyprlock gains video support upstream.** Then L2–L4 and L9–L11 are all
  cost with no benefit, and the right project is a config bridge plus a
  palette fix. greet.md listed this as a risk to P3 and it is still live; C22
  says nothing about it being hard for hyprlock's maintainers, only that it is
  not configurable today.
- **The 1440p rehearsal exceeds R7's ceiling** (L14). Then the locker needs
  the same treatment the greeter's readback path got: measured, named, and
  either fixed or the budget revised with a receipt. The parity measurement
  makes this unlikely, since the greeter holds the budget on the same
  renderer.
- **`faillock` locks the maintainer out once** (C19). Then L10 stops being a
  careful requirement and becomes the thing the whole design is arranged
  around, and the right answer may be that the locker refuses to submit more
  than once per N seconds regardless of what the user does.
- **A second monitor arrives.** L2's hotplug half and R10 both move from
  argued to exercised, and a missed output on a locker shows the desktop.
- **PAM in-process turns out to block.** `pam_authenticate` against a stack
  with `pam_systemd_home` or a network module can take seconds. If it blocks
  the thread that draws, L7's reasoning applies in reverse and the
  conversation has to move off the render thread — which is what the greeter
  already does for greetd, so the pattern exists.

## L14 — the measurement that is missing, written as a requirement

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| L14 | MUST | R7's budget is met on a lock surface at the output's native resolution and refresh rate, measured in a real session compositor. | A 1440p60 loop on an `ext-session-lock-v1` surface holds 60 fps with no dropped frames over 60 s, for no more than 50% of one core, with frames in `memory:GLMemory`. **Not met, and not inferable from what has been measured.** Today's figures are from a nested Hyprland at 1259x679 logical, scale 1, against a 30 fps clip, which differs from R7's conditions in resolution, rate and nesting — and R16's note in greet.md records a 1440p figure that was withdrawn for being *"a nested `cage` ... measuring the nesting"*. `cage` cannot stand in (C23). This needs a real Hyprland on a free VT with someone present and a second TTY open. |
