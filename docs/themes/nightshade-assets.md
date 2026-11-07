# Generating and judging nightshade assets

Status: **draft for review** · 2026-11-07

Companion to [nightshade.md](nightshade.md), which says what the theme *is*.
This says how its artwork gets made and how it gets accepted or thrown away.

Tracked on [issue 5](https://github.com/auroval/auroval/issues/5).

---

## 1. The mistake this plan exists to avoid

**Artwork cannot be judged on its own.** A scene is not a wallpaper; it is the
thing a clock, a date, a username and a password field sit on top of, every
morning, for a year. An image that is beautiful in a viewer and useless behind
a login form is a failure that only shows up at the last possible moment.

So every judgement in this plan is made on a **composite** — the candidate with
the greeter's own UI drawn over it at nightshade's real geometry — never on the
bare image.

This is not theoretical. The first preview rendered during this work put the
clock directly on a bright sun and made the date line unreadable, on an image
whose overall measurements looked acceptable. Looking at it is what caught it.

---

## 2. The pipeline

```
  brief  →  generate  →  measure  →  preview  →  live with it  →  accept
            (§4)         (§5)        (§5)        (§6)             (§7)
                           ↑                        │
                           └────── iterate ─────────┘
```

Five stages, three of which are gates. Nothing skips a gate, including
artwork the maintainer made himself and likes.

---

## 3. What a scene is

One **scene** = one subject, delivered as:

| Artefact | Count | Purpose |
|---|---|---|
| Master still | 1 | The source of truth. Highest resolution, highest bit depth. |
| Phase stills | 4 | dawn / day / dusk / night. What the desktop shows. |
| Loop | 1 per phase, or 1 shared | What login and lock show. |
| `palette.toml` | 1 | The committed derived palette (see §5.4). |
| `source.md` | 1 | Prompt, model, seed, settings, and the grading recipe. |

`source.md` is not optional. Without it the set cannot be extended in the same
voice a year later, and §1.1 of the theme doc commits to recording it.

---

## 4. The generation brief

Derived from [nightshade.md §1](nightshade.md), not invented here. Every clause
below traces to a rule there.

### 4.1 The constant part

> One subject, centred, filling the upper-middle of the frame, against
> darkness. Rim light and glow only, sourced from behind or beside the
> subject — no frontal key light. Saturated neon chroma: magenta, cyan,
> violet. Near-black field below and around the subject. Visible film grain.
> 16:9. No text, no logos, no watermark.

### 4.2 The negative constraints — these matter more than the positive ones

These are what the measurement actually tests, so they belong in the brief
rather than being discovered afterwards:

- **No bright element in the centre of the frame.** No sun, no moon, no lamp,
  no glowing orb behind the subject's head at centre height. This is the single
  most common failure and it is fatal: it lands exactly on the clock.
- **No horizon line through the middle.** A horizon is a bright band at exactly
  the wrong height. The genre's signature composition is the one nightshade
  cannot use.
- **Nothing important in the lower centre.** The input field lives there and
  will cover it.
- **No fine detail behind the text zone.** Even dark detail costs legibility;
  what reads best is a flat dark gradient.

### 4.3 Composition target

The subject's bright mass sits **above** the clock, not behind it. In practice
that means the subject occupies roughly the top third, with its glow falling
off before the vertical centre. The kimono portrait works because the cyan disc
is behind the *head*, which is above the clock line. Move that disc down 200 px
and the image fails.

### 4.4 Phase variants: grade, do not regenerate

Generating dawn/day/dusk/night as four separate images gives four subjects that
are nearly but not quite the same, and the near-miss is worse than an obvious
difference — a scene that twitches at 09:00 is exactly the thing tenet 3 calls
irritating.

**Generate the subject once. Produce the phases by grading the master.** The
phases are a change in *light*, which is what grading does well and what
generation does inconsistently. A phase recipe is a few numbers — temperature,
exposure, rim-light hue shift, black level — recorded in `source.md` and
reapplied identically for every scene, so all scenes move through the day
together.

This also means a scene's four phases are, by construction, the same picture.
No consistency checking needed, because nothing was regenerated.

---

## 5. The measurement gate

`tools/scene-check/scene_check.py`. It reports; it never refuses — nightshade
refuses nothing (tenet 6), so a failing clause names its degradation instead.

```bash
python3 tools/scene-check/scene_check.py candidate.png --preview /tmp/judge.jpg
```

In nushell, to sweep a directory of candidates:

```nu
ls candidates/*.png | each { |f| python3 tools/scene-check/scene_check.py $f.name --json | from json | select path hue scrim_alpha }
```

### 5.1 What it measures

| Group | Clauses |
|---|---|
| Format | codec and profile, colour primaries, declared range, pixel format |
| Geometry | resolution, aspect |
| Composition | mean luminance and bright fraction over the central 60×70% |
| **Legibility** | the background behind **each UI element separately** |
| Loop | duration, and the seam against an ordinary frame step |
| Palette | the derived palette, plus assertions that it holds its floors |

### 5.2 Why legibility is measured per element

An average over a large safe field hides a small bright thing. During this work
an image measured 0.0905 mean luminance — a miss, but a mild one — while the
date line sat on a disc at **0.9936**. The mean said "slightly bright"; the
render said "unreadable".

So each of the clock, date, username and input field is measured over its own
box, at the **90th percentile** rather than the mean, because the bright part of
a region is what breaks legibility, not its average. Required scrim is computed
per element and the worst one wins.

### 5.3 Composition and legibility are different clauses

They were one number and should not have been:

- **Legibility** is derived, hard, and non-negotiable — it falls out of the
  4.5:1 contrast floor and it is what the scrim exists to fix.
- **Composition** is a look clause. An image can be perfectly legible and still
  not be a subject-on-darkness image. It gets reported and it earns no scrim,
  because dimming a picture for failing a taste rule would be absurd.

A candidate that passes legibility but fails composition is the interesting
case: it will work, and it is not nightshade.

### 5.4 The palette, and a trap worth knowing

The tool extracts hue the way the shipped palette does — matugen, always with
`--dry-run` hard-coded, because without it `matugen image` writes through
`~/.config/matugen/config.toml` and re-themes the running desktop.

**matugen's extraction is sensitive to bit depth.** Measured 2026-11-07 on one
file: the 16-bit original yields `#ffb0cd`, hue 354.8° (pink). The same pixels
re-saved at 8 bits yield `#8ad0ee`, hue 236.5° (cyan). That is 118° apart. Per
file the result is perfectly deterministic — three runs identical — but any step
that re-encodes changes the answer, and `magick -strip` keeping 16 bits
reproduces the original exactly, which rules out the ICC profile as the cause.

Three consequences, all now in force:

1. **Stills are passed to matugen untouched.** The tool never re-encodes one.
2. **A video's canonical frame is extracted by exactly one pinned command**, and
   that frame is what the palette comes from. Change the command, change the
   palette.
3. **The palette is committed, not recomputed.** `palette.toml` ships with the
   scene. [nightshade §4.4](nightshade.md) already required this so the three
   surfaces agree; this is a second and stronger reason — re-derivation is not
   reproducible across pipeline changes.

A scene may also **declare** its hue outright (`--hue 355`), which is the honest
option when extraction picks a colour from the background rather than the
subject. The tool proposes; a person accepts.

### 5.5 The accent is sought, not specified

A fixed chroma target is not realisable. sRGB's gamut varies enormously by hue:
C\* 75 at L\* 57.5 is comfortable for magenta and **impossible** for cyan, which
tops out around 35. Specifying one number gives some hues neon and others
pastel, for no reason a viewer could name.

So the accent is specified as *the most chromatic colour available at a
lightness that still clears the contrast floor*, searched within L\* 50–70. Every
hue then gets the most neon it can actually be. On the measured example this
turns matugen's pastel `#ffb0cd` (C\* 33) into `#ff1296` (C\* 85.4) at the sRGB
cusp — which is the difference between the theme's stated look and its opposite.

N1 remains open: the lightness band and the floor are the maintainer's to tune.
What is settled is the *form* of the answer — seek the cusp, do not name a
chroma.

---

## 6. The judging gate

Three gates, in order. A candidate that fails an earlier one does not reach the
later ones.

### Gate 1 — the machine (minutes)

Run the tool. Read the legibility clauses first; they are the only ones that
can make a scene unusable rather than merely wrong.

**Reject outright:** any element needing a scrim above ~25%. Below that the
scrim is invisible; above it, it is a dark blob and the picture is being
destroyed to rescue text that should not have been there in the first place.

### Gate 2 — the eye (an hour)

Look at the composite, full screen, at native resolution. Not in a thumbnail,
not in a browser tab.

Questions, in this order:

1. Can you read the date line without trying? (The smallest light text, over
   the busiest part. It fails first.)
2. Does the clock sit *in* the picture or *on* it?
3. Is the input field's outline visible without being hunted for?
4. Where does your eye go first? If the answer is not the subject, the
   composition is wrong regardless of what it measured.
5. Does the palette swatch strip look like the picture? If the accent is a
   colour you cannot find in the image, the hue came from the background and
   should be declared instead (§5.4).

### Gate 3 — time (two weeks)

**This is the gate that cannot be hurried, and it is the one tenet 3 is about.**
*"Het moet tijdloos zijn en niet gaan irriteren."* Nothing in gates 1 and 2
measures that. Neither does any number.

The protocol: set the candidate as the desktop still and live with it for **two
weeks** before it is allowed into the set. Not as the login screen — as the
desktop, where it is in peripheral vision for hours rather than seconds. That
is the harsher test, and a scene that survives it will survive the greeter.

Reject on any of:

- You notice it. A scene that keeps catching your eye after a fortnight is a
  scene that will keep catching your eye for a year.
- You have started avoiding a part of the screen.
- You can predict exactly what it looks like before you look. (Different from
  not noticing it: this is boredom, not calm.)

**The honest cost:** two weeks per candidate does not scale, and it is the
reason nightshade should ship a small number of scenes rather than a library.
Tenet 4 makes this acceptable — the maintainer's own daily use is the test, and
there is exactly one maintainer. It would be unworkable for anything else.

---

## 7. Loops: build them, do not generate them

The hardest clause in the contract is R8 — a loop with no visible seam. Three
routes, and the third is recommended.

| Route | Seam | Consistency with the phase stills | Verdict |
|---|---|---|---|
| Generate video directly | Not seamless; needs a cut hidden somewhere | The subject drifts from the stills | Hardest, worst result |
| Generate short clip, ping-pong it | Seamless by construction | Same drift problem | Motion visibly reverses — fine for smoke, wrong for anything directional |
| **Animate the master still** | **Exact, by construction** | **Identical by construction** | **Recommended** |

**Drive every motion with a sine over the loop period.** A parameter that
returns to its starting value and starting *velocity* at the loop boundary has
no seam at all — not a small one, a mathematically absent one. R8 stops being a
measurement and becomes a property.

What that buys, specifically:

- The loop and the phase stills are the same picture, so login, lock and desktop
  never disagree about what the scene is.
- The motion can be made as subtle as wanted, which is what the desktop needs
  anyway — *"should not animate noticably and be distracting. Subtle is ok."*
- No video model has to hold a subject stable for 20 seconds, which is currently
  the thing they are worst at.

Candidate motions, all sine-driven and all cheap: slow parallax between
separated depth layers; drifting grain; a breathing rim-light intensity; slow
drift of a glow's position; particles on a closed path.

**Cost, stated plainly.** This is a renderer feature, not an asset. It needs
layer separation on each master and a small amount of motion code, which is
work that generating an MP4 would have avoided. It also caps how alive a scene
can look: nothing walks, nothing turns. For a theme whose desktop is explicitly
meant not to move, that cap is close to free — but it is a real limit and it
would be the wrong choice for a theme that wanted spectacle.

---

## 8. What is already settled

| | |
|---|---|
| matugen fixes tones; only hue varies | Measured, four sources, five scheme types |
| Bit depth changes extracted hue by up to 118° | Measured, same file at 16 and 8 bits |
| Extraction is deterministic per file | Three runs each, identical |
| A fixed chroma target is unreachable across hues | sRGB cusp at L\* 57.5 ranges from ~35 to ~85 |
| Mean luminance does not predict legibility | 0.0905 mean, 0.9936 behind the date line |
| `--prefer` is load-bearing | Without a value matugen exits non-zero with no TTY |

Regression: `python3 tools/scene-check/scene_check.py --matugen-assertions`
re-runs the first of these and fails loudly if matugen's behaviour changes.

---

## 9. Open

| # | Item | Owner |
|---|---|---|
| A1 | Which generator, and is its licence compatible with shipping the output? Not settled, and it gates everything. | Maintainer |
| A2 | The phase grading recipe (§4.4) — the four parameter sets, fixed once and shared by all scenes. | Maintainer |
| A3 | Does `--prefer saturation` pick the subject's colour or the background's? On the measured example it picked the background until the file was passed untouched. If it keeps missing, scenes declare their hue (§5.4). | Maintainer |
| A4 | The reject threshold for scrim depth (§6, gate 1) is a guess at 25%. Needs one real rejection to calibrate. | Maintainer |
| A5 | Layer separation for §7 — produced by the generator, or cut by hand per master? Decides whether loops are cheap or expensive. | Maintainer |
| A6 | Two weeks per candidate (§6, gate 3) is a guess. It may be one week; it is certainly not one day. | Maintainer |
