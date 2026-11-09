#!/usr/bin/env python3
"""nightshade-pack — derive a scene's palette once, so nothing derives it twice.

nightshade.md §4.4: *"the palette is derived once per scene, from one
canonical frame, when the theme is packaged. It ships with the scene."*

That has been the design since the theme was written and nothing implemented
it. What runs today is `awww-wallpaper.sh`, which invokes matugen on every
wallpaper change, at runtime, on whichever file the rotation reached. Three
separate findings on 2026-11-08 all pointed here:

  §5.5   matugen's hue is not predictable from the image -- not by area, not
         by chroma. `pond-warm-b-s1` returned 75.2 deg while its own orange
         was both the largest region and among the most saturated. It is
         deterministic per file and opaque from outside, so the only safe use
         is to look at the result once. Recomputing it per rotation means
         nobody ever does.
  N8     the theme's own scenes hold a hue band. Settled by withdrawing a
         scene after it was generated, measured, installed and lived with --
         four steps to discover a number §5.4 would have let it declare. The
         band belongs at packaging time, where `--hue` is one flag.
  #63    wallpapers have no install step at all.

So: one frame, one derivation, written down.

Usage:
    python3 tools/nightshade-pack/pack.py MEDIA OUTDIR --surface desktop
    python3 tools/nightshade-pack/pack.py MEDIA OUTDIR --surface greeter --hue 24.9

Writes into OUTDIR:
    palette.toml   the canonical record: hue and where it came from, the
                   seven roles with their L*/C*, the scrim, and provenance
    colors.lua     the seven roles as bare 6-digit hex, the shape
                   ~/.config/hypr/lua/colors.lua already has, so the existing
                   consumer reads it with no change

`--hue` is N8's mechanism. A declared hue skips extraction entirely, which is
the whole point: a scene whose hue matugen reads wrongly, or which sits in the
180-250 deg trough where no accent exceeds C* 50, says what it is instead of
being rejected after the fact.

**This tool computes nothing of its own.** Every number comes from
`scene_check.py`, which is the measurement the theme documents quote. Two
implementations of one palette is the bug this project keeps finding -- a
greeter that drew one thing and believed another, twice -- and a packaging
tool that re-derived the palette "more conveniently" would be the next one.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
import tempfile
from datetime import UTC, datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scene-check"))
import colourmath as C
import scene_check as S

ROLES = ("primary", "secondary", "tertiary", "surface", "on_surface", "outline", "error")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def tool_version() -> str:
    """The commit this was packaged at, so a palette can be traced to code.

    Returns "unknown" outside a checkout rather than failing: packaging a
    scene from a tarball is a reasonable thing to do and this is provenance,
    not a gate.
    """
    try:
        out = subprocess.run(
            ["git", "-C", str(Path(__file__).resolve().parent), "rev-parse", "HEAD"],
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        )
        return out.stdout.strip() or "unknown"
    except (OSError, subprocess.SubprocessError):
        return "unknown"


def toml_escape(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')


def render_toml(res, media: Path, declared: float | None) -> str:
    p = res.palette
    lines = [
        "# nightshade scene palette — DERIVED ONCE, AT PACKAGING TIME.",
        "#",
        "# nightshade.md §4.4. Do not regenerate this on load, and do not let a",
        "# surface derive its own: three surfaces that each extract from their own",
        '# file agree approximately, and "approximately the same accent colour" is',
        "# visibly worse than two obviously different ones.",
        "#",
        f"# Written by tools/nightshade-pack/pack.py at {datetime.now(UTC).isoformat(timespec='seconds')}.",
        "",
        "[scene]",
        f'media = "{toml_escape(media.name)}"',
        f'sha256 = "{sha256(media)}"',
        f'kind = "{res.kind}"',
        f'surface = "{res.surface}"',
        "",
        "[hue]",
        f"degrees = {res.hue:.4f}",
        f'source = "{res.hue_source}"',
    ]
    if declared is not None:
        lines += [
            "# Declared rather than extracted (§5.4, N8). matugen's hue is not",
            "# predictable from the image (§5.5), so a scene is allowed to say what",
            "# it is instead of being rejected after the fact.",
        ]
    cusp = S.max_chroma_at(p["primary"]["L"], res.hue)
    lines += [
        f"# sRGB cusp at this hue and lightness: C* {cusp:.1f}.",
        "",
        "# Bare 6-digit hex, no leading '#', so callers append their own alpha byte.",
        "[colors]",
    ]
    for r in ROLES:
        lines.append(f'{r} = "{p[r]["hex"]}"')
    lines += [
        "",
        "# L* and C* are recorded so a palette can be checked without re-deriving",
        "# it. `error` carries no C_used from the derivation because it is fixed by",
        "# the theme and does not track the media -- a failure message must not",
        "# change colour with the wallpaper -- so its chroma is read back from its",
        "# own hex and marked.",
        "[tone]",
    ]
    for r in ROLES:
        c = p[r]["C_used"]
        if c is None:
            c = C.rgb_to_lch(C.hex_to_rgb(p[r]["hex"]))[1]
            lines.append(f"{r} = {{ L = {p[r]['L']:.1f}, C = {c:.1f}, fixed = true }}")
        else:
            lines.append(f"{r} = {{ L = {p[r]['L']:.1f}, C = {c:.1f} }}")

    lines += ["", "[scrim]"]
    if res.surface == "greeter":
        lines += [
            "# §2.4. Measured on the same frame as the palette, at the same moment,",
            "# which is the point of doing both here.",
            f"alpha = {res.scrim_alpha:.4f}",
        ]
        for name, (lum, need) in res.regions.items():
            key = name.replace(" ", "_")
            lines.append(f"{key} = {{ background = {lum:.4f}, needs = {need:.4f} }}")
    else:
        lines += [
            "# A wallpaper has no UI on it, so there is nothing to be legible",
            "# against and no scrim to need.",
            "alpha = 0.0",
        ]

    lines += [
        "",
        "[provenance]",
        f'packed_with = "{toml_escape(tool_version())}"',
        f"clauses_met = {str(all(c.met for c in res.clauses)).lower()}",
        f"assertions_met = {str(all(c.met for c in res.assertions)).lower()}",
    ]
    if res.notes:
        lines.append("notes = [")
        for n in res.notes:
            lines.append(f'  "{toml_escape(n)}",')
        lines.append("]")
    return "\n".join(lines) + "\n"


def render_lua(res) -> str:
    p = res.palette
    body = "\n".join(f'    {r:<10} = "{p[r]["hex"]}",' for r in ROLES)
    return (
        "-- nightshade scene palette, derived once at packaging time and shipped\n"
        "-- with the scene (nightshade.md §4.4). This replaces the file matugen\n"
        "-- used to write on every wallpaper change; the shape is identical, so\n"
        "-- the consumer needs no change.\n"
        "--\n"
        "-- Bare 6-digit hex, no leading '#', so callers can append their own\n"
        "-- alpha byte: that is the shape Hyprland's rgba() wants.\n"
        f"-- hue {res.hue:.1f} deg, {res.hue_source}.\n"
        "return {\n" + body + "\n}\n"
    )


def self_test() -> int:
    """The properties that make this tool worth having, asserted.

    Written because the tool's whole claim is that one derivation is rendered
    twice and the two agree. Nothing enforces that but a test: the TOML and
    the Lua are produced by separate functions from the same Result, and a
    later edit to one is exactly the kind of change that silently desyncs
    them. That is the bug class this project keeps finding -- two lists of
    what is true, drifting apart.
    """
    import tomllib

    failures: list[str] = []

    def check(label: str, ok: bool, detail: str = "") -> None:
        print(f"   {'✓' if ok else '✗'} {label}" + (f"  {detail}" if detail else ""))
        if not ok:
            failures.append(label)

    print("\n  self-test\n")
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        # A frame with a bright patch sitting on the date line, so the greeter
        # surface has a scrim to record and the desktop surface has none.
        from PIL import Image, ImageDraw

        W, H = 1280, 720
        im = Image.new("RGB", (W, H), (10, 4, 12))
        d = ImageDraw.Draw(im)
        d.ellipse(
            [W // 2 - 60, H // 2 - 130 - 60, W // 2 + 60, H // 2 - 130 + 60], fill=(250, 250, 255)
        )
        media = tmp / "scene.png"
        im.save(media)

        out = tmp / "greeter"
        res = S.evaluate(media, tmp, hue_override=320.0, surface="greeter")
        out.mkdir()
        (out / "palette.toml").write_text(render_toml(res, media, 320.0))
        (out / "colors.lua").write_text(render_lua(res))

        doc = tomllib.loads((out / "palette.toml").read_text())
        lua = (out / "colors.lua").read_text()

        check("toml parses", True, f"sections {', '.join(doc)}")
        check("seven roles", set(doc["colors"]) == set(ROLES), f"{len(doc['colors'])} roles")
        check(
            "hex is bare and six digits",
            all(len(v) == 6 and not v.startswith("#") for v in doc["colors"].values()),
        )
        # The one that matters: two renderings of one derivation.
        agree = all(f'"{doc["colors"][r]}"' in lua for r in ROLES)
        check("lua and toml carry the same colours", agree)
        check(
            "lua has exactly the seven keys the consumer expects",
            all(f"{r:<10} =" in lua for r in ROLES) and lua.count(" = ") == len(ROLES),
        )
        check(
            "a declared hue is honoured and marked",
            doc["hue"]["source"] == "declared" and abs(doc["hue"]["degrees"] - 320.0) < 1e-6,
            f"{doc['hue']['degrees']:.1f}° {doc['hue']['source']}",
        )
        check(
            "error is marked fixed, because it does not track the media",
            doc["tone"]["error"].get("fixed") is True,
        )
        check(
            "the greeter surface records a scrim and where it came from",
            doc["scrim"]["alpha"] > 0 and "date" in doc["scrim"],
            f"alpha {doc['scrim']['alpha']:.2f}",
        )
        check("provenance names the media by hash", len(doc["scene"]["sha256"]) == 64)

        # Same pixels, other surface.
        out2 = tmp / "desktop"
        res2 = S.evaluate(media, tmp, hue_override=320.0, surface="desktop")
        out2.mkdir()
        (out2 / "palette.toml").write_text(render_toml(res2, media, 320.0))
        (out2 / "colors.lua").write_text(render_lua(res2))
        doc2 = tomllib.loads((out2 / "palette.toml").read_text())

        check("the same pixels as a wallpaper need no scrim", doc2["scrim"]["alpha"] == 0.0)
        check(
            "and the palette is identical across surfaces",
            doc["colors"] == doc2["colors"],
            "surface changes what is measured, never what colour it is",
        )

    print(f"\n  {len(failures)} failed\n" if failures else "\n  all passed\n")
    return 1 if failures else 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("media", type=Path)
    ap.add_argument("outdir", type=Path)
    ap.add_argument("--surface", choices=("greeter", "desktop"), required=True)
    ap.add_argument("--self-test", action="store_true", help="assert the tool's own guarantees")
    ap.add_argument(
        "--hue",
        type=float,
        help="declare the hue in degrees instead of extracting it (§5.4, N8)",
    )
    a = ap.parse_args()

    if not a.media.exists():
        sys.exit(f"{a.media}: no such file")

    with tempfile.TemporaryDirectory() as td:
        res = S.evaluate(a.media, Path(td), hue_override=a.hue, surface=a.surface)

    a.outdir.mkdir(parents=True, exist_ok=True)
    (a.outdir / "palette.toml").write_text(render_toml(res, a.media, a.hue))
    (a.outdir / "colors.lua").write_text(render_lua(res))

    print(f"  scene     {a.media.name}  ({res.kind}, {res.surface})")
    print(f"  hue       {res.hue:.1f}°  {res.hue_source}")
    print(
        f"  primary   #{res.palette['primary']['hex']}  "
        f"L* {res.palette['primary']['L']:.1f}  C* {res.palette['primary']['C_used']:.1f}"
    )
    if res.surface == "greeter":
        print(f"  scrim     {res.scrim_alpha * 100:.0f}%")
    for c in res.clauses:
        if not c.met:
            print(f"  miss      {c.ref} {c.name}: {c.measured}")
    print(f"  wrote     {a.outdir / 'palette.toml'}")
    print(f"            {a.outdir / 'colors.lua'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
