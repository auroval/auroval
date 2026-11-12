#!/usr/bin/env python3
"""Prepare a candidate for review at the scale its defects live at.

This exists because of a specific failure. On 2026-11-11 six cabinet
candidates were judged from a contact sheet 700 px wide -- 27% of the 2560 px
master -- and declared clean. The maintainer then found, in the installed
wallpaper, two skulls whose jaws pass in front of the glass that is supposed
to contain them. The evidence for that defect is the jar's rim: a line two or
three pixels wide, passing either in front of a silhouette or behind it. At
27% that line does not exist as pixels. The review could not have found the
defect; it was performed at a scale where the defect cannot be seen.

So this tool does two things, and deliberately refuses to do a third.

It cuts the master into tiles at 1:1, each small enough to be looked at
whole, so that containment is judged against real pixels. And it measures the
border strips, because a sumi-e prompt sometimes returns the painting with a
margin of bare toned paper down one side -- harmless on a greeter, fatal on a
desktop scene, which §3.1 requires to fill the frame edge to edge.

What it does not do is decide whether a specimen is inside its jar. A glass
rim in a brushed painting is a soft, broken, low-contrast line crossing a
texture full of similar lines; a detector for it would cry wolf far more
often than it caught anything, and a check that is usually wrong is worse
than no check, because it trains you to click past it. The judgement stays
with a reader. This tool's whole job is to put the right pixels in front of
that reader.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# A desktop scene fills the frame, so a border strip should be part of the
# painting: dark, because nightshade is a nocturnal theme lit from behind.
# Bare toned paper measures far above this. The threshold is set well clear
# of the darkest real border seen so far (a pale wooden cabinet post at L 70)
# and well below bare paper (L 216).
BORDER_PX = 24
BORDER_MAX_L = 120.0

# And the outermost pixels on their own, because a mean over 24 px hides a
# hairline. Seed 71 of 2026-11-12 carried a pale line across the whole top
# edge: the top two rows measure L 187, the top 24 rows measure 74, and 74
# passes. Averaging buried a real defect exactly the way downscaling buried
# the skulls -- the same mistake in a second instrument. A thin bright line
# at a screen edge is as visible as a thick one, so it gets its own test.
EDGE_PX = 4
EDGE_MAX_L = 140.0


def border_means(img: Image.Image) -> dict[str, tuple[float, float]]:
    """Each edge as (mean over BORDER_PX, mean over the outermost EDGE_PX)."""
    a = np.asarray(img.convert("L"), dtype=float)
    return {
        "left": (float(a[:, :BORDER_PX].mean()), float(a[:, :EDGE_PX].mean())),
        "right": (float(a[:, -BORDER_PX:].mean()), float(a[:, -EDGE_PX:].mean())),
        "top": (float(a[:BORDER_PX, :].mean()), float(a[:EDGE_PX, :].mean())),
        "bottom": (float(a[-BORDER_PX:, :].mean()), float(a[-EDGE_PX:, :].mean())),
    }


def cut_tiles(img: Image.Image, out: Path, stem: str, cols: int, rows: int) -> list[Path]:
    """Cut at 1:1. No resampling happens here, by design -- the point of the
    tile is that every pixel is the pixel the model drew."""
    w, h = img.size
    tw, th = w // cols, h // rows
    paths = []
    for r in range(rows):
        for c in range(cols):
            box = (c * tw, r * th, (c + 1) * tw, (r + 1) * th)
            p = out / f"{stem}-r{r}c{c}.png"
            img.crop(box).save(p)
            paths.append(p)
    return paths


def review(path: Path, out: Path, cols: int, rows: int) -> bool:
    img = Image.open(path).convert("RGB")
    out.mkdir(parents=True, exist_ok=True)
    stem = path.stem

    means = border_means(img)
    worst_band = max(b for b, _ in means.values())
    worst_line = max(e for _, e in means.values())
    ok = worst_band <= BORDER_MAX_L and worst_line <= EDGE_MAX_L

    tiles = cut_tiles(img, out, stem, cols, rows)
    edges = "  ".join(f"{k} {b:.0f}/{e:.0f}" for k, (b, e) in means.items())
    if worst_band > BORDER_MAX_L:
        verdict = f"BLANK EDGE (band {worst_band:.1f} > {BORDER_MAX_L:.0f})"
    elif worst_line > EDGE_MAX_L:
        verdict = f"BRIGHT HAIRLINE (outer {EDGE_PX} px {worst_line:.1f} > {EDGE_MAX_L:.0f})"
    else:
        verdict = "ok"
    print(f"{stem}: {img.size[0]}x{img.size[1]}  border L band/outer: {edges}  -> {verdict}")
    print(f"  {len(tiles)} tiles at {img.size[0] // cols}x{img.size[1] // rows} in {out}")
    return ok


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("paths", nargs="+", type=Path, help="candidate PNGs, or a directory of them")
    ap.add_argument("--out", type=Path, required=True, help="directory to write tiles into")
    ap.add_argument("--cols", type=int, default=3)
    ap.add_argument("--rows", type=int, default=2)
    args = ap.parse_args()

    files: list[Path] = []
    for p in args.paths:
        if p.is_dir():
            files.extend(sorted(q for q in p.glob("*.png") if "master16" not in q.name))
        else:
            files.append(p)
    if not files:
        print("no candidates found", file=sys.stderr)
        return 2

    all_ok = True
    for f in files:
        all_ok &= review(f, args.out, args.cols, args.rows)

    print()
    print("Tiles are 1:1. Read every one. Containment is not measured here and")
    print("cannot be read from a contact sheet -- look for the jar's rim passing")
    print("in front of each specimen, and for anything crossing a paper label,")
    print("which is pasted on the outside of the glass.")
    return 0 if all_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
