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
  brief  →  generate  →  render  →  measure  →  preview  →  live with it  →  accept
            (§4)         (§7.1)     (§5)        (§5)        (§6)             (§7)
                            ↑                      │
                            └─────── iterate ──────┘
```

Six stages, three of which are gates. Nothing skips a gate, including artwork
the maintainer made himself and likes.

The **render** stage was not in the first version of this document, and it is
the single biggest thing building the first scene changed. It is where anything
that has to agree with the scene's own light gets drawn rather than painted:
see §7.1. Putting it before `measure` is deliberate, because what the gates
judge has to be the thing that ships.

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

The brief has two halves and it is worth being explicit about which is which,
because an earlier version of this section was not and the omission was costly
— see §4.6.

- **Constant across the whole theme**: the frame, the lighting, the chroma, the
  grain, the composition rules, and the **rendering style** (§4.1). Every scene
  in nightshade shares these. Changing any of them does not produce a nightshade
  variant; it produces a different theme.
- **Varies per scene**: the **subject**, and only the subject (§4.5).

That split is tenet 1 at the level of artwork. A theme whose scenes disagree
about how they are drawn is the *"four programs that have never met"* failure
the thesis names, arriving through the back door.

### 4.1 The constant part — for the greeter

> One subject, centred, filling the upper-middle of the frame, against
> darkness. Rim light and glow only, sourced from behind or beside the
> subject — no frontal key light. Saturated neon chroma: magenta, cyan,
> violet. Near-black field below and around the subject. Visible film grain.
> 16:9. No text, no logos, no watermark.

#### 4.1.2 "Centred" is wrong for the greeter, and it is arithmetic

The blockquote above asks for *"one subject, centred, filling the
upper-middle of the frame"*. `tools/nightshade-gen/generate.py` no longer
says that, and the disagreement is deliberate and recorded here rather than
left to be discovered.

`scene_check.py`'s `ui_regions` places the clock at `cy-220`, the date at
`cy-130`, the username at `cy-40` and the input field at `cy+40`. The UI
occupies the middle of the frame from well above centre to just below it.
§2.2 asks for that region to be dark enough to read white text on. §4.1 asks
for a lit subject to be exactly there.

They are the same rectangle, and across eighteen candidates generated on
2026-11-08 the two were never both satisfied. §5.3's split made the failure
legible — it is confined to the subject's own column — and the candidate that
finally passed composition, `pond-warm-b-s11` at 1.74% above 0.25, still
demanded a **90% scrim**, because its subject sits where the username does.

A subject placed off to one side costs the composition nothing and hands the
UI an empty column. So the greeter's constant part now reads:

> one subject placed well off to one side of the frame, never in the middle.
> the subject high in its half of the picture, against darkness. the whole
> central band of the frame empty near-black space from top to bottom.

and §4.2's first bullet, which used to confine every bright area to the top
of a frame whose subject was centred, now confines them to one side instead —
which is the same prohibition without the self-contradiction.

**This does not apply to the desktop**, which has no UI on it and whose
constant part is unchanged.

**It also means scene-02's candidates cannot be reproduced from this
document.** Their prompts used the previous wording. They are recoverable
because every run writes the string it actually used (§3), which is the first
time that provision has earned its keep.

**This paragraph is a greeter brief, and §4.2 entirely so.** §1 says why
without quite saying the consequence: *"A scene is not a wallpaper; it is the
thing a clock, a date, a username and a password field sit on top of."* Three
of the clauses above and all four of §4.2's exist to keep a rectangle clear
for UI, and §3.1 nonetheless has the same scenes rotating daily as the
desktop wallpaper, where there is no UI to keep clear.

Applied to a wallpaper they are not merely unnecessary. "Near-black field
below and around the subject" and §4.2's "nothing important in the lower
centre" produce a frame that is two-thirds empty, which on a desktop reads as
a login screen missing its login form — because that is exactly what it is.
Six scene-02 candidates were generated this way on 2026-11-08 before anyone
noticed, and nothing in the brief or the tool could have caught it.

So the two surfaces take different constant parts, both recorded in
`tools/nightshade-gen/generate.py`, and a brief must name its `surface`. What
is shared is what nightshade *is*: the rendering style of §4.1.1, neon rim
light and glow, deep darkness, grain, 16:9, no marks. What differs is the
framing — the greeter's subject sits in cleared darkness, the wallpaper's
fills the frame and the dark sits between the forms instead of instead of
them.

**Open, and it belongs to §3.1 rather than here.** If one scene is to serve
both surfaces, it has to be generated twice, or the daily rotation has to
draw from a wallpaper set that is not the greeter set. Neither is written
down.

#### 4.1.1 Rendering style — one theme, one style

**A theme has exactly one rendering style, and it is not negotiable per scene.**
nightshade's is recorded here once:

> **Ink-and-wash painting.** Sumi-e brushwork on toned paper: fine
> calligraphic black linework, soft pigment washes, visible brush and paper
> texture. Painted, never photographed.

The operative string, recorded because it is what actually reproduces the look
and an art-historical label is not:

> `Sumi-e ink wash painting with bold calligraphic brushstrokes on toned paper,`
> `minimal flat colour accents, visible brush texture, no photographic realism`

Chosen 2026-11-07 by comparing six candidates generated from an identical
subject and seed, varying only this clause: ukiyo-e woodblock, ema votive
panel, nihonga, maki-e lacquer, two-colour screenprint, and this. Those
comparison images are not committed — the decision is, and it is reproducible
from the strings above.

Why this clause exists at all: nothing in the brief used to name a rendering
style, and a generator with no style instruction returns **photorealism every
time**. That is a default, not a choice, and it is not what the references look
like — the kimono portrait reads as Japanese figurative art rather than as a
photograph of a woman.

Why it is a theme constant rather than a per-scene setting: two scenes drawn in
different styles do not read as one theme no matter how well they match on
palette, lighting and composition. The style is the most visible thing about an
image and the least forgiving of inconsistency.

**Consequence worth stating plainly.** Wanting nightshade's subjects in a
different style is wanting a different theme, and the answer is to author one —
which is exactly what [tenet 1](../tenets.md) says and exactly the answer it
admits is steep.

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

### 4.5 The variable part — the subject

This is the half that changes per scene, and it is the half the brief used to
omit entirely.

[nightshade.md §1](nightshade.md) sets the vocabulary without quite stating it
as one: *"the skull pink-on-near-black under heavy grain, the kimono portrait
with a cyan disc behind the head"*, and later *"A scene is one subject — one
skull, one portrait"*. What those two share is not iconography but **density**:
each is a single object carrying a lot of ornament, and the ornament is why the
image holds up at the fiftieth look rather than the first.

A subject qualifies when it is:

- **Singular.** One object or one figure. Not a scene, not a pair, not a
  landscape with something in it.
- **Ornamented.** Carrying surface detail that rewards looking — embroidery,
  engraving, inlay, filigree, botanical complexity. A plain object lit well is
  what the brief produced before this section existed, and it was correct and
  dull.
- **Edged.** This is the clause that is easiest to lose and hardest to recover.
  The rendering style is traditional; **the subject matter must not be.** The
  reference kimono portrait is not a portrait of a geisha — it is a geisha
  whose kimono is covered in skulls, with a fan of elaborate pins through her
  hair. The form is traditional and the content subverts it, and that tension
  is the whole reason the image holds attention.

  Generated without this clause, the model returns cranes and florals every
  time: competent, decorative, and exactly the thing the reference is reacting
  against. Name the darker element explicitly in the subject phrase — skulls
  among the florals, spider lilies, a cracked mask, bone, chain — or it will not
  appear. This is a per-asset instruction precisely because *what* the edge is
  changes per scene, while *that* there is one does not.
- **Closed in silhouette.** It reads as a shape against black, with the field
  around and below it staying dark.
- **Still.** Nothing mid-motion. §7's loops animate by parallax over depth
  layers, which a subject caught mid-gesture fights.

Record the subject phrase for each scene in that scene's `source.md`, because
it is the one part of the prompt that is not shared and therefore the one part
that cannot be reconstructed from this document.

### 4.6 What the first generation trial found

Run on 2026-11-07 with FLUX.2 [klein] 4B, six candidates from the brief as it
then stood, then eight more with a subject named. Three findings, all of which
changed this document rather than the tooling.

**The brief produced correct, compliant, boring images.** Every one of the
first six passed all seven clauses. They were also characterless — a generic
head, well lit and well placed. That is not a model failure: §4.1 called itself
*"the constant part"*, no counterpart existed, and so the subject was never
specified. Naming one produced the kimono portrait and the engraved skull on
the first attempt. §4.5 exists because of this.

**Nothing named a rendering style, so everything came back photoreal.** §4.1.1
exists because of this.

**§4.3 constrains the subject's glow but not its body, and that is not enough.**
It says the bright mass must sit above the clock, and the kimono candidate
obeyed that exactly — the cyan disc sat high and correct. It still scored
maximum scrim, because the *torso* filled the centre and lower-middle where the
input field goes. §4.3's rule should be read as applying to the subject's whole
extent, not only to the part that is emitting light.

**The gates cannot see any of this.** Scrim depth and the composition clauses
measure legibility, not whether the brief was followed. The best-scoring
candidate of the first six had **no subject at all** — an empty glow on black,
scrim 0.000 — and a black frame would have scored better still. Gate 1 and
gate 2 can reject an unusable image; only a human can reject an uninteresting
one. That is what gate 3 is for, and it is a reason not to automate gate 3 away
later when it feels slow.

---

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
| Composition | mean luminance and bright fraction **around** the subject, inside the central 60×70% (§5.3.1); the subject column and the lower centre are reported beside it, ungated (§5.3.2) |
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

#### 5.2.1 The four boxes are four, and the greeter draws five things

The session label is the fifth, and it is measured by nothing. Found on
2026-11-09 by looking at the login screen, which was the only reader it had.

It sits 64 px from the bottom edge, which is **129 px below the safe field**,
so §2.2's composition clause cannot reach it and §5.2's legibility clauses
cover only the four boxes above. Measured behind it on scene-01, at the same
90th percentile this section uses:

| | background p90 | contrast | |
|---|---|---|---|
| as deployed | 0.8773 | **1.97:1** | against a 4.5:1 floor |
| with a plate | 0.0140 | 7.37:1 | |

Both numbers read off a rendered frame rather than reasoned about. The first
attempt at the second one came out at 3.04:1, because the measurement box
contained the label's own glyphs and the 90th percentile found them: on a
rendered frame, unlike on a candidate, the text is in the picture.

**A colour cannot fix it and neither can translucency.** The alpha needed to
reach the floor is 0.951, so the input field's own 0.8 gets to 2.05:1 — on a
near-white background, translucency buys almost nothing. The brighter palette
roles are worse rather than better, because they are lighter, and a darker
text colour fails on scene-05, whose background at that spot is 0.0175. With
§3.1's pool rotating between the two there is no single colour that works.

So the greeter draws an opaque plate, and the artwork is **not** judged on
this element — adding it to the measured boxes would reject scene-01 for a
problem the plate has already solved, and at 83–90% required scrim it would
fail §6's gate 1 outright. The invariant moves into the greeter's own tests
instead, where `the_session_label_sits_on_a_plate` renders a frame and reads
the pixels back.

The general point is worth more than the fix. The contract measures the
elements it knows about, and the greeter grew one it did not. Any element
drawn outside the safe field is invisible to this document by construction,
which is a thing to check when one is added rather than to discover on the
screen.

### 5.3 Composition and legibility are different clauses

They were one number and should not have been:

- **Legibility** is derived, hard, and non-negotiable — it falls out of the
  4.5:1 contrast floor and it is what the scrim exists to fix.
- **Composition** is a look clause. An image can be perfectly legible and still
  not be a subject-on-darkness image. It gets reported and it earns no scrim,
  because dimming a picture for failing a taste rule would be absurd.

A candidate that passes legibility but fails composition is the interesting
case: it will work, and it is not nightshade.

#### 5.3.1 Composition is measured in the centre, where the UI goes

The composition clause first averaged over the whole central 60×70%, and in
that form it never passed. Not once, across every candidate of every scene.

It could not. §2.2 asks for mean luminance ≤ 0.06 over that rectangle while
§4.1 asks for "one subject, centred ... rim light and glow only" — **in the
same rectangle**. The two clauses want opposite things and §4.1 wins every
time, because the glow is what makes the picture. §4.1's own wording says
where the darkness is supposed to be, and it is not "everywhere": *"near-black
field below and **around** the subject"*.

The symptom was that the measure stopped telling images apart. scene-01's
master and its `v5-nacre` loop scored 0.1901 and 0.1873 — the same number, to
within noise — and they are **opposites**: the master's light is entirely in
its subject and its surround is near-black, the loop's surround is the bright
part. A measure that calls those two the same is measuring the wrong region.

So a centred column of 40% of the safe field's width — 24% of the frame — was
excluded, and the clause gated on what was left.

**That lasted an afternoon, and §4.1.2 replaced it with something simpler.**
The split made the failure legible without removing its cause: §4.1 still
asked for a lit subject in the rectangle §2.2 wanted dark. Moving the
greeter's subject off centre removes the cause, and once the subject is not in
the middle, the middle is simply the thing to measure — which is the clause
§4.2's first bullet always stated, *"no bright element in the centre of the
frame"*.

The clause is therefore **gated on the central column** and the flanks are
reported. The flank figure is not decoration: it catches a frame that passes
only because it is dark everywhere, and A8 already noted that scrim alone
rewards emptiness.

The table below is kept as it was measured, under the split, because it is
what justified the change of subject placement rather than the change of
gating.

| | whole field | subject column | surround (gated) |
|---|---|---|---|
| scene-01 `master` | 0.1901 | 0.3915 | **0.0561** |
| scene-01 `v5-nacre` | 0.1873 | 0.1204 | **0.2319** |
| scene-02 `shrine-b-s7` | 0.0992 | 0.1726 | **0.0503** |
| scene-02 `shrine-s1` | 0.1653 | 0.1700 | **0.1623** |

`--self-test` builds two frames carrying the same light in the same safe field
at different places, and asserts that the whole-field number cannot separate
them while the gated number does. Those two assertions were inverted when the
gating moved, which is the clearest statement of what changed: light in the
centre used to pass and now fails, light off to one side used to fail and now
passes.

A third assertion was **not** inverted, and the test refused the attempt. A
small bright disc on the date line still passes composition, reading 0.0167
over the column against a 0.06 limit while sitting at 0.99 on the date line
itself. That is §5.2's lesson intact — *"an average over a large safe field
hides a small bright thing"* — and it is why legibility is measured per
element and not inferred from this clause.

**On the bright-fraction threshold, written and then corrected the same day.**

What stood here said that ≤ 2% above 0.25 was met by nothing, that the best
images sat at 7.6% and 7.8%, and that the threshold was therefore calibrated
against an idea rather than against the reference — filed as A10, proposing
recalibration.

That was a conclusion drawn from the eight images that existed at the time,
and scene-04 falsified it a few hours later. `pond-warm-b-s11` measures
**0.0353 mean and 1.74% above 0.25**, and passes §2.2's composition clause
outright — the first candidate of any scene to do so.

So the threshold is reachable and A10 is withdrawn. Acting on it would have
loosened a limit that works, on the evidence that nothing had yet reached it,
which is an argument from a small sample and not from the measure.

What it took was not a tuning change. It was a brief that stated darkness as
appearance rather than as lighting (§5.5.1): *"gone almost black with wet moss
and shadow"*, *"everything that is not a fish sunk in deep shadow"*.

#### 5.3.2 The lower centre, and why §4.1.2 was the fix

Found by review of the split in §5.3.1, before §4.1.2 landed. Worth keeping,
because the obvious repair was wrong and the one that worked did not touch
this clause at all.

Gating the surround and excluding the subject's column left the region §4.2
is most specific about — *"nothing important in the lower centre"*, *"the
lower middle of the frame is empty near-black space"* — read by nothing. The
column ran the full height of the safe field, so composition excluded it by
construction; legibility measures only where UI lands and the greeter's
lowest element stops at 56% of the height, while the safe field reaches 85%.

A frame of near-black with one bright disc at 72% of the height cleared
**every clause the tool had**: composition passed with the surround at 0.00%
bright, legibility passed because no UI is down there, and it needed no
scrim. The whole-field measure it replaced had failed it at 4.60%.

**Bounding the excluded column at the bottom of the UI stack was tried and
is worse.** It moves scene-01's `master.png` from 0.0561 to 0.1381 and fails
the one image the split had finally made measurable, because that subject
does not stop at the input field — it falls away into darkness below the
*frame*. Measured on the candidates of the time, with the lower centre as a
region of its own:

| | surround | lower centre |
|---|---|---|
| scene-01 `master` | 0.0561 / 7.62% | **0.4329 / 55.12%** |
| scene-01 `v5-nacre` | 0.2319 / 29.13% | 0.1339 / 18.93% |
| scene-01 `v2-moths` | 0.0161 / 2.58% | 0.1435 / 19.49% |
| scene-05 `lantern-s11` | 0.0407 / 6.77% | 0.0022 / 0.08% |
| the synthetic disc above | 0.0014 / 0.00% | 0.2458 / 27.52% |

Every scene-01 candidate fails that region and only scene-05 passes it, so
gating it would have rejected the reference. The region was therefore briefly
reported and not gated, filed as A12.

**§4.1.2 dissolved it.** Moving the greeter's subject off centre means the
centre is simply the thing to measure, the gated column runs the full height,
and the lower centre is inside it — §4.2's first bullet, *"no bright element
in the centre of the frame"*, measured literally. No separate region, no
second threshold, nothing ungated. The disc above now fails composition at
11.50% bright, and the self-test pins that.

The lesson is the one A10 taught a few hours after it was filed: a measure
that disagrees with the reference is a question, not a defect to patch. Both
times the answer was upstream of the measure — in the brief for A10, in the
composition for this one.


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

**This is an offline production step, not a renderer feature.** An earlier
version of this section said the opposite, and it mattered: described as a
renderer feature, sine-driven motion implied an architecture in which the
greeter composites depth layers each frame and needs no decoder at all. That
fork was raised on #18 and **the maintainer decided it on 2026-11-07: bake.**

Tenet 6 is what settles it. *"mensen moeten zelf ook eigen video's in kunnen
gebruiken"* — a user points the greeter at an arbitrary MP4, which has no depth
layers in it, so runtime compositing cannot serve the media the project has
committed to supporting. The decoder is mandatory, which means live compositing
would add a second motion path rather than replace the first. See
[`../ideas.md`](../ideas.md), where it is parked with its trigger condition.

**Nothing in this section is lost by that.** The sine argument is about the
*parameter*, not about when it is evaluated: a value that returns to its
starting value and starting velocity at the loop boundary has no seam whether it
is evaluated at 60 Hz in a renderer or written to frames by a script the night
before. So the route stays recommended and the pipeline becomes: animate the
master still offline with sine-driven parameters, render the frames, encode an
HEVC loop. R8 is still satisfied by construction.

**Cost, stated plainly.** It needs layer separation on each master and a small
amount of animation code, which is work that generating an MP4 would have
avoided. It caps how alive a scene can look: nothing walks, nothing turns. For a
theme whose desktop is explicitly meant not to move, that cap is close to free —
but it is a real limit and would be the wrong choice for a theme that wanted
spectacle.

**One residual risk, and it is the encoder's — now measured.** The seam is
exact in the frames and only *nearly* exact in the file: inter-frame
compression means the last encoded frame and the first are not reconstructed
identically, and a keyframe at the loop point can differ in quality from the
predicted frames before it. That is a subtle pop rather than a visible cut, and
it is what R8's fit criterion measures.

On scene-01's 10 s HEVC loop at CRF 16: last→first **0.00394**, an ordinary
consecutive step **0.00394**. Identical, so the boundary is not merely within
tolerance but indistinguishable from any other frame step. The risk is real and
on this encode it did not bite.

**R8's criterion should not be read alone, however**, and that is the more
durable finding. It is a *ratio*, and a ratio improves when its denominator
gets worse: an all-intra encode "passes" by making every frame noisier, which
is a worse file scoring better. An earlier attempt did exactly that. The
criterion needs an absolute floor as a second term, the same shape of fault A8
records for the scrim threshold. Until it has one, a pass on R8 is evidence and
not a verdict. See §10.4.

### 7.1 The render stage: draw what has to answer to the light

`tools/nightshade-loop/`. Built for scene-01 on 2026-11-07 and generalised
back into this document afterwards.

The scene needed a pendant hanging from one of her hairpins, and the generator
would not put one there — repeated, explicit instruction about structure was
ignored while instruction about style and subject was obeyed (§10). So the
charm was taken from a *different* generated image and composited in.

**Pasting a bitmap failed, and failed for a reason worth generalising.** A
region cut out of render B carries B's lighting, B's grain and B's perspective
into a scene that has none of them. At the size it needed to be, a painted bead
strand was a grey sliver. No amount of grading fixed it, because the problem
was not the levels; it was that the object did not belong to the light.

So the rule: **anything that must respond to the scene's light is drawn under
that light, not painted and pasted.** In practice that splits the artwork in
two.

| | Comes from | Why |
|---|---|---|
| The scene | The generator | A painting. It is the source of truth and nothing redraws it. |
| Anything added to it | A shader | It has to obey the scene's key light, and a shader can be told where that light is. |

For scene-01 that meant beads as analytic spheres with real thin-film
interference — the optical path through an aragonite film a few hundred
nanometres thick, which is why the colour travels across a bead instead of
sitting on it — and the skull keeping its painted albedo while gaining a normal
map built from its own alpha and luminance. Both lit from the same disc.

**The loop's physics and its seam are reconcilable, and the reconciliation
generalises.** §7 says to drive every motion with a sine over the loop period,
which is right and is what the camera path still does. But a chain cannot be
driven that way: a bead strand moving on a sine reads as fake instantly,
because the tell is the chain's *lag* behind its anchor. A free simulation, on
the other hand, never returns to where it started.

The way out is to stop trying to make the simulation return and let it forget.
A damped chain under a periodic drive has a steady state whose period is the
driving period; the transient from the initial conditions decays. Drive at a
harmonic of the loop, run 24 periods, record the 25th. Measured closure:
**0.0000 px**. The drive sits near the chain's own natural period on purpose —
off resonance a chain merely follows its anchor, near it the chain swings
several times further than the drive does.

**Parallax without cutting layers.** §7 assumed depth-layer separation, which
is open item A5 and expensive. A monocular depth map plus a one-step
displacement gets most of it for nothing — but it smears silhouettes, because
the displacement reads the depth at its *destination*, so next to her edge a
foreground pixel takes its colour from behind her. Growing the near depth
outward by more than the largest displacement (25 px against a maximum 23 px)
removes the smear entirely.

The cost, stated rather than buried: the background within those 25 px now
travels with her. That is not occlusion, and against a glowing disc it is far
less visible than a ghost of her jawline, but A5 is still open and this is a
cheaper alternative rather than an answer.

### 7.2 Generate the background separately — the route to A5

The maintainer's proposal, 2026-11-07, and the right one: generate the
background as its own image instead of cutting the figure out of a finished
scene. Then layer separation is not a extraction problem at all. The plate
behind her was never occluded, so there is nothing to inpaint and nothing to
guess, and real parallax --- the kind where the disc actually moves behind her
--- becomes possible instead of the depth-dilation compromise in §7.1.

Three ways to get there, and they are not equally good.

| Route | How | Verdict |
|---|---|---|
| Two text-to-image passes | Generate the plate, then the figure | **No.** Two passes will not agree on light, style or disc position, and nothing forces them to. |
| Inpaint the figure out of a finished scene | One scene, then an editing pass that removes her | Works, keeps agreement by construction, but needs an inpainting-capable model and the result behind her is invented anyway. |
| **Render the background, generate only the figure** | The disc is a radial gradient with grain --- a shader, not a painting. Generate the figure on a flat plate and matte it. | **Recommended.** Agreement is guaranteed because there is only one generated layer. |

The third route also pays forward: a disc that is drawn rather than painted can
breathe, drift and change hue per phase (§4.4) without regenerating anything,
and it is the same argument §7.1 already makes about the charm.

**The cost, and it is the serious one.** What makes these images good is the
rim light the model puts on her hair and shoulders *because it knows the disc
is there*. Generate her on a flat plate and that interaction is gone; it has to
be added back in the shader, from her matte's own normal, the way the skull
already is. That is the hardest part of the look to fake and the part most
likely to read as a cut-out --- which is exactly the failure §7.1 exists to
avoid. It should be prototyped on one scene before the brief commits to it.

Matting is already half-solved: BiRefNet (MIT) and ViTMatte (Apache-2.0) were
both trialled while cutting the charm, and both are shippable licences.

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
| A5 | Layer separation for §7 — produced by the generator, or cut by hand per master? **Partly relieved 2026-11-07:** §7.1 gets usable parallax from a monocular depth map with no layers at all, so this is no longer blocking. §7.2 proposes the actual answer — render the background and generate only the figure — and names the one thing that could sink it. Needs a prototype on one scene. | Maintainer |
| A6 | Two weeks per candidate (§6, gate 3) is a guess. It may be one week; it is certainly not one day. | Maintainer |
| ~~A7~~ | ~~nightshade's rendering style~~ **Settled 2026-11-07: ink-and-wash (sumi-e), recorded in §4.1.1.** Chosen from six candidates generated from an identical subject and seed. The paired finding went into §4.5: the style is traditional, the subject matter must not be. | Settled |
| A8 | A4's scrim threshold now looks like the wrong axis on its own. Measured: the only candidate with correct composition scored 0.275, just *above* the 25% guess, while a candidate with **no subject** scored 0.000. Scrim alone rewards emptiness, so the reject rule needs a second term — minimum subject extent, or ornament density — or gate 1 will systematically prefer duller images. | Maintainer |
| A9 | The generator obeys instruction about style and subject and ignores instruction about structure (§10). Is that FLUX.2 [klein] specifically, a 4B-parameter limit, or every model? Decides whether the filter stage can ever shrink. | Maintainer |
| A10 | **Withdrawn the day it was filed.** Proposed recalibrating the composition clause's bright-fraction threshold (≤ 2% above 0.25) because no image had met it. `pond-warm-b-s11` then measured 1.74% and passed the clause outright (§5.3.1), so the threshold is reachable and loosening it would have removed a working limit on the evidence of a small sample. What it took was a brief stating darkness as appearance rather than as lighting (§5.5.1). | Withdrawn |
| ~~A12~~ | ~~§4.2's lower centre is read by nothing~~ **Settled 2026-11-09 by §4.1.2.** The surround-gated split left the lower centre unmeasured by any clause, and a frame that was one bright disc at 72% of the height cleared all of them (§5.3.2). Bounding the excluded column would have failed scene-01's own master. Moving the subject off centre made the centre the gated region, which contains it. | Settled |
| A11 | Hue is not stable across seeds of one brief: six scene-02 candidates, one subject, gave 344.9°, 57.9°, 355.7°, 306.4°, 2.6°, 7.3°. If tone and chroma are the theme's and hue is the media's (N1), the accent is decided by which candidate is accepted rather than by the scene, and acceptance becomes an unrecorded palette decision. | Maintainer |

---

## 10. What building the first scene taught the pipeline

Written 2026-11-07, after scene-01 went from six boring candidates to a
finished loop. These are the lessons that changed how the work is done, not a
log of what happened. Each one cost something to learn.

### 10.1 The generator is steerable on style and subject, and not on structure

Instruction about *what* and *in what manner* landed on the first attempt:
naming a subject produced the kimono portrait immediately, and naming a
rendering style ended the photoreal default (§4.6). Instruction about *where*
and *how things connect* was ignored over and over — off-centre framing, pins
rooted in the hair rather than floating, a side view rather than a
three-quarter one. Re-prompting did not help, because the seed dominates those
properties.

**So the pipeline is generate-many-and-filter, not prompt-and-refine.** The
useful lever on structure is the number of candidates, and the brief's job for
structure is to describe what to *reject*, which is already why §4.2 says the
negative constraints matter more than the positive ones.

### 10.2 Separate on colour, not on luminance

Three separate times, a luminance threshold silently deleted something: her
hair and obi, then the dark moths, then the skull's beak. nightshade's palette
is a near-black field with near-black objects on it, so **luminance is not a
separable axis in this theme** — and a near-black element that vanishes does
not look like an error, it looks like a slightly different picture, which is
why it survived three rounds.

Hue is separable. The bloom mask is `min(G,B) - R`, which isolates the cyan
disc from a white kimono and pink skin without a hand-drawn mask. Any future
masking step should reach for hue first and justify luminance if it uses it.

### 10.3 Measure the complaint before fixing it

"She still looks overexposed" became: the painting clips 0.066% of its pixels,
the render clipped 14.3%, and 10.4% of the frame was clipped *on her* rather
than on the disc. That number did two things a visual judgement could not —
confirmed the complaint was about the bloom rather than the painting, and said
exactly where to cut. After the fix, clipping away from the disc is 0.085%,
against the painting's own 0.066%.

The general form: a complaint phrased as a feeling usually has a number
underneath it, and finding the number is faster than guessing at the fix.

### 10.4 Any relative criterion is gameable, and two of ours are

R8 asks that the seam between the last frame and the first be no worse than an
ordinary frame step. That is a **ratio**, and a ratio improves when the
denominator gets worse: an all-intra encode "passes" by making every frame
noisier. A4's scrim threshold has the same shape — scrim alone rewards
emptiness, which is why the best-scoring candidate of the first six had no
subject at all (A8).

**A relative criterion needs an absolute floor as its second term.** Neither of
ours has one yet. Until they do, neither should be read as a pass on its own.

### 10.5 A metric proposed for an aesthetic judgement has to be validated first

Two attempts to automate taste produced confidently wrong answers:

* A Laplacian "inkiness" score, meant to tell ink-and-wash from photoreal,
  ranked a *detailed photoreal* skull highest — detail energy is detail
  energy, whatever drew it.
* A scale-and-shift search, meant to optimise the composition gates, returned a
  gate-perfect framing with her head cut off, because dark bands where the UI
  goes are exactly what the gates reward.

Both were caught by looking. **A new metric gets checked against the eye on
known-good and known-bad examples before anything is decided with it** — which
is the same argument §4.6 makes for not automating gate 3 away.

### 10.6 Place and anchor things by their function, not by their geometry

Two bugs with one shape. The pendant was positioned by the centre of its
bounding box, but its beak sticks far out to one side, so the cranium — the
part a pendant actually hangs by — ended up floating beside the chain instead
of under it. And the chain was driven by sliding its anchor, so the top bead
slid against the painted hairpin it hangs from.

**An attachment point is a named feature, not a coordinate**: hang from the
crown, found from the alpha; pin to the pin, and drive the motion with a force
instead. The fix for the second one is also physically truer — air moves a
hairpin charm, the hairpin does not move.

### 10.7 One convention for parallax, in one place

The charm separated from its pin because the composite applied parallax with
the opposite sign to the shader's: the shader moves the *sample*, so the image
moves the other way. It also guessed the charm's depth instead of reading it at
the pin. Both are the same mistake — a second implementation of a rule that
already existed.

**Anything composited over the scene takes its parallax from the same code and
the same depth map as the scene does.**

### 10.8 Verify that an edit applied

Two edits to the renderer silently did not apply, because the text they were
looking for had already been changed by an earlier edit. Neither failed; they
simply did nothing, and the render came back identical. It was caught only when
a later line referenced a variable that did not exist.

**An edit that cannot fail loudly will eventually fail quietly.** Anchor edits
on something that will raise if it is missing.

### 10.9 A tag left unset is a decision, taken by the reader

Every loop this project had made came back from `scene_check` as
`colour primaries / range: unset / unset`, the deployed greeter background
included — and the deployed file was worse than that, because it was a
hand-run H.264 transcode of the HEVC render and carried no tag at all on any
of the four fields.

Undeclared is not neutral. §2 says the import assumes limited range because it
is the safer guess, so an untagged full-range file is silently crushed. The
pixels were always bt709/limited; nothing but the declaration was missing, and
the declaration is the half a reader can actually see.

Two things made it stay missing:

* **ffmpeg's own colour flags are half the fix.** libx26x writes its own VUI
  and `ffprobe` reads that rather than the container tags. With
  `-color_primaries`/`-color_trc`/`-colorspace` alone the HEVC file came back
  `color_space=bt709` but `color_primaries=unknown` — half-tagged, still a
  §2.1 miss. `-x265-params colorprim=…:transfer=…:colormatrix=…:range=…` is
  the other half, and x264 spells the range differently (`range=tv` against
  x265's `range=limited`), so the two cannot share one string.
* **The deployed format was not something the tool could produce.** The
  greeter's measured hardware path is `nvh264dec`, so what ships is H.264,
  while the renderer only emitted HEVC. The gap was closed by hand, once, and
  a hand step carries no contract. `render.py --codec {hevc,h264}` now emits
  both, tagged.

Re-rendered scene-01 on both paths: §2.1 passes, and **matugen returns the
same 215.9° and the same seven colours**, so nothing downstream of the palette
moves. The seam improved from 1.40× to 1.23× (H.264) and 1.13× (HEVC) — the
old file's seam was partly the transcode's.

**A clause that reports "unset" for every artefact is not measuring the
artefacts; it is measuring a missing step.**

### 10.10 A pool and a chain are not the same list

Asked to put a second scene *into rotation* on the login screen rather than
*in place of* the first, the obvious move was to append it to R9's
`background` list. That list already held two paths and took more.

It would never have been seen. R9's list is **ordered by degradation**: the
first entry that works wins, and the ones behind it exist for when it does
not. A second scene appended to a working chain is dead text.

A pool is **ordered by nothing** — its entries are equals, and one is chosen
per launch. Rotating over a chain would promote a still above a video, which
is R9 read backwards. So they are nested rather than merged: the outer list is
the pool, each entry a chain, and R9 is untouched inside a candidate.

Two consequences worth having measured:

* **The choice advances, it is not drawn.** With two candidates a fair coin
  shows the same one twice running half the time, which does not read as
  rotation. Advancing costs one integer remembered across launches, in
  `/var/lib/auroval/last-background` beside R4's login memory — the same
  directory, for C10's reason. The file is advisory: unreadable means "start
  somewhere", and somewhere comes from the clock rather than being fixed at 0,
  because a machine that cannot keep the file would otherwise pin to the first
  candidate on every boot, which is the behaviour rotation exists to avoid.
* **Rotation and the derived palette pull against each other.** Candidates
  have their own hues — 215.9° for scene-01, 69.3° for scene-05, 146° apart —
  and §4.3 derives the UI accent from *the* scene's hue. One static palette
  therefore matches at most one candidate. Measured both ways round: each
  scene clears all four legibility clauses under the *other* scene's palette,
  so this is an aesthetic mismatch and not a safety one. It is also latent
  rather than live, because `greet.lua` currently ships no palette table at
  all and the greeter uses its defaults. It becomes real the moment §4.4's
  packaging reaches the greeter, and the honest shape is then a palette *per
  candidate*, not a second key parallel to the pool — see N9.

**A list whose order means something cannot also be a list whose order means
nothing.**
