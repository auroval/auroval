# B1 — what colour is auroval itself?

This is the *brand's* hue, not one product's: it paints auroval.org, which
introduces both Auroval Shell and Auroval Remote. See the
[README](../../README.md#the-name) for which name means what.

Status: **decided — one fixed hue, 330°.** Chosen by the maintainer on
2026-11-20, which is option B below. The question is new: it is not in the PRD
and not in [`nightshade.md`](../themes/nightshade.md) §8, because it only
appears once the project has a web presence. `auroval.org` was registered on
2026-11-19 and needs a colour the day it serves a page.

## Why this is a question at all

[`nightshade.md`](../themes/nightshade.md) §4.3 settles how the *product* gets
its colour, and settles it in the strongest possible way:

> The media supplies hue. nightshade supplies everything else.

That rule is the theme's best idea. It is also unusable for a brand. A logo that
changes colour with the wallpaper is not a logo, and a website cannot derive its
accent from media it does not have. The two surfaces have opposite requirements:
the desktop must follow whatever is behind it, and the brand must not follow
anything.

So the question is not *which* colour — it is whether the brand is allowed to
have one.

## The options

### A. The brand follows the scenes

Rotate the site's accent over the shipped scene palettes, the way the greeter
rotates its background.

- **For** Perfectly consistent with §4.3, and demonstrates the theme's central
  rule on the one page where people meet the project. Nothing to decide and
  nothing to maintain: the site consumes `palette.toml` like every other
  consumer.
- **Against** The brand becomes unrecognisable by construction. Measured over
  the shipped scenes the accent swings from C\* 38.4 at 216° to C\* 110 at 330°,
  which is not a family of colours but a cyan and a magenta. It also makes every
  screenshot, social card and favicon a moving target, and those are exactly the
  artefacts that must stay identical across years to be recognised.

### B. One fixed hue, chosen once

The site, the wordmark and the documentation carry a single hue. The theme's
derivation rule is unchanged and continues to govern the product.

- **For** A brand that is stable is the entire point of a brand. The two layers
  stay honestly separate: the fixed hue is *chrome*, the derived hue is
  *content*, and nothing pretends otherwise. The token machinery is reused
  rather than duplicated — `build_palette(330)` is the same function the theme
  calls, so the site's surface, text, outline and error roles come out of the
  project's own code with its own contrast floors already applied.
- **Against** The site shows a colour the product may never show. Someone who
  installs Auroval Shell after seeing a magenta page gets whatever their
  wallpaper gives them, which is usually not magenta. That gap is real and
  permanent.

### C. No accent — neutral chrome only

Near-black ground, L\* 92 text, no brand colour at all; let the scene imagery
carry every bit of colour on the page.

- **For** Cannot misrepresent the product, because it makes no colour claim.
  Also the cheapest thing to keep consistent.
- **Against** The scene media is not in the repository — only the generation
  prompts under `tools/nightshade-gen/` — so today this option is not "let the
  imagery carry it", it is "no colour". And a project whose thesis is about
  colour deriving from media cannot introduce itself in greyscale.

## Why 330°, and not some other fixed hue

Once B is chosen the hue is a measurement rather than a preference, because
`tools/scene-check/scene_check.py` already defines what a good accent is: the
most chromatic colour inside L\* 50–70 that still clears 4.5:1 against the
theme's own surface, capped at C\* 110.

Running `build_palette()` across the circle gives a curve with one flat top.
`ACCENT_C_CAP` — the theme's own ceiling, not the sRGB boundary — binds over
**322.3° to 330.8°**, and nowhere else. Every hue in that 8.5° band reaches
C\* 110; the question is which of them does it with room to spare.

| Hue | Primary | L\* | C\* | On surface |
|---|---|---|---|---|
| 318° | `c748ff` | 56.0 | 102.0 | 4.52:1 |
| 322° | `da32ff` | 56.5 | 109.3 | 4.57:1 |
| 324° | `e024f9` | 56.0 | 110.0 | 4.54:1 |
| 328° | `ee01ef` | 56.5 | 110.0 | 4.60:1 |
| **330°** | **`fb07f1`** | **59.0** | **110.0** | **5.00:1** |
| 331° | `ff04ef` | 59.5 | 109.6 | 5.09:1 |
| 333° | `ff06e4` | 59.0 | 105.9 | 5.00:1 |

At 330° the solver can hold C\* 110 at L\* 59.0 rather than 56.0, and the extra
three points of lightness are worth half a point of contrast. It is the only
hue that is simultaneously at the chroma ceiling and half a point clear of the
floor; its neighbours inside the band sit at 4.52–4.60:1, which clears 4.5:1
only on the strength of the third decimal. **330° is the last hue at the cap
and the first with margin.** One degree further and the cap stops binding.

That margin is not decoration. It is what lets the accent be used for link text
and small interface labels at the 4.5:1 floor rather than only for large marks.

## What this does *not* change

- **§4.3 stands untouched.** The theme still derives hue from media, and
  [tenet 6](../tenets.md) — a person's own video wins — is not weakened by the
  website having a colour.
- **N8 and N9 stay open.** They are about how *scenes* relate to each other and
  to the greeter's palette. A brand hue answers neither, and must not be used to
  close them: picking 330° for the site is not an argument that nightshade's
  scenes should move toward 330°.
- **`error` remains fixed at `ffb4ab`** in both layers, for the same reason it
  is fixed in the theme: a failure message must not change colour with its
  surroundings.

---

## The decision, and what it cost

**One fixed hue, 330°** — option B. Recorded 2026-11-20.

The maintainer answered "één vaste hue", then "330° magenta", in that order —
the structure first and the number second, which is the right order and is why
this record separates them.

What it cost, stated plainly rather than buried:

- **The site is 114° from `scene-01`.** That scene sits at 215.9° with a cusp
  chroma of C\* 38.4 — the floor of the trough that runs 180°–250°. A visitor
  who meets the project in magenta and installs it over a cyan scene sees two
  different-looking things, and both are correct. The published landing page
  answers this by *showing* the curve rather than hiding it: the gamut figure
  plots all 120 hues with their derived accents, so the magenta is visibly one
  point on a rule rather than a claim about what the desktop looks like.
- **It is 43° from nightshade's own rotation.** N8 records the shipped scenes
  settling into a band 11.9° wide around 13.0°–24.9°, whose cusp chroma is
  C\* 79–83. So the brand is adjacent to the theme's own arc rather than
  opposite it — near enough to read as related, far enough to be its own thing.
  That is a comfortable accident, not a reason, and it should not be promoted
  into one later.
- **A second palette layer now exists and can drift.** The site's tokens are
  generated by the same `build_palette()` call, but nothing in CI re-derives
  them, so if `ACCENT_C_CAP`, the lightness band or the contrast floor ever
  change, the website keeps the old hexes silently. The honest mitigation is
  that the site should regenerate its tokens from `scene_check` at build time
  rather than carry them as literals. Until it does, this is known debt.

### The tokens, for reference

Derived once from `build_palette(330)`, 2026-11-20:

| Role | Hex | CIE LCh | On surface |
|---|---|---|---|
| `surface` | `251d24` | lch(12% 6 329) | — |
| `on_surface` | `f2e4f0` | lch(92% 8 329) | 13.38:1 |
| `primary` | `fb07f1` | lch(59% 110 330) | 5.00:1 |
| `secondary` | `d481cb` | lch(65% 50 330) | 6.10:1 |
| `tertiary` | `f66d66` | lch(63% 60 30) | 5.70:1 |
| `outline` | `958892` | lch(58% 8 332) | 4.86:1 |
| `error` | `ffb4ab` | lch(80% 30 31) | 9.66:1 |

As everywhere else in the project, hierarchy on these tokens comes from size and
weight and never from lightness (§4.6): on a near-black ground, every way of
making text quieter moves it toward the background.
