# scene-08 — cabinet

**surface: desktop.** A wallpaper, not a greeter background. It fills the frame
edge to edge and keeps nothing clear, because nothing sits on top of it
(nightshade-assets.md §3.1).

Chosen by the maintainer on 2026-11-10 from six seeds (1, 3, 7, 11, 17, 23), together
with the other two scenes committed that day.

> **This record describes the seed-11 run, and the brief has since changed.**
> On 2026-11-11 the maintainer found that the seed-11 master shows a lamb both
> inside and outside a jar on the middle shelf — its hindquarters within the
> glass, its foreleg across the rim. The brief was the cause: *"standing in
> that liquid the things the cabinet keeps"* was its only containment clause,
> buried mid-sentence where it read as subject rather than structure, and
> *"a two-headed lamb foetus"* was split by the model into two separate lambs.
> `scene-08-cabinet.json` now states containment three times and binds the
> lamb to one jar, and asks for brushed characters on the labels under the new
> `lettering: labels` key. Twelve candidates were generated under the amended
> brief; none has been selected. Everything below — seed, prompt, measured
> hue — still describes the image that shipped, and will be rewritten when a
> replacement is chosen.

## Why this subject

An apothecary specimen cabinet. Not one of §4.5.2's five: proposed 2026-11-10 while generating the other two, on the grounds that it satisfies both clauses a desktop scene inherits (§4.5.1). *Ornamented* is the whole substance of it rather than a layer on top — ranked jars, cobwebs, instruments, shelf after shelf of surface. *Edged* sits in what the jars hold: the form is the traditional cabinet still life and the content is a two-headed foetus, loose teeth, a bored skull.

## The subject phrase

This is the one part of the prompt that is not shared, and therefore the one
part that cannot be reconstructed from nightshade-assets.md (§4.5). It is the
`subject` field of `scene-08-cabinet.json`, reproduced here so the record stands
without the brief:

> the inside of a crowded apothecary cabinet seen head on and close, shelf above shelf filling the frame edge to edge with ranked glass specimen jars and squat stoppered bottles, their glass clouded and ringed with old residue and their blank paper labels peeling, and standing in that liquid the things the cabinet keeps, a two-headed lamb foetus, coiled pale snakes, a jar of loose human teeth, finger bones laid out in rows, a small cracked skull with a hole bored in its crown, brass calipers and bundles of dried herbs and bone-handled instruments hung on the uprights between the shelves, cobweb and dust thick in every corner and on the near edge of the lowest shelf, every jar lit from behind by a hot saturated magenta light, deep pink and violet shading to crimson, so the liquid glows and the specimens and the shelf boards read as silhouette, deep shadow between and behind the jars

## The full prompt, as assembled

`generate.py`'s `assemble()` is `STYLE + subject + SURFACES["desktop"] +
NO_MARKS`. The result, exactly as the model received it:

> Sumi-e ink wash painting with bold calligraphic brushstrokes on toned paper, minimal flat colour accents, visible brush texture, no photographic realism. the inside of a crowded apothecary cabinet seen head on and close, shelf above shelf filling the frame edge to edge with ranked glass specimen jars and squat stoppered bottles, their glass clouded and ringed with old residue and their blank paper labels peeling, and standing in that liquid the things the cabinet keeps, a two-headed lamb foetus, coiled pale snakes, a jar of loose human teeth, finger bones laid out in rows, a small cracked skull with a hole bored in its crown, brass calipers and bundles of dried herbs and bone-handled instruments hung on the uprights between the shelves, cobweb and dust thick in every corner and on the near edge of the lowest shelf, every jar lit from behind by a hot saturated magenta light, deep pink and violet shading to crimson, so the liquid glows and the specimens and the shelf boards read as silhouette, deep shadow between and behind the jars. a dark nocturnal scene that fills the entire frame edge to edge, every part of the image occupied. deep shadow between and behind the forms rather than empty black space. rim light and glow from behind and between the forms, no frontal key light. saturated neon chroma, magenta and cyan and violet. dense detail and texture throughout, foreground as well as background. visible film grain. 16:9. no text, no lettering, no signature, no logo, no watermark.

## Settings

| | |
|---|---|
| model | `black-forest-labs/FLUX.2-klein-4B` |
| revision | `e7b7dc27f91deacad38e78976d1f2b499d76a294` |
| seed | **11** |
| steps | 4 |
| guidance_scale | 1.0 |
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

- 2560x1440, aspect 1.778 — both clear.
- hue **340.7 degrees** from the media via matugen, accent `#fe00c2` at
  **C\* 95.7**, which is the sRGB cusp for that hue at that lightness.
- All five palette assertions hold: surface L\* 12.0, on_surface 13.34:1,
  primary and error both clear of 4.5:1.

The hue lands in the 300-350 degree window §4.5.2 asks new scenes to aim for,
clear of the 180-250 degree trough where §4.3.1 measured that nothing reaches
C\* 50. It widens the spread N9 is open about; see the note below.

## Known gaps

- **N9 is widened, not resolved.** scene-01 sits at 215.9 degrees and scene-05
  at 69.3, and §4.3 gives the UI one accent. This scene adds a third cluster
  near 340-356. What mitigates it slightly is that scene-07 and scene-08 land
  within half a degree of each other, so the three new scenes are two hues, not
  three. The decision is the maintainer's and this file does not take it.
- **The palette is not committed.** §3.2 asks for a `palette.toml` per scene and
  §5.4 explains why re-derivation is not reproducible across pipeline changes.
  The numbers above are a measurement, not that artefact.
- **This still carries lettering, and that breaks §4.1.** `NO_MARKS` ends every
  prompt with *"no text, no lettering, no signature, no logo, no watermark"*,
  and the brief asks for *blank* peeling paper labels. Four of the six seeds —
  1, 7, 11 and 23 — nonetheless returned CJK characters on the jar labels; only
  3 and 17 came back clean. The chosen seed, 11, is one of the four. It was
  chosen with the defect named, which is the project's own rule working as
  intended (tenet 6: the measurement reports, it never refuses) rather than an
  oversight.

  The mechanism is §4.2's, one level up. At guidance_scale 1.0 there is no
  classifier-free guidance and therefore no negative prompt, so *"no lettering"*
  supplies the tokens for lettering and nothing opposes them — and naming a
  *label* at all, even a blank one, supplies them a second time. A letter-free
  cabinet wants the word "label" out of the subject phrase entirely, not a
  stronger negation. That edit is not made here, because it would change the
  image this file is the record of.

- **The media is not in this repository.** Issue #5 packages theme media
  separately. The master, the 8-bit still and the `run.json` this file was
  written from live outside it.
