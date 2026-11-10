# scene-06 — moth

**surface: desktop.** A wallpaper, not a greeter background. It fills the frame
edge to edge and keeps nothing clear, because nothing sits on top of it
(nightshade-assets.md §3.1).

Chosen by the maintainer on 2026-11-10 from six seeds (1, 3, 7, 11, 17, 23), together
with the other two scenes committed that day.

## Why this subject

A moth at a lamp, proposed in nightshade-assets.md §4.5.2 and marked desktop-yes there: it fails *singular* as a greeter pair, but as a filled nocturnal frame with the lamp as its own light source it is one of the two strongest candidates that section lists.

## The subject phrase

This is the one part of the prompt that is not shared, and therefore the one
part that cannot be reconstructed from nightshade-assets.md (§4.5). It is the
`subject` field of `scene-06-moth.json`, reproduced here so the record stands
without the brief:

> a huge moth clinging to the cracked glass housing of an old iron street lamp, seen close so the lamp and the moth fill the whole frame, with the mass of a dead hedge and dry seed heads and tangled bare twigs crowding in on every side and into the near foreground, the moth's wings spread wide and worked all over with fine scaled patterning, and set into that patterning not eyespots but small pale human skulls and rows of tiny teeth and a scatter of thin bones, the lamp's broken head filled with a hot saturated magenta light, deep pink and violet shading to crimson, by far the brightest thing in the picture, that light pouring out from behind the moth and throwing its wings and the twigs and dozens of smaller moths behind it into silhouette, a drift of torn wings and dry insect husks heaped on the lamp's rim and scattered across the twigs below

## The full prompt, as assembled

`generate.py`'s `assemble()` is `STYLE + subject + SURFACES["desktop"] +
NO_MARKS`. The result, exactly as the model received it:

> Sumi-e ink wash painting with bold calligraphic brushstrokes on toned paper, minimal flat colour accents, visible brush texture, no photographic realism. a huge moth clinging to the cracked glass housing of an old iron street lamp, seen close so the lamp and the moth fill the whole frame, with the mass of a dead hedge and dry seed heads and tangled bare twigs crowding in on every side and into the near foreground, the moth's wings spread wide and worked all over with fine scaled patterning, and set into that patterning not eyespots but small pale human skulls and rows of tiny teeth and a scatter of thin bones, the lamp's broken head filled with a hot saturated magenta light, deep pink and violet shading to crimson, by far the brightest thing in the picture, that light pouring out from behind the moth and throwing its wings and the twigs and dozens of smaller moths behind it into silhouette, a drift of torn wings and dry insect husks heaped on the lamp's rim and scattered across the twigs below. a dark nocturnal scene that fills the entire frame edge to edge, every part of the image occupied. deep shadow between and behind the forms rather than empty black space. rim light and glow from behind and between the forms, no frontal key light. saturated neon chroma, magenta and cyan and violet. dense detail and texture throughout, foreground as well as background. visible film grain. 16:9. no text, no lettering, no signature, no logo, no watermark.

## Settings

| | |
|---|---|
| model | `black-forest-labs/FLUX.2-klein-4B` |
| revision | `e7b7dc27f91deacad38e78976d1f2b499d76a294` |
| seed | **3** |
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
- hue **355.7 degrees** from the media via matugen, accent `#ff1494` at
  **C\* 84.9**, which is the sRGB cusp for that hue at that lightness.
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
- **The toned paper's torn edge renders inside the frame.** The style clause
  (§4.1.1) asks for *"toned paper"*, and on several moth seeds — this one
  included — the model draws the sheet's deckle edge as a pale border along the
  left and right of the picture rather than treating the paper as texture. On a
  wallpaper that reads as a frame around the image. It is confined to this
  subject: the jellyfish seeds, whose subject matter reaches the edges on all
  sides, never leave the paper anywhere to show. Not a contract clause, so
  `scene_check.py` does not see it; recorded because the next lamp-like subject
  will hit it again.

- **The media is not in this repository.** Issue #5 packages theme media
  separately. The master, the 8-bit still and the `run.json` this file was
  written from live outside it.
