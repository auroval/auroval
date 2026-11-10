# scene-07 — jelly

**surface: desktop.** A wallpaper, not a greeter background. It fills the frame
edge to edge and keeps nothing clear, because nothing sits on top of it
(nightshade-assets.md §3.1).

Chosen by the maintainer on 2026-11-10 from six seeds (1, 3, 7, 11, 17, 23), together
with the other two scenes committed that day.

## Why this subject

A jellyfish lit from below, proposed in nightshade-assets.md §4.5.2 and marked yes on both surfaces: *ornament is inherent*. §4.5.2 also attaches the one condition this brief had to honour — the hue must be pushed off cyan, because the obvious colour for the subject is the one colour the sRGB gamut cannot saturate (§4.3.1's 180-250 degree trough).

## The subject phrase

This is the one part of the prompt that is not shared, and therefore the one
part that cannot be reconstructed from nightshade-assets.md (§4.5). It is the
`subject` field of `scene-07-jelly.json`, reproduced here so the record stands
without the brief:

> a great jellyfish filling the frame, seen close and from slightly below in black water, its bell translucent and veined with fine branching channels and ringed with crowded frilled lappets, and hanging from it a dense curtain of oral arms and trailing tentacles that falls through the whole lower half of the picture, caught in that curtain and wound about by it the pale picked-clean bones of small fish and a few thin bird skulls and a scatter of bleached vertebrae, a drowned string of prayer beads tangled in the arms, the whole animal lit from below and behind by a hot saturated magenta glow, deep pink and violet shading to crimson, the water around it and the sediment drifting through it tinted that same deep magenta, the light rising through the bell so the channels and the bones read as silhouette, more jellyfish massed in the dark behind it, fine motes and sediment filling the water everywhere to the edges of the frame

## The full prompt, as assembled

`generate.py`'s `assemble()` is `STYLE + subject + SURFACES["desktop"] +
NO_MARKS`. The result, exactly as the model received it:

> Sumi-e ink wash painting with bold calligraphic brushstrokes on toned paper, minimal flat colour accents, visible brush texture, no photographic realism. a great jellyfish filling the frame, seen close and from slightly below in black water, its bell translucent and veined with fine branching channels and ringed with crowded frilled lappets, and hanging from it a dense curtain of oral arms and trailing tentacles that falls through the whole lower half of the picture, caught in that curtain and wound about by it the pale picked-clean bones of small fish and a few thin bird skulls and a scatter of bleached vertebrae, a drowned string of prayer beads tangled in the arms, the whole animal lit from below and behind by a hot saturated magenta glow, deep pink and violet shading to crimson, the water around it and the sediment drifting through it tinted that same deep magenta, the light rising through the bell so the channels and the bones read as silhouette, more jellyfish massed in the dark behind it, fine motes and sediment filling the water everywhere to the edges of the frame. a dark nocturnal scene that fills the entire frame edge to edge, every part of the image occupied. deep shadow between and behind the forms rather than empty black space. rim light and glow from behind and between the forms, no frontal key light. saturated neon chroma, magenta and cyan and violet. dense detail and texture throughout, foreground as well as background. visible film grain. 16:9. no text, no lettering, no signature, no logo, no watermark.

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
- hue **341.2 degrees** from the media via matugen, accent `#fe00c1` at
  **C\* 95.4**, which is the sRGB cusp for that hue at that lightness.
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
- **The media is not in this repository.** Issue #5 packages theme media
  separately. The master, the 8-bit still and the `run.json` this file was
  written from live outside it.
