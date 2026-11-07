# nightshade — auroval's first theme

Status: **draft for review** · Org: `auroval` · 2026-11-07

A theme is not a colour scheme. Per the [thesis](../vision.md#the-thesis), it
determines *"de set van achtergronden, hoe die switchen, wat er in de bar
staat, gewoon het hele pakket."* This document answers those, concretely, for
one theme.

Written from a design conversation with the maintainer. Where the words are the
content rather than a summary of it, they are his, quoted verbatim and
unedited. The [vision](../vision.md) and [tenets](../tenets.md) are upstream of
this document and win where the two disagree.

---

## 1. What nightshade looks like

**One subject, centred, on darkness, rim-lit in neon.**

The starting evidence was the maintainer's own wallpaper folder: twelve files,
all synthwave/outrun — neon magenta, chrome, dark skies, sunset horizons. Asked
whether that was the project's identity or just what he had grabbed, the answer
was to lean into it. Asked which ones he actually liked:

> "I especially like the skull and kimono lady."

Those two are the only two in the set that are not landscapes. Both are a
single centred subject against a dark field with a neon rim: the skull
pink-on-near-black under heavy grain, the kimono portrait with a cyan disc
behind the head. The other ten put a bright sunset horizon straight through the
middle of the frame.

That distinction is doing more work than it looks. **The two he picked are also
the two that leave room for a login screen.** A clock and an input field land in
the centre of the frame; a sunset lands there too. Subject-on-darkness is the
composition that lets the UI sit in the picture rather than on top of it, with
no scrim, no dimming layer, and no fight for contrast.

So nightshade's rule, chosen deliberately and not as a style preference:

- One subject, roughly centred, occupying the upper-middle of the frame.
- The field around and below it is dark, and stays dark.
- Light is rim light and glow, sourced from behind or beside the subject.
- Neon chroma: magenta, cyan, violet. Saturated, never pastel.
- Grain is permitted and welcome. Gradient banding on a near-black field is
  the failure mode grain exists to hide.

**Cost, stated plainly.** This rules out the horizon-and-open-road image that
the genre is actually famous for, which is ten of his twelve current files. The
theme is narrower than the mood that inspired it.

### 1.1 The media is generated, not found

> "I want to generate our own images."

> "Same for the video, we generate."

nightshade ships its own artwork. Nothing in it is a scraped wallhaven file.
The generation happens **once**, and the result is committed:

> "Generated once, commited as files but packaged individually so we don't pull
> in every theme all the time."

Two consequences, both load-bearing:

1. **A theme is a package.** Installing auroval does not install every theme's
   media. Each theme is its own artefact — `auroval-theme-nightshade` — carrying
   its images, its loops, and its derived palette (§4). The core carries none.
2. **Media lives outside any home directory.** This is forced anyway by
   [C10](../prd/greet.md): the greeter runs as its own unprivileged user and
   cannot traverse a home at mode `0700`. A theme package installs to a
   system path, which satisfies C10 and R15 without a copy-out step.

The prompts and settings that produced each image are recorded alongside it, so
the set can be extended later in the same voice rather than re-guessed.

---

## 2. The media contract

Tenet 6 says the user's own file wins: *"mensen moeten zelf ook eigen video's in
kunnen gebruiken."* So the theme is a **contract** at least as much as it is a
set of files.

**The contract describes what nightshade's own media meets. It is not a gate on
yours.** Nothing is refused. Every clause below names what happens when a file
does not meet it, because tenet 6's own cost clause already committed the
project to that shape: arbitrary files mean arbitrary formats, resolutions,
aspect ratios and colour spaces, *"all of which must degrade gracefully rather
than fail."*

Values below are proposals to be tuned during the build, not measurements. The
[tracking issue](https://github.com/auroval/auroval/issues/5) carries them as checklist items.

### 2.1 Format

| Property | Requirement | Why |
|---|---|---|
| Video codec | H.264 High, or HEVC Main / Main10 | The profiles [C6](../prd/greet.md) verified at `VAEntrypointVLD` on the target. Anything else is a software-decode trap. |
| Still format | PNG or JPEG | Matches what the desktop path already handles. |
| Colour primaries | BT.709, range declared (full or limited) | An undeclared range is the usual cause of a washed-out or crushed render, and it would poison the derived palette (§4). |
| Bit depth | 8-bit minimum, 10-bit preferred for video | A near-black field at 8-bit bands visibly. 10-bit or grain; ideally both. |

### 2.2 Geometry

| Property | Requirement | Why |
|---|---|---|
| Resolution | Shorter side ≥ 1440 px | Below that, the renderer is upscaling on the maintainer's own output. |
| Aspect | 16:9 source | The renderer **covers** (centre-crops) to the output, never letterboxes — R10 says neither letterboxed nor cropped-wrong, and a centre-crop of a centred subject survives both ultrawide and portrait. |
| Safe field | The central region — 60% of width by 70% of height — must be dark | This is where the clock, the date, the user label and the input field land. It is the contract clause that makes "no scrim" possible. |

**Safe-field test:** within that region, mean relative luminance ≤ 0.06, and no
more than 2% of pixels above 0.25. nightshade's own artwork meets this by
construction. Media that does not gets a local scrim (§2.4), not a rejection.

### 2.3 Loop

| Property | Requirement | Why |
|---|---|---|
| Duration | 10–30 s | Long enough not to read as a repeat, short enough to hold in memory and to seek cheaply. |
| Seam | First and last frame differ no more than two ordinary consecutive frames | R8's fit criterion, measured at packaging so a visible cut is reported rather than discovered at 3 a.m. |
| Motion | Slow, continuous, no cuts | §3 explains why this is a hard limit and not taste. |

### 2.4 What happens when media does not meet it

Nothing is refused. Each clause degrades, and the degradation is reported at
import — information, not a veto.

| Clause fails | What happens |
|---|---|
| Codec unsupported | Runtime fallback per R9: still image, then solid colour. Logged, not silent. This path already exists and is release-blocking. |
| Resolution below 1440 | Upscaled. It looks soft. That is the user's file being soft. |
| Aspect not 16:9 | Centre-crop to cover, as for any source. A centred subject survives it. |
| Range undeclared | Assume limited, the safer guess, and say so in the import report. |
| Loop seam visible | Played as-is. A visible cut once every 20 seconds is a flaw in their file, not a reason to refuse it. |
| **Safe field too bright** | A local scrim is composited behind the UI — a soft radial gradient over the central region, only as strong as needed to reach the text contrast floor. |

The scrim is the one real piece of compensation, and it is worth being precise
about. It is **not** applied to nightshade's own artwork, which meets the clause
by construction, so the maintainer never sees one. It is applied only to media
that would otherwise put bright pixels under the clock and the input field, and
it is local to that region rather than a full-frame dim.

**Cost, stated plainly.** A user's bright video is shown with a dark patch
behind the UI that nightshade's own images do not have, so the theme looks
slightly different in kind on their machine than on the maintainer's. "One
consistent theme" becomes "one consistent theme, plus compensation where the
media forces it." That is a genuine dilution of tenet 1, accepted here because
the alternative — turning the file away — is a larger dilution of tenet 6, and
tenet 6 is the one that named this exact situation in advance.

**Where checking happens.** Measurement is done at import, never at login. A
greeter that evaluates a background while you are trying to log in has put work
on the critical path of the one goal the PRD calls non-negotiable:
*"Authentication is never the thing that breaks."* Import computes the scrim and
the palette once; the greeter reads the result.

## 3. How the backgrounds switch

Two axes, both chosen, plus one hard limit that is the most important sentence
in this section.

### 3.1 The model

A **scene** is one subject — one skull, one portrait — rendered as a set of
**phase variants**: dawn, day, dusk, night. A theme holds several scenes.

- **Daily rotation** picks one scene per day, as `awww-wallpaper.sh` does today
  at `INTERVAL=86400`.
- **Time of day** moves through that scene's phase variants as the day passes.

So the subject changes once a day; the light on it changes through the day. The
vision asked for exactly this and ranked it second to visible motion —
*"meebeweegt met de tijd van de dag op zijn minst, maar idealiter zelfs ook de
seizoenen."* Seasons are not in nightshade. They are a theme-level extension
the model already supports — a season axis is another variant dimension — and
the first theme does not spend the generation effort on four times the artwork.

### 3.2 The hard limit: the desktop does not move

> "however the desktop background should not animate noticably and be
> distracting. Subtle is ok."

This is the single most consequential thing said in the conversation, because
it splits motion by surface:

| Surface | Background |
|---|---|
| Login (greet) | The scene's **moving loop**. This is where the wow lives. |
| Lock | The scene's **moving loop**, identical to login. |
| Desktop | The scene's **still** for the current phase. At most an ambient drift — slow enough that you cannot tell it is moving without staring. |

**This refines tenet 2 and should be read against it.** Tenet 2 says *"motion
you can see over motion you infer"*, and its stated cost was that the whole
premise rested on an unverified assumption: that hardware decode works in the
greeter's context. **That risk is retired.** P0 measured it as greetd's own user
(uid 950, `runuser` with `env -i`, no `XDG_RUNTIME_DIR`, no D-Bus, `HOME=/`):
59.50 fps at 1440p60, 0 late buffers, 2.5% of one core, frames in
`memory:CUDAMemory`. Tenet 2's *"expires if"* condition was tested and did not
fire.

So the split here is **not** risk mitigation, and should not be read as a
cheaper fallback for a decode failure that can no longer be assumed. It is a
taste decision, and it rests on tenet 3 alone — which is a stronger footing than
the one an earlier draft of this section gave it. A moving background on a login
screen is seen for four seconds a day; a moving background on a desktop is in
your peripheral vision for eight hours, which is exactly what *"gaat
irriteren"* by week two.

**This does not amend tenet 2;** whether it wants a per-surface clause is
flagged as N5.

Tenet 2 and tenet 3 therefore do not conflict here. They apply to different
surfaces, and each gets what it asks for.

### 3.3 Where a declarative format runs out

This section exists for [Q4](../prd/greet.md#open-questions) and takes no
position on it.

**Expressible as data** — a table, in any format with no execution:

- The scene list, and each scene's phase variants and still/loop pair.
- The phase boundaries, if they are fixed clock times.
- The derived palette per scene (§4), precomputed.
- Every clause of the media contract (§2) as thresholds.

**Not expressible as data** — this is behaviour:

- *Which scene today.* Random, sequential, seeded by date? "Pick one per day"
  is a rule, and the rule has to live somewhere.
- *When a phase begins*, if boundaries follow the sun rather than the clock.
  Solar dawn needs latitude, longitude and a date — a computation, not a
  constant. Fixed clock times would be data; they would also be wrong in
  December.
- *The transition.* Crossfade on phase change, on wake from suspend, on unlock
  — duration, curve, and whether it happens at all when the screen was off.

This is the same observation [docs/ideas.md](../ideas.md) makes about executable
themes: *"The thesis already says a theme determines 'hoe die switchen'. That is
behaviour, not data, and no config format expresses it."* nightshade is the
first concrete instance of it. **It does not argue for WebAssembly, for Lua, or
for anything else.** It says: whatever Q4 decides, it has to have an answer for
those three bullets, and "put it in the config file" is not one. The cheapest
answer — the renderer implements these three rules natively and the theme only
selects between named strategies — is also a legitimate answer and should be
priced before anything executable is.

---

## 4. The palette

### 4.1 What was decided

Today the machine is inconsistent in exactly the way the vision complains
about, and nobody had noticed: `lua/base.lua` takes its palette from matugen,
derived from the current wallpaper, while `hyprlock.conf` is hard-pinned to
Catppuccin Mocha. **The lock screen is literally the one surface that does not
follow the desktop** — the same genus of fault as the SDDM screen that started
the project.

nightshade's answer: **derive everywhere, from the media.** Greeter, lock and
desktop all take their palette from the scene on screen.

### 4.2 What "derived" actually means — measured, not assumed

An earlier draft of this document specified contrast gates that a user's media
had to pass, on the assumption that bad media yields an unreadable palette.
**That assumption is false, and the measurement is worth recording because it
dissolves the problem rather than solving it.**

matugen run on four sources — a flat mid-grey field, a washed-out low-contrast
gradient, a near-white field, and the maintainer's own skull wallpaper:

| source | `surface` rel. luminance | `on_surface`/`surface` | `primary`/`surface` | `primary` L\* / C\* |
|---|---|---|---|---|
| flat grey `#808080` | 0.0065 | 14.41:1 | 10.88:1 | 79.9 / 31.1 |
| washed gradient | 0.0066 | 14.37:1 | 10.92:1 | ~80 / ~31 |
| near-white | 0.0065 | 14.41:1 | 10.88:1 | 79.9 / 31.1 |
| skull wallpaper | 0.0066 | 14.39:1 | 10.91:1 | 80.0 / 33.0 |

Material You fixes tones. `surface` is always near-black, `on_surface` always
lands at ~14.4:1, `primary` always at ~10.9:1 — whatever you feed it. Repeated
across `scheme-tonal-spot`, `-vibrant`, `-content`, `-fidelity` and
`-expressive`, `primary` stays pinned at L\* ≈ 80 and C\* ≈ 31–33 in every one.

**The only thing the media actually contributes is hue.** 354.8° for the skull,
278.4° for flat grey. Everything else is a constant of the scheme.

Two conclusions follow, and they point in opposite directions:

1. **There is no such thing as an ugly derived palette.** A readable, correctly
   contrasted dark scheme is guaranteed by construction. Refusing media to
   protect the palette was defending against a failure that cannot occur.
2. **nightshade cannot get its own look this way.** §1 says *"Saturated, never
   pastel."* C\* ≈ 33 at L\* ≈ 80 is pastel — that is what `f8b1dd` in the live
   `colors.lua` is. Neon magenta is roughly L\* 55–60 at C\* 70+. Taking matugen
   as-is guarantees the accent is the one thing the theme says it is not.

### 4.3 The derivation: hue from the media, tone and chroma from the theme

> **The media supplies hue. nightshade supplies everything else.**

This is tenet 1 doing exactly what it exists to do: the theme owns the look, and
the media feeds it one parameter. It is also tenet 6 honoured in full, because
every file has a hue — there is nothing to refuse, nothing to correct, and no
file that can fail.

| Role | Source | Target |
|---|---|---|
| Accent hue | Extracted from the scene's canonical frame | whatever the media gives |
| Accent lightness / chroma | **nightshade** | L\* ≈ 55–60, C\* ≥ 70 — neon, not pastel |
| Surface | **nightshade** | near-black, L\* ≤ 20 |
| On-surface | **nightshade** | ≥ 7:1 against surface |
| Error | **nightshade** | fixed; a failure message must not change colour with the wallpaper |

Exact tonal targets are the maintainer's to pick — they are the theme's visual
signature and the one number that decides whether nightshade reads as neon. They
are carried as an open item, not settled here.

The contrast figures from §4.2 are kept as **assertions**, not gates: cheap
regression checks that matugen still behaves as measured, run at packaging time.
They test the tool, not the user.

**Cost, stated plainly.** "Derived from the media" now means less than it sounds
like. Two different videos with similar hue produce identical palettes, and the
coherence is more authored than discovered. The honest framing: matugen already
worked this way and nobody had noticed — the live `colors.lua` is pastel because
the scheme makes it pastel, not because the wallpaper is. This states it instead
of leaving it to be discovered.

### 4.4 Derive once per scene, at packaging time

The greeter shows the loop; the desktop shows the still. If each surface derived
its own palette from its own file, the three surfaces would agree approximately
and not exactly — and "approximately the same accent colour" is visibly worse
than two obviously different ones.

So: **the palette is derived once per scene, from one canonical frame, when the
theme is packaged. It ships with the scene.** This buys four things at once:

- All three surfaces are byte-identical on colour, because they read the same
  table.
- The greeter does no colour analysis at login, which is work it should not be
  doing anyway (R16's budget).
- The greeter reads a world-readable theme package rather than analysing a
  file it may not be allowed to open (C10).
- The scrim measurement (§2.4) happens at the same moment, on the same frame.

Output shape stays as it is today: bare 6-digit hex with no leading `#`, so
callers append their own alpha byte — *"that is the shape Hyprland's `rgba()`
wants"*, per the existing template's own comment.

**Consequence to state plainly:** live re-derivation on wallpaper change goes
away. Today `awww-wallpaper.sh` runs `matugen image "$wp" --prefer saturation`
and nudges five consumers. Under nightshade the palette is a property of the
scene, already computed. matugen moves from runtime dependency to packaging
tool. That is a change to something the maintainer currently has and uses, and
it should be a conscious trade rather than a side effect.

### 4.5 Tenet check

Tenet 1 refuses per-element overrides, and that refusal is only honest if the
whole is reliably good. §4.3 is what makes it honest: the theme owns tone and
chroma, so the whole is good by construction rather than by luck, and a user
never has a reason to want one colour changed because no file can produce a bad
one.

Tenet 6 is honoured without exception on the palette. Nothing is refused, and
nothing needs correcting, because the media was only ever supplying hue.

The remaining tension is §2.4's scrim, which is composition rather than colour,
and it is the one place where a user's file changes how nightshade looks rather
than just what colour it is. That cost is stated where it occurs.

## 5. The bar

Tenet 3 asks what earns a permanent place on screen for someone who will see it
every day for a year. The current waybar carries seventeen modules.

**nightshade's bar:**

| Position | Modules |
|---|---|
| Left | Workspaces |
| Right | Volume · Network · Notifications · Keep-awake · Power profile · Tray · Clock · Power |

**Dropped:** CPU, memory, temperature, headset battery, caps lock, num lock,
microphone source.

The principle, in the maintainer's own cut: things you *look at without deciding
to* (clock, workspaces), things where *seeing it makes you act* (volume,
network, notifications), *modes you are in* (keep-awake, power profile), and
*targets you click* (tray, power). Machine vitals went, and they are the
textbook tenet-3 casualty: glanceable, satisfying, and almost never acted on. A
number that changes constantly and means nothing is the definition of something
that grates by the fiftieth look.

Caps lock leaving the bar is **not** a conflict with [R6](../prd/greet.md),
which requires a caps-lock warning on the greeter. That warning is transient and
appears only when caps lock is on, on a surface where typing blind is a real
failure. A permanent indicator on the desktop is a different thing, and it went.

**Open, flagged rather than decided:** the centre `hyprland/window` title. It is
neither time-and-place nor state-you-act-on, so by the stated principle it goes
— but it was not asked about explicitly, and a window title is the kind of thing
whose absence is only noticed in use. Carried as a checklist item, not settled
here.

**Cost, stated plainly.** Eight modules gone means eight things that are now
either invisible or require a command. The first week will produce at least one
"where did the temperature go". Tenet 3 says that is the right trade and tenet 4
says the maintainer's own daily use decides it — but the regret is real and
should be allowed to reverse the call, once, with evidence from use.

---

## 6. How login, lock and desktop relate

This is the project's whole proof: *"dat ik echt het gevoel heb dat ik gewoon in
één consistent logisch plaatje zit."*

| | Login (greet) | Lock | Desktop |
|---|---|---|---|
| Background | Scene's moving loop | **Same moving loop** | Scene's still, current phase |
| Palette | Scene's derived palette | Same | Same |
| Font | MesloLGS Nerd Font | Same | Same |
| Geometry | Large clock, rounded input (r=12, squircle) | Same | Window rounding r=12, `rounding_power=4` |

**The lock screen stops showing your desktop.** Today `hyprlock.conf` uses
`path = screenshot` with `blur_passes = 3` and `brightness = 0.6`: you see what
you were doing, dimmed. Under nightshade, lock shows the theme's scene, the same
one login shows.

The reason is structural and not aesthetic. **Login can never screenshot a
desktop** — there is no session yet. So as long as lock shows a blurred desktop
and login shows artwork, the two surfaces can never match, which is the exact
split the project exists to close. One of them has to give, and it cannot be
login.

What is lost: the glance that tells you what you left open. What is gained,
incidentally: your screen contents are no longer rendered, however blurred, on
a locked machine in a room you may not be alone in.

Geometry inherits from the live desktop rather than being invented: rounding 12
with `rounding_power = 4` (*"the corner curve bends toward a squircle rather
than a circular arc; it reads as softer at the same nominal radius"*), 2 px
borders, the accent-gradient focused border, the `11111baa` shadow. The input
field's `rounding = 12` already matches the window rounding — that was not an
accident and it is now a rule.

---

## 7. What this implies elsewhere

Three items that are **not** this document's to decide, recorded so they are not
discovered later.

### R11 has been rewritten

[R11](../prd/greet.md#appearance) used to read:

> Reproduce hyprlock's visual language: **Catppuccin Mocha**, MesloLGS Nerd
> Font, the large clock, and the rounded input field.

§4.1 contradicts the first item directly, and keeping it would have shipped the
very fault the project was started over — `hyprlock.conf` is the one surface on
this machine that does not follow the others. The pin is gone; the geometry and
typography are kept, with the corner radius and rounding power now named, and
colour comes from the active theme. The PRD carries a dated note saying why.

R12's priority is **still open**. §3.1 makes time-of-day variants part of what a
theme *is*, which argues for MUST, but that follows Q1 and is not settled here.

### Q1 is effectively answered — confirm it

[Q1](../prd/greet.md#open-questions) asks: video loops, or Apple-style
time-of-day stills, or both? nightshade uses **both, with different jobs**:
loops on login and lock, time-of-day stills on the desktop (§3.2). That was not
a design preference imposed here — it follows from the maintainer's own line
about the desktop not animating noticeably, combined with his choice of
time-of-day switching.

It is recorded as a consequence, **not** closed. Q1 is the maintainer's to
settle in the PRD, and doing so changes R12's priority.

### Q4 gets a requirement, not an answer

§3.3 lists the three things a theme has to express that are behaviour rather
than data: which scene today, when a phase begins, and how the transition
happens. [Q4](../prd/greet.md#open-questions) and
[issue 3](https://github.com/auroval/auroval/issues/3) decide the format;
nightshade only supplies the test it has to pass. The parked
[WebAssembly idea](../ideas.md) is not pulled forward, and the cheapest option —
the renderer owns these three rules and the theme names a strategy — is
explicitly on the table.

---

## 8. Open

| # | Item | Owner |
|---|---|---|
| N1 | nightshade's tonal targets (§4.3) — the accent's L\* and C\*, which decide whether it reads as neon or as pastel. The theme's visual signature, and the one number worth getting wrong a few times first. | Maintainer, against the first scene |
| N2 | Does the centre window title stay in the bar (§5)? | Maintainer |
| N3 | How many scenes does nightshade ship? Daily rotation over two scenes is a two-day cycle, which is not rotation. | Maintainer |
| N4 | Fixed clock times or solar phase boundaries (§3.3)? Materially changes what the format must express. | Maintainer, with Q4 |
| N5 | Should tenet 2 gain a per-surface clause (§3.2)? | Maintainer |
| N7 | The scrim (§2.4) is the one place a user's file changes how nightshade looks rather than what colour it is. Acceptable, or does nightshade prefer to show bright media undimmed and let the text fight it? | Maintainer |
| N6 | The name collides with Nightshade, the University of Chicago image-poisoning tool. Different domain, and theme names are namespaced under auroval — noted, not raised as a blocker. | Maintainer |
