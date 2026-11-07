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
set of files. The contract is what a user's own media has to satisfy to be
nightshade media.

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
more than 2% of pixels above 0.25. A frame that fails is not nightshade media.

### 2.3 Loop

| Property | Requirement | Why |
|---|---|---|
| Duration | 10–30 s | Long enough not to read as a repeat, short enough to hold in memory and to seek cheaply. |
| Seam | First and last frame differ no more than two ordinary consecutive frames | This is R8's fit criterion, promoted into the contract so it is checked at packaging rather than discovered at 3 a.m. |
| Motion | Slow, continuous, no cuts | §3 explains why this is a hard limit and not taste. |

### 2.4 Enforcement

Media that fails the contract is **refused, with a reason naming the clause**.
It is not accepted and corrected.

> *Chosen deliberately: "Refuse it, with a reason."*

**Where the refusal happens matters.** Checking is done when media is packaged
or imported into a theme — never at login. A greeter that rejects a background
while you are trying to log in has broken the one goal the PRD calls
non-negotiable: *"Authentication is never the thing that breaks."* At runtime
the greeter still falls back per R9 (still image, then solid colour) and logs
it. The contract is a packaging-time gate; R9 is the runtime floor. Both exist.

**Cost, stated plainly.** This bites tenet 6 directly and knowingly. Someone's
favourite video gets turned away with a message about luminance in the central
60×70 region. Tenet 6's own cost clause anticipated exactly this and chose the
other way — *"the user's own file wins"* — and nightshade is, on this point,
stricter than the tenet. It is a theme-level choice, not a project-level one:
a different theme is free to accept anything. But it should be read as what it
is, which is the first place a theme has pushed back on a tenet.

---

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
you can see over motion you infer"*, with the cost that the whole premise rests
on decode working in the greeter's context (C6, unverified, P0's job). Split by
surface, that cost shrinks: the desktop is unaffected by a decode failure
because it was never going to move. Tenet 2's *"expires if"* clause — "then
motion is a lock-screen feature only" — now has a cheaper failure mode than it
anticipated. **This does not amend tenet 2;** it is flagged here so the
maintainer can decide whether tenet 2 wants a per-surface clause.

It also sits squarely with tenet 3. A moving background on a login screen is
seen for four seconds a day. A moving background on a desktop is in your
peripheral vision for eight hours, which is precisely the thing that *"gaat
irriteren"* by week two.

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

### 4.2 Derive once per scene, at packaging time

A detail that falls out of §1.1 and §3.2 and matters more than it sounds.

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
- The §2.4 gates can be evaluated at the same moment, which is what makes
  packaging-time refusal possible at all.

**Consequence to state plainly:** live re-derivation on wallpaper change goes
away. Today `awww-wallpaper.sh` runs `matugen image "$wp" --prefer saturation`
and nudges five consumers. Under nightshade the palette is a property of the
scene, already computed. matugen moves from runtime dependency to packaging
tool. That is a change to something the maintainer currently has and uses, and
it should be a conscious trade rather than a side effect.

### 4.3 The gates

The derived palette must satisfy these, or the media is refused (§2.4):

| Gate | Threshold | Why |
|---|---|---|
| `surface` darkness | L\* ≤ 20 | The dark field is the composition. A light surface is a different theme. |
| `on_surface` vs `surface` | contrast ≥ 7:1 | WCAG AAA for body text. The input field's placeholder sits here. |
| `primary` vs `surface` | contrast ≥ 4.5:1 | The accent is also the focused border and the input outline; it has to be seen, not merely present. |
| `primary` chroma | above a floor, to be set | A desaturated accent is not neon. This is the gate that keeps a grey video out, and it is the one most likely to need tuning against real generated media. |
| `error` vs `surface` | contrast ≥ 4.5:1 | R3 renders auth failures here. A failure message you cannot read is a lockout. |

Shape stays as it is today: the matugen template writes bare 6-digit hex with
no leading `#`, so callers append their own alpha byte — *"that is the shape
Hyprland's `rgba()` wants"*, per the existing template's own comment. No reason
to change it.

### 4.4 Tenet check

Tenet 1 refuses per-element overrides, and that refusal is only honest if the
whole is reliably good. Derive-everywhere plus arbitrary media is the
combination that breaks that promise: a bad file produces a bad desktop and the
user is told they may not fix one colour. §2.4's refusal is what keeps tenet 1
defensible. The two decisions stand or fall together — accepting arbitrary media
*and* refusing overrides would have been the worst of both.

---

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

### R11 must be rewritten

[R11](../prd/greet.md#appearance) currently reads:

> Reproduce hyprlock's visual language: **Catppuccin Mocha**, MesloLGS Nerd
> Font, the large clock, and the rounded input field.

§4.1 contradicts the first item directly. nightshade derives the palette from
the media on all three surfaces; hyprlock's Catppuccin Mocha is the pin that
makes the lock screen inconsistent in the first place. Keeping R11 as written
would mean shipping the fault the project was started to fix.

The rest of R11 survives intact — the font, the large clock, and the rounded
input field are all kept, and §6 adds the squircle radius and the border
treatment that R11 did not name.

**Proposed replacement, for the maintainer to accept or reject:**

> **R11 (MUST)** — Reproduce hyprlock's geometry and typography: MesloLGS Nerd
> Font, the large clock, and the rounded input field at the desktop's own
> corner radius and rounding power. Colour comes from the active theme's
> derived palette (§4), not from a fixed scheme.
>
> **Fit criterion:** side-by-side with hyprlock at the same resolution, the
> font, clock size and the input field's size and rounding are identical;
> palette values match the active theme's table on all three surfaces and are
> byte-identical between greeter, lock and desktop.

Also affected, more mildly: **R12** (time-of-day variants) is currently SHOULD.
§3.1 makes it part of what a theme *is*, which argues for MUST — but that is a
PRD decision and is left open here.

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
| N1 | The `primary` chroma floor (§4.3) — the gate most likely to be wrong on first contact with real generated media. | Maintainer, after the first scene exists |
| N2 | Does the centre window title stay in the bar (§5)? | Maintainer |
| N3 | How many scenes does nightshade ship? Daily rotation over two scenes is a two-day cycle, which is not rotation. | Maintainer |
| N4 | Fixed clock times or solar phase boundaries (§3.3)? Materially changes what the format must express. | Maintainer, with Q4 |
| N5 | Should tenet 2 gain a per-surface clause (§3.2)? | Maintainer |
| N6 | The name collides with Nightshade, the University of Chicago image-poisoning tool. Different domain, and theme names are namespaced under auroval — noted, not raised as a blocker. | Maintainer |
