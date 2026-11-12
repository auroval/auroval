# scene-08 — cabinet

**surface: desktop.** A wallpaper, not a greeter background. It fills the frame
edge to edge and keeps nothing clear, because nothing sits on top of it
(nightshade-assets.md §3.1).

Generated 2026-11-10 and chosen from six seeds (1, 3, 7, 11, 17, 23).
Regenerated 2026-11-11 as seed 37, after the maintainer found a lamb both
inside and outside a jar on the middle shelf of the seed-11 master.
**Regenerated again and reselected 2026-11-12**, seed 79 of a third six
(59, 61, 67, 71, 73, 79), after the maintainer found that the seed-37 master
shows **two skulls on the lowest shelf whose jaws pass in front of the glass**,
with the jar's base line running behind them.

## Why this file was written twice over

Both defects are the same fault, and so was the review that let the second one
through. The entry below is kept because the project's rule is that a record
says what was wrong, not only what is right.

**The brief was the first cause.** Containment was stated once, mid-sentence,
where it read as subject rather than as structure -- A9 in
nightshade-assets.md almost verbatim. The 2026-11-11 pass fixed that for soft
specimens: every lamb and snake since has stayed in its jar. It did not fix it
for hard ones. Skulls and bones are *props*, and the model goes on placing
them like props -- on a shelf board, against the glass, across a rim -- in
both runs. The brief now names them: skulls, bones and loose teeth are sealed
in jars **exactly like the soft specimens**, and no skull or bone rests on a
board or stands in front of the glass. It also states that a paper label is
pasted on the **outside** of the glass with nothing inside the jar crossing in
front of it, because that occlusion is how the defect reads when it is too
small to see as a broken rim.

**The review was the second cause, and the worse one.** The six seed-37
candidates were judged from a contact sheet 700 px wide: 27% of a 2560 px
master. The evidence for this defect is the jar's rim -- a line two or three
pixels wide, passing in front of a silhouette or behind it. At 27% that line
does not exist as pixels, so the check could not have found the defect and
should never have been reported as a pass. `tools/nightshade-gen/review.py`
now cuts a master into tiles at 1:1 and measures the border strips; the
contact sheet is for choosing a composition and nothing else. The tool does
not judge containment and deliberately refuses to -- see its docstring.

The same lesson arrived a second time during the 2026-11-12 review. The
border test averaged 24 px and passed seed 71 at L 74, while the top two rows
of that image measure L 187: a pale hairline across the whole top edge. An
average hid a real defect exactly the way a downscale had. The tool now tests
the outermost 4 px separately.

## What the amended brief cost

Seed 61 of the third run shows the price of the label clause: the model
satisfied "nothing crossing in front of the label" by drawing the specimens
**as illustrations on the labels**, botanical-plate fashion, instead of as
things behind glass. One seed in six, and a reminder that a clause aimed at
depth can be answered by removing the depth.

## Why this subject

An apothecary specimen cabinet. Not one of §4.5.2's five: proposed 2026-11-10 while generating the other two, on the grounds that it satisfies both clauses a desktop scene inherits (§4.5.1). *Ornamented* is the whole substance of it rather than a layer on top — ranked jars, cobwebs, instruments, shelf after shelf of surface. *Edged* sits in what the jars hold: the form is the traditional cabinet still life and the content is a two-headed foetus, loose teeth, a bored skull.

## The subject phrase

This is the one part of the prompt that is not shared, and therefore the one
part that cannot be reconstructed from nightshade-assets.md (§4.5). It is the
`subject` field of `scene-08-cabinet.json`, reproduced here so the record stands
without the brief:

> the inside of a crowded apothecary cabinet seen head on and close, shelf above shelf filling the frame edge to edge with ranked glass specimen jars and squat stoppered bottles, every jar stoppered and sealed and every specimen suspended inside the liquid behind its glass, nothing standing loose on the shelf boards, the skulls and the bones and the loose teeth each sealed inside a jar of their own exactly like the soft specimens, no skull and no bone resting on a shelf board or standing in front of the glass, every jar's mouth and base drawn complete and unbroken so that each specimen sits wholly behind the near wall of its jar, almost every jar carrying a paper label, foxed and water stained and peeling at one corner, each label pasted flat on the outside of the glass and left unobscured with nothing inside the jar crossing in front of it, each label hand brushed with two or three characters in faded black ink in a single vertical column, the names of dried herbs and medicines, their glass clouded and ringed with old residue, and sealed inside those jars the things the cabinet keeps, one two-headed lamb foetus curled inside the tallest jar, coiled pale snakes, a jar of loose human teeth, finger bones laid out in rows, a small cracked skull with a hole bored in its crown, brass calipers and bundles of dried herbs and bone-handled instruments hung on the uprights between the shelves, cobweb and dust thick in every corner and on the near edge of the lowest shelf, every jar lit from behind by a hot saturated magenta light, deep pink and violet shading to crimson, so the liquid glows and the specimens and the shelf boards read as silhouette, deep shadow between and behind the jars

## The full prompt, as assembled

`generate.py`'s `assemble()` is `STYLE + subject + SURFACES["desktop"] +
LETTERING[brief["lettering"]]`, and this brief sets `lettering: labels`, so the
tail clause is the narrowed one. The result, exactly as the model received it:

> Sumi-e ink wash painting with bold calligraphic brushstrokes on toned paper, minimal flat colour accents, visible brush texture, no photographic realism. the inside of a crowded apothecary cabinet seen head on and close, shelf above shelf filling the frame edge to edge with ranked glass specimen jars and squat stoppered bottles, every jar stoppered and sealed and every specimen suspended inside the liquid behind its glass, nothing standing loose on the shelf boards, the skulls and the bones and the loose teeth each sealed inside a jar of their own exactly like the soft specimens, no skull and no bone resting on a shelf board or standing in front of the glass, every jar's mouth and base drawn complete and unbroken so that each specimen sits wholly behind the near wall of its jar, almost every jar carrying a paper label, foxed and water stained and peeling at one corner, each label pasted flat on the outside of the glass and left unobscured with nothing inside the jar crossing in front of it, each label hand brushed with two or three characters in faded black ink in a single vertical column, the names of dried herbs and medicines, their glass clouded and ringed with old residue, and sealed inside those jars the things the cabinet keeps, one two-headed lamb foetus curled inside the tallest jar, coiled pale snakes, a jar of loose human teeth, finger bones laid out in rows, a small cracked skull with a hole bored in its crown, brass calipers and bundles of dried herbs and bone-handled instruments hung on the uprights between the shelves, cobweb and dust thick in every corner and on the near edge of the lowest shelf, every jar lit from behind by a hot saturated magenta light, deep pink and violet shading to crimson, so the liquid glows and the specimens and the shelf boards read as silhouette, deep shadow between and behind the jars. a dark nocturnal scene that fills the entire frame edge to edge, every part of the image occupied. deep shadow between and behind the forms rather than empty black space. rim light and glow from behind and between the forms, no frontal key light. saturated neon chroma, magenta and cyan and violet. dense detail and texture throughout, foreground as well as background. visible film grain. 16:9. no signature, no logo, no watermark, and no writing anywhere in the picture except the brushed characters on the jars' paper labels.

## Settings

| | |
|---|---|
| model | `black-forest-labs/FLUX.2-klein-4B` |
| revision | `e7b7dc27f91deacad38e78976d1f2b499d76a294` |
| seed | **79** |
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
- hue **343.0 degrees** from the media via matugen, accent `#ff04bb` at
  **C\* 93.6**, against an sRGB cusp of 93.8 for that hue at that lightness.
- All five palette assertions hold: surface L\* 12.0, on_surface 13.34:1,
  primary 4.67:1 and error 9.65:1, both clear of 4.5:1.

`review.py <file>` additionally reports the border strips, which for this
master are L 26 (left), 29 (right), 44 (top) and 27 (bottom) over 24 px, and
the same within a pixel over the outermost 4 px. It is the darkest-edged of
the three cabinet masters.

The hue has now been 340.7 (seed 11), 356.6 (seed 37) and 343.0 (seed 79) for
what is nominally the same brief, which is N8 stated as a measurement rather
than an argument.

## Known gaps

- **N9 is widened, not resolved, and this seed narrows it the wrong way.**
  scene-01 sits at 215.9 degrees and scene-05 at 69.3, and §4.3 gives the UI
  one accent. The five installed wallpapers now measure 341.2 (jelly), 343.0
  (this), 355.7 (moth), 13.0 (hollow) and 24.9 (pond). Seed 37 sat beside
  moth at 0.9 degrees; seed 79 sits beside jelly at 1.8. Either way the three
  new scenes are two hues rather than three. The decision is the maintainer's
  and this file does not take it.
- **The palette is not committed.** §3.2 asks for a `palette.toml` per scene and
  §5.4 explains why re-derivation is not reproducible across pipeline changes.
  The numbers above are a measurement, not that artefact.
- **The lettering is asked for, and it cannot be vouched for.** `lettering:
  labels` refuses signature, logo and watermark and permits writing on the
  jars' paper labels only. What it cannot do is make the characters mean
  anything: a 4B distilled model draws glyph-shaped marks, not words. So the
  labels on this master were read at native resolution before it was
  installed, as the seed-37 master's were.

  Most are not characters at all, or are real characters in combinations that
  are not words: 曼円, 田泛, 枚抹, 戾猒. One reads as 杏浸 -- apricot, steeped
  -- which is not a word but is exactly the shape a herbal preparation's name
  would take. **Two labels on the lowest shelf carry single characters with
  religious weight: 神 on a jar of crossed bones, and 佛 (partly hidden behind
  a herb bundle) on its neighbour.** Both are also ordinary components of real
  materia medica names -- 神麹 is a medicated leaven, 佛手柑 is Buddha's-hand
  citron -- so in an apothecary cabinet they are defensible. On a jar of human
  bones they can also read as something else. It was installed with that named
  rather than hidden, which is the rule working as intended; anyone who
  disagrees should reroll the seed, not retouch the master.

  The check does not generalise. It was done by eye, by a reader who is not a
  native one, on one image. A second scene under this key needs its own pass.

- **The media is not in this repository.** Issue #5 packages theme media
  separately. The master, the 8-bit still and the `run.json` this file was
  written from live outside it.
