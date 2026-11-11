# scene-08 — cabinet

**surface: desktop.** A wallpaper, not a greeter background. It fills the frame
edge to edge and keeps nothing clear, because nothing sits on top of it
(nightshade-assets.md §3.1).

Generated 2026-11-10 and chosen from six seeds (1, 3, 7, 11, 17, 23), together
with the other two scenes committed that day. **Regenerated and reselected
2026-11-11**, seed 37 of a second six (31, 37, 41, 43, 47, 53), after the
maintainer found that the seed-11 master shows a lamb both inside and outside
a jar on the middle shelf -- hindquarters in the glass, foreleg across the
rim.

The brief caused that, in two ways, and both were fixed in the brief rather
than in the image. Containment was stated once, mid-sentence -- *"standing in
that liquid the things the cabinet keeps"* -- inside a list, where it read as
subject and not as structure. That is A9 in nightshade-assets.md almost
verbatim. And *"a two-headed lamb foetus"* was split by the model into two
separate lambs, one of which it had nowhere to put but the shelf board. The
brief now states containment three times, in three places, and binds the lamb
to a single jar. All six seeds of the second run put every specimen behind
glass -- including seed 41, which again drew two lambs, both of them in jars.

## Why this subject

An apothecary specimen cabinet. Not one of §4.5.2's five: proposed 2026-11-10 while generating the other two, on the grounds that it satisfies both clauses a desktop scene inherits (§4.5.1). *Ornamented* is the whole substance of it rather than a layer on top — ranked jars, cobwebs, instruments, shelf after shelf of surface. *Edged* sits in what the jars hold: the form is the traditional cabinet still life and the content is a two-headed foetus, loose teeth, a bored skull.

## The subject phrase

This is the one part of the prompt that is not shared, and therefore the one
part that cannot be reconstructed from nightshade-assets.md (§4.5). It is the
`subject` field of `scene-08-cabinet.json`, reproduced here so the record stands
without the brief:

> the inside of a crowded apothecary cabinet seen head on and close, shelf above shelf filling the frame edge to edge with ranked glass specimen jars and squat stoppered bottles, every jar stoppered and sealed and every specimen suspended inside the liquid behind its glass, nothing standing loose on the shelf boards, almost every jar carrying a paper label, foxed and water stained and peeling at one corner, each label hand brushed with two or three characters in faded black ink in a single vertical column, the names of dried herbs and medicines, their glass clouded and ringed with old residue, and sealed inside those jars the things the cabinet keeps, one two-headed lamb foetus curled inside the tallest jar, coiled pale snakes, a jar of loose human teeth, finger bones laid out in rows, a small cracked skull with a hole bored in its crown, brass calipers and bundles of dried herbs and bone-handled instruments hung on the uprights between the shelves, cobweb and dust thick in every corner and on the near edge of the lowest shelf, every jar lit from behind by a hot saturated magenta light, deep pink and violet shading to crimson, so the liquid glows and the specimens and the shelf boards read as silhouette, deep shadow between and behind the jars

## The full prompt, as assembled

`generate.py`'s `assemble()` is `STYLE + subject + SURFACES["desktop"] +
LETTERING[brief["lettering"]]`, and this brief sets `lettering: labels`, so the
tail clause is the narrowed one. The result, exactly as the model received it:

> Sumi-e ink wash painting with bold calligraphic brushstrokes on toned paper, minimal flat colour accents, visible brush texture, no photographic realism. the inside of a crowded apothecary cabinet seen head on and close, shelf above shelf filling the frame edge to edge with ranked glass specimen jars and squat stoppered bottles, every jar stoppered and sealed and every specimen suspended inside the liquid behind its glass, nothing standing loose on the shelf boards, almost every jar carrying a paper label, foxed and water stained and peeling at one corner, each label hand brushed with two or three characters in faded black ink in a single vertical column, the names of dried herbs and medicines, their glass clouded and ringed with old residue, and sealed inside those jars the things the cabinet keeps, one two-headed lamb foetus curled inside the tallest jar, coiled pale snakes, a jar of loose human teeth, finger bones laid out in rows, a small cracked skull with a hole bored in its crown, brass calipers and bundles of dried herbs and bone-handled instruments hung on the uprights between the shelves, cobweb and dust thick in every corner and on the near edge of the lowest shelf, every jar lit from behind by a hot saturated magenta light, deep pink and violet shading to crimson, so the liquid glows and the specimens and the shelf boards read as silhouette, deep shadow between and behind the jars. a dark nocturnal scene that fills the entire frame edge to edge, every part of the image occupied. deep shadow between and behind the forms rather than empty black space. rim light and glow from behind and between the forms, no frontal key light. saturated neon chroma, magenta and cyan and violet. dense detail and texture throughout, foreground as well as background. visible film grain. 16:9. no signature, no logo, no watermark, and no writing anywhere in the picture except the brushed characters on the jars' paper labels.

## Settings

| | |
|---|---|
| model | `black-forest-labs/FLUX.2-klein-4B` |
| revision | `e7b7dc27f91deacad38e78976d1f2b499d76a294` |
| seed | **37** |
| steps | 4 |
| guidance_scale | 1.0 |
| lettering | `labels` |
| size | 2560x1440 |

## Versions, as resolved at generation time

| package | version |
|---|---|
| diffusers | `0.42.1` |
| transformers | `5.22.2` |
| accelerate | `1.15.0` |
| torch | `2.15.0+cu132` |
| cuda | `13.2` |
| device | `NVIDIA GeForce RTX 4070 Ti` |
| numpy | `2.5.4` |
| opencv | `5.0.0` |
| pillow | `13.0.0` |

`diffusers` and `transformers` are pinned in `requirements.txt` and recorded
here anyway: a pin says what should be installed, this says what was. `torch`
and `accelerate` are deliberately unpinned, and this table is the only place
their versions exist (requirements.txt explains why).

## Grading recipe

None. The master is the pipeline's output, ungraded. §4.4's phase variants —
dawn / day / dusk / night by exposure, rim-light hue shift and black level —
have not been produced for this scene; when they are, the recipe belongs here.

## Measured

`scene_check.py <file> --surface desktop`, which for a desktop scene exercises
geometry and palette only (§3.1):

- 2560x1440, aspect 1.778 -- both clear.
- hue **356.6 degrees** from the media via matugen, accent `#ff1692` at
  **C\* 84.4**, which is the sRGB cusp for that hue at that lightness.
- All five palette assertions hold: surface L\* 12.0, on_surface 13.34:1,
  primary 4.51:1 and error 9.64:1, both clear of 4.5:1.

Seed 11 measured 340.7 degrees, so the replacement moves the scene about 16
degrees. It does not widen the rotation: measured together, the five installed
wallpapers run 341.2 (jelly), 355.7 (moth), 356.6 (this), 13.0 (hollow) and
24.9 (pond). Dropping 340.7 for 356.6 leaves the span half a degree narrower
than before and puts this scene beside moth rather than beside jelly.

## Known gaps

- **N9 is widened, not resolved.** scene-01 sits at 215.9 degrees and scene-05
  at 69.3, and §4.3 gives the UI one accent. This scene adds a third cluster
  near 340-356. Seed 37 moves this scene from 340.7 to 356.6, which pairs it
  with scene-06 (355.7) instead of scene-07 (341.2) -- the three new scenes
  are still two hues rather than three, just a different two. The decision is
  the maintainer's and this file does not take it.
- **The palette is not committed.** §3.2 asks for a `palette.toml` per scene and
  §5.4 explains why re-derivation is not reproducible across pipeline changes.
  The numbers above are a measurement, not that artefact.
- **The lettering is now asked for, and it cannot be read.** This file used to
  record the opposite: the brief asked for *blank* labels, `NO_MARKS` forbade
  lettering, and four of the six first-run seeds returned CJK characters
  anyway. The maintainer liked them. So `NO_MARKS` was split rather than
  fought -- see `generate.py` -- and this brief sets `lettering: labels`,
  which still refuses signature, logo and watermark and permits writing on
  the jars' paper labels only.

  What that cannot do is make the characters mean anything. A 4B distilled
  model draws glyph-shaped marks, so the labels on this master were read
  before it was installed rather than assumed. Most are not characters at
  all: brush forms carrying the grass radical, which is the right instinct
  for a herb name and not a word. A few are real and ordinary -- the medicine
  character among them. **One label, lowest shelf, right of centre, can be
  read as the first two characters of the word for hell**, though the second
  glyph is not a clean one and the third is not a character, so it does not
  form the word. It was installed with that named, which is this project's
  rule working as intended rather than an oversight; nightshade is a theme
  whose §4.5 asks scenes to be *edged*, and a hell jar in an apothecary
  cabinet is inside that. Anyone who disagrees should reroll the seed, not
  retouch the master.

  The check does not generalise. It was done by eye, by a reader who is not a
  native one, on one image. A second scene under this key needs its own pass.

- **The media is not in this repository.** Issue #5 packages theme media
  separately. The master, the 8-bit still and the `run.json` this file was
  written from live outside it.
