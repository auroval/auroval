# greet — Product Requirements

Status: **draft for review** · Org: `auroval` · 2026-11-07

## Summary

`greet` is auroval's login and lock screen for Hyprland, showing an animated
background in the way macOS and tvOS do. It is one component of the suite; the
suite's [vision](../vision.md) and [tenets](../tenets.md) are upstream of this
document and win where the two disagree. It renders as a Wayland client, speaks
the greetd IPC protocol to authenticate, and is configured in hyprlang — the
same language Hyprland and hyprlock already use.

It is also meant to support logging in with a cloud identity — an OIDC provider
such as Google Workspace or Microsoft Entra ID — and to show the profile picture
that comes with it, without implementing any of that authentication itself.
See C11.

Two surfaces today look nothing like the desktop they guard: SDDM's unthemed Qt
greeter, and hyprlock, which is themed but can only show a still image.
`greet` replaces the first and is intended to replace the second.

## The decision this document should enable

1. **Is the two-mode design right?** One binary that is both a greeter (greetd)
   and a locker (ext-session-lock), or a greeter only, leaving the lock screen
   to a patched hyprlock?
2. **Is the remaining gap worth a project?** Prior art does not cover the goals
   as a set, but it is not empty either: an animated login background is
   available off the shelf today (C7). What is unserved is the lock screen, the
   visual identity and multi-monitor. The decision is whether those three are
   worth writing a greeter for — not whether animation is possible.
3. **Does cloud identity belong in v1?** It costs nothing in `greet` itself
   (C11) but everything in the PAM layer underneath, which this project does not
   control.

Everything else here follows from those two answers.

## Goals

- The login screen and lock screen are visually continuous with the desktop —
  same palette, same font, same geometry — so that no surface looks borrowed.
- A background can move: a looping video, hardware-decoded, at the monitor's
  native resolution and refresh rate, without the login taking noticeably
  longer to appear.
- Authentication is never the thing that breaks. A failure in theming,
  decoding or config parsing degrades the appearance, never the ability to log
  in.
- Logging in with a cloud identity is possible and looks native — including the
  profile picture that comes with it — without `greet` ever handling a token,
  a secret or a network connection.
- Someone running Hyprland on another distro can install it, point it at a
  video and have it work, from the README alone.

## Non-goals

These are refusals, not "later". Each would arrive disguised as an obvious
extension.

- **Not a display manager.** greetd owns PAM, seats, TTYs and session launching.
  `greet` is a client. If a requirement needs privileged seat management, it
  belongs upstream in greetd, not here.
- **Not an X11 greeter.** Wayland only. No Xorg code path, no `xsessions`
  rendering path beyond listing such sessions for greetd to start.
- **No autologin.** That is greetd configuration, and a login screen that can
  skip itself is a security surface with no owner.
- **No OIDC, OAuth or SSO implementation.** `greet` holds no client secret,
  makes no network request, and validates no token. Cloud identity is a PAM
  concern (C11). `greet`'s job is to render the prompts that arrive and to
  send back the answers. A design in which the greeter talks to
  `accounts.google.com` is wrong and must be refused: the greeter is an
  unauthenticated process on a locked machine.
- **No user management** — no account creation, password changes, or
  "add user" flows.
- **No remote login.** No XDMCP, no VNC, no network protocol of any kind.
- **No general-purpose wallpaper daemon.** It draws its own background on its
  own surfaces; `awww` keeps the desktop.
- **No theme marketplace.** Themes are not discovered, downloaded, rated or
  installed by `greet`. A theme is a path in a root-owned config file.
- **No unrestricted scripting in the config.** This non-goal used to read *"no
  scripting language in the config — hyprlang describes a look; it does not
  run"*, and Q4's answer contradicts it, so it is rewritten here rather than
  bent. Themes are Lua, and Lua runs. What remains refused is the part that
  mattered: a theme never gets the host's capabilities. The interpreter is
  created without `io`, `os`, `package`, `require`, `debug`, `load`, `dofile`
  or any means of opening a file, a socket or a process, and it runs under an
  instruction and memory ceiling. A theme computes values and returns them; it
  cannot reach the machine it is drawing on.

  **The threat model this accepts.** The greeter is an unauthenticated process
  on a locked screen, so theme code executes before anyone has proved who they
  are. The mitigation is that a theme has no capabilities to abuse and cannot
  outlive its ceiling, not that themes are trusted. What is *not* claimed is
  that a Lua sandbox is as strong a boundary as a separate process or a
  WebAssembly instance — `docs/ideas.md` makes that argument and it still
  stands. This is a weaker guarantee than "it does not run", knowingly taken,
  because the thesis requires a theme to determine *"hoe die switchen"* and no
  declarative format expresses that.

## Constraints, with receipts

Each of these was checked on the target machine or in a primary source on
2026-11-07. Unverified items are marked as such and must not be treated as
settled.

**C1 — greetd IPC is small and fully specified.** From `greetd-ipc(7)`: *"The
path to the socket is stored in the environment variable GREETD_SOCK"*,
`<payload-length> <payload>` where *"<payload-length> is a 32-bit integer in
native byte order"* and *"<payload> is a UTF-8-encoded JSON string."* Four
requests (`create_session`, `post_auth_message_response`, `start_session`,
`cancel_session`), three responses (`success`, `error`, `auth_message`).

**C2 — the auth flow is a loop, not two fields.** Same source: *"There are no
limits on the number and type of messages that may be required for
authentication to succeed"* and *"a greeter should not make any assumptions
about the messages."* A greeter hard-wired to username+password is wrong, and
breaks the moment a PAM stack asks anything else — a second factor, a password
expiry notice, or a provider's device code (C13).

**C3 — hyprlang has no C ABI, so Rust cannot bind it. Moot since Q4.** The
package ships one header, `/usr/include/hyprlang.hpp`, and `grep 'extern "C"'`
over it returns nothing, so choosing Rust would have meant reimplementing the
parser or bridging C++. A third-party crate exists — `hyprlang` on crates.io,
pest-based, from `spinualexandru/hyprlang-rs`, self-described as *"not endorsed
by or affiliated with the Hyprland project"* — and its licence, maintenance and
parity were never verified because they no longer need to be.

Q4 chose Lua, so there is no hyprlang parser to write, adopt or bridge. The
binding is `mlua`, which is **MIT**, over Lua itself, also MIT — both checked
against their own manifests, and both compatible with this project's
`MIT OR Apache-2.0`. This constraint is kept rather than deleted because it is
most of the reason Q4 was hard: the cheapest-looking option carried a parser
nobody wanted to own.

**C4 — hyprlock cannot show video, and this is not a configuration gap.**
`strings /usr/bin/hyprlock` yields no match for gstreamer, ffmpeg, libav, mp4 or
video. Its image loader, hyprgraphics, advertises only still formats
(png, jpeg, bmp, svg, jxl, webp, avif). Animated lock-screen backgrounds are
unreachable without new decoding code somewhere.

**C5 — a self-written locker fails closed, by protocol design.**
`ext-session-lock-v1.xml`, installed locally: *"If the client dies while the
session is locked the session remains locked, possibly permanently depending on
compositor policy."* A crash cannot expose the desktop. The cost is in that
clause — a crash can lock you out until a VT switch or reboot.

**C6 — hardware video decode works in the greeter's context. Verified by the
P0 spike, 2026-11-07.** Both halves of this constraint are now settled, and the
first half needed correcting.

*The working decode path is NVDEC through GStreamer's `nvcodec`, not VA-API.*
`vainfo` does report *"VA-API NVDEC driver [direct backend]"* with
`VAProfileH264High` and `VAProfileHEVCMain/Main10/Main12` at `VAEntrypointVLD`,
and that is true — but GStreamer cannot use it. The VA-API plugin is a separate
package (`extra/gst-plugin-va`), and once installed its plugin loads with **0
features** on this machine, so no `va*` element exists to be selected. The
hardware decoders GStreamer actually has are `nvh264dec` and `nvh265dec` at
rank 257, above libav's 256, and they go through `/dev/nvidia*` and never touch
`/dev/dri/renderD128`. **A `vainfo` listing is not evidence that a GStreamer
greeter can decode in hardware**, which is the mistake the original wording of
this constraint made.

*It works as greetd's own user, with no session at all.* Measured as uid 950
(`greeter`) via `runuser` with `env -i`: no `XDG_RUNTIME_DIR`, no D-Bus, no
`WAYLAND_DISPLAY`, no driver hints, `HOME=/`. Twenty seconds per run, 1440p60,
on the target monitor's native resolution and refresh rate:

| | H.264 High | HEVC Main |
|---|---|---|
| decoder `decodebin3` chose unaided | `nvh264dec` | `nvh265dec` |
| sustained rate | 59.50 fps | 59.70 fps |
| late buffers | 0 | 0 |
| CPU | **2.5%** of one core | **2.9%** |
| software baseline, same clip | 36.4% | 54.0% |
| frame memory | `memory:CUDAMemory` | `memory:CUDAMemory` |

The 14–19× CPU gap against the software baseline is the evidence that the GPU
is doing the work, not an assertion that it is. Decode ceiling with the clock
released: **549.65 fps at 1440p, or 9.2× real time**.

*Why the greeter's user turned out not to matter.* `/dev/nvidia0`,
`/dev/nvidiactl`, `/dev/nvidia-modeset` and `/dev/nvidia-uvm` are all mode
`0666`, and Arch's `greetd` package already places `greeter` in the `video`
group — so the device access a greeter needs is there before anything is
configured. This is a property of this driver and this packaging, not a general
guarantee: a VA-API-only machine would depend on `/dev/dri/renderD128`, which is
`0666` here but is `0660 root:render` on distributions that tighten it.

*Portability: VA-API is the more portable API and the weaker path here.*
Measured on the same clip, 2026-11-07, after forcing the plugin past its own
filter:

| | NVDEC `nvh264dec` | VA-API `vah264dec` |
|---|---|---|
| 1440p60 rate | 59.50 fps | 59.90 fps |
| late buffers | 0 | 0 |
| CPU | 2.5% of one core | 3.8% |
| frame memory | `memory:CUDAMemory` | plain `video/x-raw` |

VA-API decodes in hardware and holds the rate, but hands frames back in
**system memory**, so it fails R7's no-readback budget on this machine. The
reason GStreamer refuses it is explicit rather than accidental:
`_gst_va_display_filter_driver` matches the driver's vendor string against an
allowlist and logs *"Unsupported driver: VA-API NVDEC driver [direct
backend]"*. `GST_VA_ALL_DRIVERS=1` bypasses that filter, which is how the
figures above were obtained — and the measurement shows the filter is a
reasonable judgement, not an oversight.

This does not make VA-API the wrong choice elsewhere. On Intel (`iHD`) and
Mesa/AMD (`radeonsi`) it is the *only* hardware path, its vendor string is on
the allowlist, and the `va` plugin exports DMABuf rather than copying down. So
the portable position is not to pick one API:

- **Never name a decoder.** `decodebin3` rank-selects, and it chose
  `nvh264dec` unaided here — and still chose it with the VA-API filter
  bypassed, since `nvcodec` outranks `va`. The same pipeline would select
  `vah264dec` on AMD or Intel with no code change. Portability costs nothing
  as long as no element is hard-coded.
- **Do not assume where frames live.** This is the real cost, and it lands on
  Q5: the renderer must accept `memory:CUDAMemory`, `memory:DMABuf` and
  `memory:VAMemory`, and fall back to system memory per R9. A renderer written
  against CUDA memory alone would work on this machine and nowhere else.

**Still unverified:** every VA-API figure here comes from NVIDIA's shim. Real
Intel and AMD behaviour, including whether DMABuf export actually avoids the
readback, has not been measured on that hardware. That is a P4 question.

Reproduce with `spike/p0/greeter-decode-test.nu`; full output in
`spike/p0/RESULTS.md`.

**C15 — the greeter has nowhere to cache, and it costs about 0.9 s of every
login.** greetd's greeter user has `HOME=/`, which is not writable. GStreamer
therefore cannot persist its plugin registry and rescans every plugin on every
start; fontconfig reports *"No writable cache directories"* for the same reason.
Measured from process start to GStreamer ready, as the `greeter` user:

| | |
|---|---|
| no writable cache (the default) | **1018 ms** |
| writable cache, registry absent | 1011 ms |
| writable cache, registry warm | **93 ms** |

**C16 — the greeter cannot reach a sound device at all, today.** Checked on
the target machine on 2026-11-07, because R23 asks for ambient audio and this
is the assumption underneath it. Three facts, and together they close the door:

| | |
|---|---|
| Sound server | PipeWire 1.6.10, **per-user units only**; `systemctl list-unit-files` shows no system-wide `pipewire` or `pulseaudio` |
| Device nodes | `/dev/snd/*` are `root:audio 0660`, plus a POSIX ACL granting `user:dirk:rw-` — logind's, for the **active session's** user |
| The greeter user | uid 950, groups `greeter` and `video`. **Not `audio`.** And P0 established it has no session, so logind grants it no ACL either |

So the greeter has neither a sound server to talk to nor permission to open the
device directly. This is the same shape as C6 — an assumption about what works
in a context with no session — and it has the same answer available: measure it
before building on it.

Two routes, and neither is free. Adding `greeter` to the `audio` group works
without a session, but group `audio` is not playback-only: it carries the
capture devices too, so a pre-authentication process would hold the
microphone permanently. Giving the greeter a logind session gets the ACL
instead, which is narrower, but it changes how greetd is deployed. **Q10.**

**Why R23 is silent by default, and short.** The maintainer's framing is that
this is something Apple does not do and it would be *"super cool"* — which is a
first-impression argument, and [tenet 3](../tenets.md) says the fiftieth look
beats the first. A sound that plays on every single login is close to the
canonical example of what that tenet warns about: delightful in week one,
something you reach to mute in week three, and by then it is attached to the
one screen you cannot skip. The tenet does not forbid it; it sets the terms.
Short, quiet, theme-supplied, and off unless a theme asks for it. A sound that
is still wanted after two weeks of gate 3 has earned its place; one chosen from
a demo has not.

The lock screen is a separate question and the answer there is probably no.
Login happens when you sit down at a machine you have just turned on. Unlocking
happens in a meeting, next to someone sleeping, in an office. The same sound in
those two places is not the same sound.

So a writable cache directory for the greeter — `XDG_CACHE_HOME` and
`GST_REGISTRY` pointed somewhere it owns — is part of deploying this, not an
optimisation: it is worth more than nine tenths of a second on every single
boot, which is most of R16's budget. `greet` sidesteps the font half
independently by loading font files by path rather than through fontconfig,
because a font that needs a cache is a font that fails in the one context that
matters.

Deploying that cache is `deploy/auroval-greet.sh` and `deploy/auroval.tmpfiles.conf`;
see `deploy/README.md`.

**C7 — prior art already solves the animated login background; it does not
solve the rest.** This matters more than it is comfortable to admit, so the
quotes are verbatim from ReGreet's README:

- Animation is covered: its requirements list *"GStreamer (for background
  animated images & video; supported formats depend on installed plugins)"*.
  So `greetd` + ReGreet + a video file is an animated login screen **today**,
  with no code written. Animation alone does not justify this project.
- One output only: *"which is useful since ReGreet is a single-monitor
  application"*.
- Blur is not mentioned anywhere in the README, and its theming model is GTK
  CSS, which will not reproduce hyprlock's geometry without fighting it.
- It is a greeter only. Nothing in the README concerns session locking, and
  greetd has no part in locking an already-running session.
- GPL-3.0-or-later, so code cannot flow from it into a permissively licensed
  project — though that constrains reuse, not evaluation.

The honest residue: `greet` is justified by the **lock screen**, the **shared
visual identity** across both surfaces, and **multi-monitor** — not by moving
pixels on the login screen.

**C8 — the current greeter runs on Xorg.** `/usr/lib/sddm/sddm.conf.d/default.conf`
sets `DisplayServer=x11` and the journal confirms *"Starting X11 session:
/usr/bin/sddm-greeter-qt6"*. `xorg-server` is installed for this and
essentially nothing else; `/usr/share/xsessions/` does not exist.

**C9 — "like Apple" covers two different things, and the answer is the video
one.** tvOS aerials are HEVC video loops; macOS dynamic desktops are HEIC
containers holding several stills keyed to time of day or light/dark. Q1 is
settled: **video loops**, decided 2026-11-07. So the tvOS model is the subject
and the macOS one is the secondary feature — the requirements' existing
assumption is now a decision, and R12 stays a Should rather than becoming a
Must. HEVC loops are a measured path rather than an aspiration: C6 found
`nvh265dec` holding 1440p60 for 2.9% of one core.

**C10 — the greeter runs as a different user than the desktop.** A greeter runs
as its own unprivileged system user, which cannot traverse a home directory at
mode `0700` — the default on many distributions. Verified by attempting the read
as the greeter's own user, which fails. Any media the greeter shows must
therefore live outside the user's home, or be copied out when it changes. This
is a property of greeters in general, not of any one display manager.

**C11 — cloud login happens in PAM, below `greet`.** greetd ships its own
stack at `/etc/pam.d/greetd` (`pacman -Fl greetd`), and greetd authenticates
through it. An OIDC provider is therefore added by installing a PAM module and
editing that file — not by writing code here. This is what makes R1 load-bearing
rather than theoretical: whatever that module prompts for arrives as
`auth_message` and must be rendered without the greeter understanding it.

**C12 — a working OIDC path exists, but not as a native package.**
Canonical's authd supports both Microsoft Entra ID and Google IAM. For Google
it uses the OAuth 2.0 device authorisation grant, the issuer is
`https://accounts.google.com`, and the OAuth client must be of the type intended
for *televisions and limited-input devices*, since that is the type permitted to
use the device code flow. Entra ID additionally supports automatic device
registration; Google IAM does not. authd's brokers ship as snaps (for example
`snap install authd-google`), and `snapd` is **not** in the Arch repositories
(`pacman -Si snapd` finds nothing), so an Arch-based target needs another
route. **Unverified and important:**
whether authd's PAM module works under greetd at all — its documented desktop
path is GDM, which has purpose-built support including broker selection on the
login screen. The alternatives are the several unrelated `pam_oauth2_device`
projects, whose maturity, licences and suitability for local (rather than SSH)
login are **unverified**.

**C13 — the device code flow is a conversation, not a password field.** The
provider returns a URL and a short code which the person must open on another
device. In PAM terms that is an `info` message, possibly followed by a long
wait. Canonical notes the default 60-second login timeout usually has to be
raised. A greeter that renders `info` messages small, or that times out on its
own, makes this flow unusable.

**C14 — the profile picture can only ever be shown from the second login
onwards.** The `picture` claim lives in the ID token, which exists only after
authentication succeeds. Before that the greeter does not know who is at the
keyboard and must not be able to ask the provider. So the picture has to be
written to local disk by whatever completes the login, and read back next time.
A world-readable location already exists and is the convention:
`/var/lib/AccountsService/icons/<user>`, present and `drwxrwxr-x` with
accountsservice installed. SDDM's own default config uses the same
pattern — *"The files should be named <username>.face.icon"*. This also sidesteps
C10, since nothing is read out of the user's home.

### Ruled out

- **Forking hyprlock** (BSD-3-Clause, so permitted). Rejected: it would inherit
  the look for free, but commits the project to C++ and to tracking an upstream
  whose config semantics assume an unlocked session to screenshot.
- **Patching video support into hyprlock upstream.** Rejected as the primary
  route: it fixes the lock screen and does nothing for login, and it puts the
  schedule in someone else's review queue. Still worth doing afterwards as a
  contribution.
- **An SDDM QML theme.** Rejected: keeps the greeter on Xorg (C8), and SDDM
  themes cannot be a lock screen, so the two surfaces would diverge again.
- **Baseline: greetd + ReGreet with a video background, and keep hyprlock.**
  This is the "do nothing new" option and it is genuinely viable for part of the
  goals: an animated, Wayland login screen this week. Rejected because it leaves
  the lock screen a still image (C4), caps the machine at one monitor, and keeps
  two surfaces that do not match each other or the desktop. Anyone evaluating
  this PRD should price the project against this baseline, not against SDDM.
  If the lock screen stops mattering, this baseline wins and `greet` should
  not be built.
- **Implementing OIDC inside the greeter.** Rejected on security grounds, not
  effort: it would put a client secret and an outbound network connection in an
  unauthenticated process that renders on a locked screen, and it would
  duplicate PAM badly. Non-negotiable.
- **Writing a display manager.** Rejected: PAM, seat and TTY handling is the
  part where mistakes lock people out of their machines, and greetd already
  does it.

## Requirements

Priority is MoSCoW: **MUST** is release-blocking; **SHOULD** is expected but can
slip a release; **COULD** ships if cheap; **WON'T** is out of scope for v1.
Every requirement carries a fit criterion — a check that settles whether it is
met.

### Authentication and session

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| R1 | MUST | Authenticate by driving the greetd auth loop, making no assumption about the number, order or type of messages (C2). | A PAM stack carrying an extra `pam_exec` prompt completes login with no code change. A fixed two-field implementation fails this. |
| R2 | MUST | Offer the sessions found in the configured session directories and start the one chosen. | Both `hyprland.desktop` and `hyprland-uwsm.desktop` are listed, and each starts the session it names. |
| R3 | MUST | Surface `auth_message` of type `info` and `error`, and treat `auth_error` as non-fatal. | A wrong password shows the message and returns to the prompt; the process does not exit and the session is cancelled cleanly. |
| R4 | MUST | Remember the last user and the last session, per machine, outside any user's home. | After a login as user A to session X, both are preselected on the next boot. |
| R5 | SHOULD | Offer suspend, reboot and power off. | Each action is reachable by keyboard alone and takes effect. |
| R6 | SHOULD | Show keyboard layout and warn on caps lock. | With caps lock on, a warning is visible before submitting. |

### Identity providers

`greet` implements none of these protocols. Each requirement below is about
rendering and plumbing only.

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| R17 | MUST | Render `info` messages as first-class content, not a status line: large enough to read a URL and a code from a seated distance, and a QR code when the message contains a URL. | A device-code prompt is readable and scannable from a phone at arm's length, without selecting or scrolling text. |
| R18 | MUST | Impose no timeout of its own on an in-progress auth conversation; let PAM and greetd own the deadline (C13). | A conversation left idle for five minutes still completes when answered; nothing in `greet` cancels it. |
| R19 | SHOULD | Display a user's avatar from a system path, defaulting to `/var/lib/AccountsService/icons/<user>` (C14), with a graceful placeholder when absent. | With a PNG placed there, it renders; with the file removed, the placeholder renders and nothing is logged as an error. |
| R20 | SHOULD | Let the person choose between several configured authentication mechanisms when the PAM stack offers more than one. | With two mechanisms available, both are selectable by keyboard and each reaches its own prompt. |
| R21 | COULD | Ship a documented hook that writes the `picture` claim to the avatar path after a successful cloud login. | Second login after a first cloud login shows the provider's picture. Deliberately a hook, not a built-in: whoever holds the token writes the file. |

### Appearance

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| R7 | MUST | Play a looping video background, hardware-decoded, at the output's native resolution and refresh rate. | A 1440p H.264 loop holds the output's refresh rate with no dropped frames over 60s, measured. **Budget, revised 2026-11-07 after Q5.** *Decode:* no more than 5% of one core; P0 measured 2.5% with frames in `memory:CUDAMemory`, and a software decode of the same clip costs 36%. That ceiling stands. *The whole present path, decode included:* **no more than 50% of one core at 1440p60**, with frames never leaving the GPU. Measured under `cage` on a free VT — the greeter's own context — over five alternating pairs at a held 60 fps: the GL path averages **33.7%** (26.2–41.7) and a `wl_shm` readback averages **68.8%** (67.4–69.3). So the ceiling sits above the worst observed GL run with room, and the readback path fails it, which is the point. The ratio matters more than either figure: presentation costs more than an order of magnitude beyond the 2.5% decode above it, so a decode-only budget constrains the cheap half. An earlier draft set this same 50% from a measurement taken inside a live desktop session and it was withdrawn, because the same binary read 44% there on a quiet machine and 60% on a busy one. See [`spike/q5/RESULTS.md`](../../spike/q5/RESULTS.md). |
| R8 | MUST | Loop without a visible seam. | Frame-to-frame diff across the loop boundary is no larger than between two ordinary consecutive frames. |
| R9 | MUST | Fall back, in order, to still image then solid colour, when decode is unavailable. | With GStreamer plugins made unavailable, the greeter still renders and still logs in; the fallback is logged, not silent. |
| R10 | MUST | Render one surface per output at that output's scale. | On two outputs of differing scale, text is sharp on both and neither is letterboxed or cropped. |
| R11 | MUST | Reproduce hyprlock's geometry and typography — MesloLGS Nerd Font, the large clock, and the rounded input field at the desktop's own corner radius and rounding power. Colour comes from the active theme's palette, not from a fixed scheme. | Side-by-side with hyprlock at the same resolution, the font, the clock size and the input field's size and rounding are identical. Palette values match the active theme's table and are byte-identical across greeter, lock and desktop. |
| R12 | SHOULD | Support time-of-day or light/dark background variants (C9). | A config naming two sources renders the one matching the current time, verified by faking the clock. **Confirmed as Should by Q1**, which chose video loops as the subject: this is the clock making a moving background stay interesting, not a substitute for motion. A build that shipped only this and no loop would fail tenet 2. |

**R11 dropped its Catppuccin Mocha pin on 2026-11-07.** The original wording
required reproducing hyprlock exactly, palette included. That is the wrong
target: `hyprlock.conf` is hard-pinned to Catppuccin while the desktop takes a
wallpaper-derived palette from matugen, which makes the lock screen the one
surface on this machine that does not follow the others — the same fault as the
SDDM greeter this project was started over. Reproducing it would have shipped
the bug. The geometry and typography were always the valuable half of R11 and
are kept, with the corner radius and rounding power now named explicitly. See
[themes/nightshade.md](../themes/nightshade.md) for how a theme supplies colour.

R12's priority was left open here pending Q1 and is **settled**: Q1 chose video
loops, so time-of-day variants stay a Should. [nightshade](../themes/nightshade.md)
treats them as part of what a theme *is*, which is consistent with that — a
theme may require of itself more than the greeter requires of every theme.

### Robustness

| # | Priority | Requirement | Fit criterion |
|---|---|---|---|
| R13 | MUST | A config that fails to parse must not prevent login. The error is reported with file and line, and the greeter starts with defaults. | A deliberately broken config still yields a usable login, and the error names the line. |
| R22 | MUST | Theme code must not be able to prevent login. A theme that errors, loops forever, allocates without bound, or tries to touch the filesystem, network or a process is stopped and the greeter continues on defaults, with the reason logged. | Four themes, each misbehaving in one of those ways, each yield a usable login. A theme calling `io.open`, `os.execute` or `require` fails at the call rather than succeeding. A theme looping forever is terminated by the instruction ceiling and does not delay the first prompt. |
| R14 | MUST | In lock mode, fail closed. | `SIGKILL` while locked leaves the session locked, per C5. Verified deliberately, with a second TTY open. |
| R15 | MUST | No media path reads from a user's home directory (C10). | With `/home/<user>` at mode 700, the configured background still renders. |
| R16 | SHOULD | Appear within a budget of first frame after launch. | Measured from process start to first presented frame. **Budget, set from P0:** 500 ms with a warm GStreamer registry. P0 measured 243 ms through `wl_shm` at 1255×674, so the budget allows for a full-resolution surface and a real renderer. The registry must be cached (C15) or roughly 0.9 s is added before any of this starts; an uncached greeter cannot meet this budget and the failure is in deployment, not in code. **Still provisional**: 243 ms was measured through a *nested* `cage` inside a running Hyprland session, which is both the wrong surface size and the wrong machine load. `spike/p1e/cage-measure.nu` is the harness that settles it — ten warm runs at 2560×1440 on a real VT — and it cannot be run until the Wayland shell exists. |
| R23 | SHOULD | Play a short ambient sound while the login screen is up, supplied and configured by the theme, and silent by default. | With a theme that names a sound, it plays within a second of the first frame and stops when the session starts. With a theme that names none, nothing is opened and no device is held. The greeter still logs in when the sound device is absent, busy or unreadable, and says so in the log rather than failing. **Gated on C16:** the greeter currently cannot open a sound device at all, so this requirement is unbuildable until Q10 is answered. |

### WON'T (v1)

X11 rendering; autologin; user creation or password changes; remote login;
fingerprint or smartcard; per-user backgrounds; a theme repository; replacing
`awww` on the desktop.

## Success metrics

Distinct from the budgets above: those say the code is fast enough, these say
the project worked. Deliberately none of them counts users or contributors —
tenet 4 says an absent audience is information, not failure, so a metric that
measured it would be measuring the wrong thing.

The first one is the suite's own proof, restated for this component: *"dat ik
echt het gevoel heb dat ik gewoon in één consistent logisch plaatje zit."*

- Logging in and unlocking feel like the same machine as the desktop — no
  surface that announces itself as belonging to a different program.

- SDDM is uninstalled from the author's own workstation and not reinstated
  within a month.
- hyprlock is removed from the autostart path, with `greet`'s lock mode used
  daily for two weeks and no lockout incident.
- A cloud identity login completes at least once, end to end, with the device
  code read off the login screen — proving C11's premise rather than assuming
  it.

## Phasing

Each phase is independently useful and answers the question the next one needs.

- **P0 — spike (days). Done, 2026-11-07.** A Rust client that talks greetd
  under `cage` and decodes video in greetd's context. C6's open half is
  answered: hardware decode works as the greeter's own user with no session,
  at 1440p60, for 2.5% of one core, with 9.2× headroom. **P2 is cheap.** The
  spike also corrected C6's first half (the path is NVDEC, not VA-API), found
  C15 (the greeter cannot cache, costing ~0.9 s per login), set the budgets in
  R7 and R16, and settled Q2. Code in `crates/`, receipts in `spike/p0/`.
- **P1 — daily driver (weeks).** Still background, hyprlock's look, multi-output
  surfaces, session list, last-user memory. At the end of P1, SDDM comes off the
  machine. Everything MUST except R7, R8 and R14.
- **P2 — animation.** Video background, loop seam, decode fallback. R7–R9.
- **P3 — lock mode.** ext-session-lock surface, shared renderer and config, the
  fail-closed test with a second TTY open. R14. hyprlock stays installed as the
  fallback throughout.
- **P4 — other people.** AUR package, README that stands alone, config
  reference, and a second machine with two monitors to prove R10.
- **P5 — cloud identity.** Gated on Q7, and deliberately last: R17, R18 and R20
  are cheap and can land earlier, but the PAM side is outside this project's
  control, so no earlier phase may depend on it. The avatar hook (R21) comes
  after, since it needs a working cloud login to have a picture at all.

## A note on secrets, for later

Nothing in this repository should ever hold an OAuth client id, client secret,
issuer configuration or device registration. Those belong in the PAM layer's own
configuration on the machine being set up (C11), outside version control, and
the device-code client type Google requires for this flow is still not something
to publish. The README may describe how to create one; it must not contain one.

## Open questions

| # | Question | What would settle it | Owner |
|---|---|---|---|
| Q1 | ~~Video loops, or Apple-style time-of-day stills, or both?~~ **Answered: video loops.** | Decided by the maintainer, 2026-11-07. Video loops are the subject; time-of-day variants stay a Should (R12) rather than becoming a Must. This is tenet 2 applied rather than overridden — motion you can see comes first, and the clock is how it stays interesting. P0 having shown decode is cheap (C6) means nothing forces the fallback to stills. | Settled |
| Q2 | ~~Is `greetd_ipc` usable, and under what licence?~~ **Answered: GPL-3.0-only, and not used.** | `greetd_ipc` 0.10.3 declares `license = "GPL-3.0-only"` in its own `Cargo.toml.orig` and bundles the GPLv3 text; docs.rs simply does not render the field, which is why the secondary source looked unreliable. The codec is therefore implemented in-house in `crates/greetd-proto` — the whole protocol, with tests, well under 200 lines — so no GPL code sits in the authentication path and Q3 is free. | Settled |
| Q3 | ~~Which licence for `greet`?~~ **Answered: MIT OR Apache-2.0.** | Q2 removed the only constraint, and an audit of all 113 crates in the dependency graph found no copyleft whatsoever, so the choice was entirely free. Permissive follows tenet 4: copyleft's worth is enforcement, which this project has said it will not do. Apache-2.0 accompanies MIT for its patent grant, relevant in a project built on hardware video decode — though that grant covers contributions here and not codec patents, which are unrelated. | Settled |
| Q4 | ~~Which configuration format at all?~~ **Answered: Lua.** | Decided by the maintainer, 2026-11-07. Consistency pointed at Lua on the evidence: Hyprland 0.57.1 ships a Lua config subsystem, the maintainer's own `hyprland.lua` is live with `hyprland.conf` kept only as a rollback artefact, and matugen already emits the palette as a Lua table that a Lua config can consume with no parser in between. It also lets a theme express *how* backgrounds switch, which the thesis requires and no declarative format provides. The cost is that the scripting non-goal had to be rewritten rather than bent, and a new R22 exists because a theme that runs is a theme that can hang. C3 is moot; the binding is `mlua` (MIT). **Recorded as Lua in a restricted interpreter** — see the non-goal for exactly what is denied and what is therefore accepted. | Settled |
| Q5 | Which Wayland stack: smithay-client-toolkit with wgpu, or a toolkit? | **Settled 2026-11-07: `smithay-client-toolkit` for surfaces and input, GL sharing GStreamer's context for pixels. Not wgpu.** Decision at [`docs/decisions/q5-renderer.md`](../decisions/q5-renderer.md), measurements at [`spike/q5/RESULTS.md`](../../spike/q5/RESULTS.md). A minimal GL renderer was built rather than the choice being argued: it negotiates `memory:GLMemory` with `nvh264dec` for real and costs **33.7% of one core** at 1440p60 against **68.8%** for the `wl_shm` readback, both at a held 60 fps, measured under `cage` on a free VT. wgpu is ruled out because it has no safe way to import an external texture, which R7's no-readback budget makes mandatory. `memory:DMABuf` on Intel and AMD stays unverified for want of the hardware, and the GL path's run-to-run spread (26–42%, against the readback path's 2 points) is unexplained. | Settled |
| Q6 | Does the two-mode design hold, or does lock mode become its own binary? | P3. If the renderer shares cleanly through P1 and P2, one binary; if the lock path needs materially different plumbing, split. | Maintainer |

| Q7 | Does authd's PAM module function under greetd, or only under GDM (C12)? | A spike: greetd plus authd on one machine, with a device-code client, driven by `agreety` first so the greeter is not a variable. This is the question that decides whether cloud login is weeks or months. | Maintainer |
| Q8 | Is a snap dependency acceptable on an Arch-based distribution for the broker, or does this need a native PAM module (C12)? | Follows Q7. If snapd is unacceptable, the `pam_oauth2_device` forks need evaluating, or a broker needs writing — a separate project, not part of `greet`. | Maintainer |
| Q9 | Who writes the avatar file, and is the `picture` claim even available to that component? | Reading what authd stores after a successful login. If it keeps nothing, R21's hook has to be driven by a PAM session module. | Maintainer |
| Q10 | How does the greeter reach a sound device, and is it worth what it costs (C16, R23)? | Two routes measured against each other. Adding `greeter` to the `audio` group is a one-line deployment change that works with no session — and grants a pre-authentication process permanent access to the **capture** devices, not only playback, which is a real widening of what a compromised greeter could do. Giving the greeter a logind session gets logind's own ACL instead, which is scoped to the active session, but changes how greetd is deployed and has not been tried. A third answer is "not worth it", and that is a legitimate outcome: nothing else in the project needs audio. | Maintainer |

## What would change this plan

- ~~**Decode does not work in greetd's context**~~ — **retired by P0.** It
  works, measured (C6). Had it not, the animated background would have become a
  lock-screen feature only and "patch hyprlock" the better project. The
  residual version of this risk is narrower and now partly measured. VA-API
  does decode in hardware here, at rate, once past GStreamer's vendor filter —
  but it copies frames to system memory, so it fails R7's budget on this
  machine. On Intel and AMD the same plugin exports DMABuf instead, which is
  why the portable answer is to name no decoder and accept several memory
  types (C6). What remains untested is real Intel and AMD hardware, which is a
  P4 question, not a premise question.
- **hyprlock gains video support upstream** before P3. Then lock mode is not
  worth writing; `greet` stays a greeter and contributes a config bridge
  instead.
- **A deployment adds a second authentication factor.** R1 stops being a design
  nicety and becomes the feature that decides whether this is usable at all.
- **The maintainer's own setup gains a second monitor.** R10 moves from a
  correctness claim verified on someone else's hardware to something exercised
  every day — which
  is where ReGreet's single-monitor limitation came from in the first place.
- **authd turns out not to work under greetd** (Q7). Then cloud login is not a
  rendering problem but a missing PAM module, which is a second project of its
  own size. `greet` would ship with R17, R18 and R20 ready and unused, and
  the cloud identity goal would move out of v1 entirely.
- **greetd is deprecated or its IPC changes incompatibly.** The whole
  authentication path is built on a protocol whose own man page describes the
  project as small and stable; that is an assumption, not a guarantee.
