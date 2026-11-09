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
| Video codec | H.264 High, or HEVC Main / Main10 | The formats [C6](../prd/greet.md) measured decoding in hardware in the greeter's own context — 59.50 and 59.70 fps at 1440p60, against a 36–54% software baseline. The theme names **formats, never decoders**: `decodebin3` rank-selects, which is what keeps the same media working on AMD and Intel. |
| Still format | PNG or JPEG | Matches what the desktop path already handles. |
| Colour primaries | BT.709, range declared (full or limited) | An undeclared range is the usual cause of a washed-out or crushed render, and it would poison the derived palette (§4). |
| Bit depth | 8-bit minimum, 10-bit preferred for video | A near-black field at 8-bit bands visibly. 10-bit or grain; ideally both. |

### 2.2 Geometry

| Property | Requirement | Why |
|---|---|---|
| Resolution | Shorter side ≥ 1440 px | Below that, the renderer is upscaling on the maintainer's own output. |
| Aspect | 16:9 source | The renderer **covers** (centre-crops) to the output, never letterboxes — R10 says neither letterboxed nor cropped-wrong, and a centre-crop of a centred subject survives both ultrawide and portrait. |
| Composition | The central region — 60% of width by 70% of height — is dark | A look clause: this is what makes nightshade a subject-on-darkness theme rather than a landscape one. It earns no compensation on its own. |
| Legibility | Behind **each UI element separately** — clock, date, username, input field — the background clears the 4.5:1 contrast floor | Measured per box at the 90th percentile, not as an average over the whole field. A mean hides a small bright thing: a measured image averaged 0.0905 while the disc behind its date line sat at 0.9936. |

**Composition test:** within that region, mean relative luminance ≤ 0.06, and no
more than 2% of pixels above 0.25.

**Legibility test:** for each UI element, the background at the 90th percentile
of its own box leaves the text above 4.5:1. This is the one that earns a scrim;
composition does not, because dimming a picture for failing a taste rule would
be absurd. nightshade's own artwork meets both by construction.

Both are measured by `tools/scene-check/scene_check.py`; see
[nightshade-assets.md](nightshade-assets.md) for how candidates are judged.

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
| Codec unsupported, or no hardware decoder for it | Runtime fallback per R9: still image, then solid colour. Logged, not silent. Release-blocking, and the only honest answer on hardware where the theme's own loops cannot be decoded — C6's remaining unverified case is a VA-API-only machine, which no one has tested. |
| Resolution below 1440 | Upscaled. It looks soft. That is the user's file being soft. |
| Aspect not 16:9 | Centre-crop to cover, as for any source. A centred subject survives it. |
| Range undeclared | Assume limited, the safer guess, and say so in the import report. |
| Loop seam visible | Played as-is. A visible cut once every 20 seconds is a flaw in their file, not a reason to refuse it. |
| **Legibility missed on an element** | A local scrim is composited behind the UI — a soft gradient over the central region, only as strong as the worst element needs to reach the contrast floor. |
| **Composition missed** | Nothing. It is reported and the media is used as it is. A legible picture that is not subject-on-darkness still works; it just is not nightshade. |

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

### 3.3 The three rules that have to run

This section was written while [Q4](../prd/greet.md#open-questions) was open and
deliberately took no position. **Q4 is now answered: Lua, in a restricted
interpreter** — created without `io`, `os`, `package`, `require`, `debug`,
`load` or `dofile`, under instruction and memory ceilings. A theme can express
behaviour and has no capabilities to abuse.

So the question is no longer *whether* these three rules can live in a theme.
It is what they are allowed to do.

**Data — a table, no execution needed:**

- The scene list, and each scene's phase variants and still/loop pair.
- The phase boundaries, if they are fixed clock times.
- The derived palette per scene (§4), precomputed and committed.
- Every clause of the media contract (§2) as thresholds.

**Behaviour — this is what Q4 bought:**

- *Which scene today.* Random, sequential, seeded by date? "Pick one per day"
  is a rule, and the rule now has somewhere to live.
- *When a phase begins*, if boundaries follow the sun rather than the clock.
  Solar dawn needs latitude, longitude and a date — a computation, not a
  constant. Fixed clock times would be data; they would also be wrong in
  December. **N4** chooses between them, and it is now a free choice rather
  than one constrained by the format.
- *The transition.* Crossfade on phase change, on wake from suspend, on unlock
  — duration, curve, and whether it happens at all when the screen was off.

**These three run inside a greeter, so R22 is binding.** Theme code must not be
able to prevent login. R22 names four failure modes — erroring, looping forever,
allocating without bound, and reaching for the filesystem, the network or a
process. The last is denied by construction, since the interpreter never
receives those capabilities. The other three all end the same way: no answer
comes back. So every rule nightshade specifies needs a defined answer for that
case, and here they are:

| Rule | If the theme's code does not produce an answer |
|---|---|
| Which scene today | The first scene in the list. Always exists; never empty. |
| When a phase begins | Fixed clock boundaries, from the theme's own data. |
| The transition | No transition. The new scene is simply there. |

None of those is a degraded login. They are a slightly less clever one, which
is the correct trade when the alternative is a locked-out machine.

**What nightshade does not need.** The cheapest option — the renderer owns the
three rules natively and the theme selects between named strategies — remains
sufficient for nightshade as specified, and is worth preferring where it fits.
Q4 made behaviour possible; it did not make it compulsory. The parked
[WebAssembly idea](../ideas.md) stays parked, with its own condition to watch.

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
| Accent hue | Extracted from the scene's canonical frame, or declared | whatever the media gives |
| Accent lightness / chroma | **nightshade** | the sRGB cusp — the most chromatic colour available within L\* 50–70 that still clears the contrast floor. **Not a fixed C\***: C\* 75 is comfortable for magenta and impossible for cyan, so one number would make some hues neon and others pastel. |
| Surface | **nightshade** | near-black, L\* ≤ 20 |
| On-surface | **nightshade** | ≥ 7:1 against surface |
| Error | **nightshade** | fixed; a failure message must not change colour with the wallpaper |

The lightness band is the maintainer's to tune (N1). What is settled is the
*form* of the answer: seek the cusp, never name a chroma. On a measured example
this turns matugen's pastel `#ffb0cd` (C\* 33) into `#ff1296` (C\* 85.4) — the
difference between the theme's stated look and its opposite.

#### 4.3.1 Measured against the scenes, and the answer is not a lightness band

N1 asked for the accent's L\* and C\*, *"the one number worth getting wrong a
few times first"*, to be chosen against a real scene. There are now three
scenes, and measuring against them says the question is aimed at the wrong
parameter.

`scene_check.py` already implements the form settled above: scan L\* from 50
to 70 in steps of 0.5, take the lightness whose cusp chroma is highest while
still clearing 4.5:1 against surface. Only the band is open. Run against six
real extracted hues:

| band | C\* range | ratio |
|---|---|---|
| 50–70 (as implemented) | 38.5 – 86.2 | **2.24×** |
| 55–60 (this section's stated starting point) | 34.0 – 80.4 | **2.36×** |
| 60–70 | 38.5 – 86.2 | **2.24×** |

The proposed starting point is worse than what is already running, on both
counts: a wider spread and less chroma throughout. And no band comes close to
consistency, because **the spread is not caused by lightness.** It is caused
by hue.

Across the hue circle, the achievable accent chroma runs from **38.6 at 210°**
to **110 at 330°**. Below C\* 50 — which is where "neon" stops being an honest
word — there is one continuous trough, **180° to 250°**: cyan and blue. No
choice of L\* lifts a hue out of it, because the limit is the sRGB gamut and
not a setting.

This section half knows it already. It says *"C\* 75 is comfortable for magenta
and impossible for cyan, so one number would make some hues neon and others
pastel"*, and uses that to justify seeking the cusp instead of naming a
chroma. But seeking the cusp does not remove the problem. It relocates it:
some hues are still neon and others still pastel, and the difference is that
no single number now shows it coming.

**What this means for the scenes that exist.** scene-01's hue is 211°, the
floor of the trough, so its accent cannot be neon — C\* 38.5 at best. The two
wallpapers in rotation extract 13.0° and 259.2°, which is not only a 246°
swing but C\* 79 against C\* 50: one is neon and the other cannot be. That is
the daily change on the desktop, and it is a gamut fact rather than a tuning
mistake.

**So the lightness band is settled by default and the real question moves.**
50–70 stays: of the three it gives the most chroma and the smallest spread,
and nothing argues for 55–60 except that this section proposed it before
anything had been measured. What decides nightshade's visual signature is
which hues its own scenes have, and §4.3 declines to constrain hue at all.

There is an exit that costs tenet 6 nothing, and §5.4 already opens it:
*"a scene may simply declare its hue"*. nightshade's own artwork could declare
hues outside the trough — roughly 280° through 80°, by way of magenta, red and
orange — while a user's media keeps getting whatever it gives. Nothing is
refused, nothing can fail, and the theme's own scenes become coherent by
construction rather than by luck. That is **N8**, and it is the question N1
was trying to ask.

**Extraction is fragile, which is a third reason to commit the palette.**
matugen's result depends on the file's bit depth: the same pixels at 16 and 8
bits gave hues 118° apart (pink against cyan). Per file it is deterministic, but
any re-encode changes the answer. So stills are never re-encoded before
extraction, a video's canonical frame comes from one pinned command, and a scene
may simply declare its hue. Detail in
[nightshade-assets.md §5.4](nightshade-assets.md).

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

### 4.6 Which role each element uses, and the floor it is held to

Settled 2026-11-07, after gate 1 demanded a **78% scrim** on scene-01 and the
artwork turned out not to be the cause.

`scene-check` was drawing the date line in `outline`, and **nothing in these
documents said otherwise** — the date was the one UI element whose colour had
never been specified anywhere, so a default buried in the measuring tool was
deciding it. `outline` is Material's role for strokes and dividers, and it is
the lowest-contrast token in the palette by design. Written out as a budget,
the brightest background each token can sit on and still clear 4.5:1:

| token | L\* | max background Y @ 4.5:1 | ceiling vs pure black |
|---|---:|---:|---:|
| `on_surface` | 91.9 | 0.1402 | 17.12:1 |
| `tertiary` | 74.1 | 0.0653 | 10.38:1 |
| `primary` | 70.1 | 0.0519 | 9.17:1 |
| `outline` | 57.9 | **0.0186** | **6.17:1** |

`outline` needs a background about **7.5× darker** than `on_surface` does, and
cannot reach 7:1 even against pure black. That is why the clock *passed* at
p90 0.1169 while the date *failed* at p90 0.0828 — a darker background. The
difference was entirely the text colour.

**The decision.**

| Element | Size | Role | Floor | Why that floor |
|---|---:|---|---:|---|
| Clock | 110 px | `on_surface` | 3:1 | WCAG large text |
| Date | 22 px | `on_surface` | 4.5:1 | normal text |
| Username | 16 px | `primary` | 4.5:1 | normal text |
| Input field | 20 px | `primary` | 4.5:1 | normal text |
| Error | — | `error` | 4.5:1 | fixed by the theme (R3) |

**Hierarchy comes from size and weight, never from lightness.** This is the
part worth remembering, because it is specific to a dark theme and it is
counter-intuitive: on a near-black field, every way of making text *quieter* —
a dimmer token, lower opacity — moves it **towards** the background and costs
contrast. A date rendered at 22 px in a dim colour is the worst case in the
whole layout. Five times the clock's size is already all the hierarchy the
layout needs.

**Why the brighter option rather than the looser one.** The date could instead
have been held to 3:1, which WCAG allows for text at 24 px or more, and then an
L\* of 66 would have been enough. That was rejected: tenet 6 promises the user
their own media, which means arbitrary and often bright backgrounds, and the
surface that has to absorb that is the text, not the picture. Making the text
robust costs nothing; dimming the user's artwork to rescue a dim colour costs
exactly what tenet 6 says not to spend.

The clock does move to 3:1, because at 110 px that is simply the correct floor.
It loosens the clock's clause, which is worth saying plainly rather than
quietly banking.

**Effect on scene-01.** Gate 1 goes from a 78% scrim to **no scrim at all**;
all four legibility clauses pass. Nothing about the artwork changed.

**Consequence for A4.** The scrim reject threshold must be calibrated knowing
that it measures the image *and* the palette's role assignment. With the date
in `outline`, almost any scene with a glow near the date line would have been
rejected regardless of how it looked, and a calibration run against that would
have encoded the wrong cause.

`scene_check.py` now reads both the role and the floor from one table, and that
table also sizes the measured regions — so the px that justifies a floor and
the px that is measured cannot drift apart.

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

R12 stays a **Should**, confirmed by Q1 (below).

### Q1 is answered: video loops

[Q1](../prd/greet.md#open-questions) asked: video loops, or Apple-style
time-of-day stills, or both? **The maintainer chose video loops**, and C9 is
rewritten from an open question into a decision: tvOS-style loops are the
subject, macOS-style time-of-day stills are secondary.

That is consistent with what this document already specified. nightshade uses
both, *with different jobs* — loops on login and lock, where the motion is the
point, and time-of-day stills on the desktop, which deliberately does not move
(§3.2). The PRD's framing says the same thing from the other side: time-of-day
variants are the clock making a moving background stay interesting, not a
substitute for motion, and a build shipping only stills would fail tenet 2.

So R12 stays a Should. nightshade requires time-of-day variants of *itself*
(§3.1), which is a theme asking more of itself than the greeter asks of every
theme — not a conflict.

### Q4 is answered, and nightshade is its first customer

Q4 chose **Lua in a restricted interpreter**, and
[issue 3](https://github.com/auroval/auroval/issues/3) is closed. §3.3 named
three things a theme has to express that are behaviour rather than data — which
scene today, when a phase begins, how the transition happens — as a test any
answer had to pass. Lua passes it.

Two consequences for this document, both now applied:

- §3.3 states, per rule, what happens when theme code fails to produce an
  answer, because **R22** requires that theme code cannot prevent login and
  nightshade is specifying code that runs in a greeter.
- N4 — fixed clock times or solar boundaries — is no longer constrained by what
  the format can express. It is now purely a question of what is wanted.

The palette needs no translation either: matugen already emits `colors.lua` as a
Lua table, so a Lua config consumes it directly. One generated palette, nothing
to keep in sync.

---

## 8. Open

| # | Item | Owner |
|---|---|---|
| N1 | **Reformulated 2026-11-08, measured rather than argued (§4.3.1).** Asked for the accent's L\* and C\* as the theme's visual signature. Measured against six real scene hues, the lightness band barely moves the result: 50–70 gives a 2.24× chroma spread, and the 55–60 this document proposed gives 2.36× with less chroma throughout. **Settled: the band stays 50–70.** The spread comes from hue, not lightness — the sRGB cusp runs from C\* 38.6 at 210° to 110 at 330° — so the signature is not controlled by this parameter and the real question is N8. | Settled |
| N8 | Do nightshade's **own** scenes hold a hue band, so the theme's coherence is design rather than luck? **The principle was settled in practice 2026-11-08**: offered a rotation of 13.0°, 24.9° and 259.2°, the maintainer withdrew the 259.2° scene — coherence over a liked image — leaving a rotation 11.9° wide. The first time nightshade's own scenes shared a palette. **Still open: the mechanism.** Selection is what happened, and it scales badly: it rejects finished artwork after the fact and gives no guidance before generating. Declaring a hue (§5.4) would do it at packaging time instead, and §4.4's packaging step is where that belongs. Either way a user's media keeps getting whatever it gives, so tenet 6 is untouched. | Principle settled; mechanism open |
| N2 | Does the centre window title stay in the bar (§5)? | Maintainer |
| N3 | How many scenes does nightshade ship? Rotation over two scenes is a two-step cycle, which is barely rotation. **Mechanism built 2026-11-08** — the login screen takes a pool of candidates and advances per launch, so the question is now only the count, not whether it is possible. Two are in the pool. | Maintainer |
| N4 | Fixed clock times or solar phase boundaries (§3.3)? No longer constrained by the format — Q4 chose Lua — so this is now purely what is wanted. Solar is right and costs a location; clock is wrong in December and costs nothing. | Maintainer |
| N5 | Should tenet 2 gain a per-surface clause (§3.2)? | Maintainer |
| N7 | The scrim (§2.4) is the one place a user's file changes how nightshade looks rather than what colour it is. Acceptable, or does nightshade prefer to show bright media undimmed and let the text fight it? | Maintainer |
| N9 | A rotating background and a derived palette pull against each other: scene-01 is 215.9° and scene-05 is 69.3°, 146° apart, and §4.3 gives the UI one accent. Measured harmless for now — each scene clears all four legibility clauses under the other's palette, and `greet.lua` ships no palette table yet — but it becomes real when §4.4's packaging reaches the greeter. The shape is a palette per candidate rather than a second key parallel to the pool; a parallel key is the drift R9 already rejected once. | Maintainer |
| N6 | The name collides with Nightshade, the University of Chicago image-poisoning tool. Different domain, and theme names are namespaced under auroval — noted, not raised as a blocker. | Maintainer |
