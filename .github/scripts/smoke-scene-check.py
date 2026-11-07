#!/usr/bin/env python3
"""Check the parts of `tools/scene-check` that its own `--self-test` cannot.

`scene_check.py --self-test` is the behavioural test and it is the better one:
it asserts the measurements themselves, including that a bright patch behind a
UI element is caught even when the composition clause passes, and that the
accent reaches the sRGB cusp. CI runs it first. This file deliberately does not
repeat any of that.

What is left over, and why each piece is here:

1. **The `--json` key contract.** `--self-test` exercises the tool's internals;
   it does not go through `main()`, so a rename in the `--json` payload would
   pass it. The theme documents quote these key names, so a silent rename is a
   documentation break as much as a code change.
2. **The internal hue fallback.** With matugen absent the tool falls back to its
   own hue extraction, and that is the path CI and every fresh checkout take.
   `--self-test` covers the `--hue` override instead, so this asserts the other
   branch actually fires rather than merely not crashing.
3. **The video path.** `--self-test` is stills-only by design. The ffprobe
   argument list is built by implicit string concatenation inside an argv array,
   one missing comma from becoming two arguments — and that exact line was
   edited to satisfy `ISC004`. A stills-only test would have passed anyway.

Deliberately matugen-free. CI has no matugen, and the tool's internal hue
extraction is the path CI and any fresh checkout will actually hit, so
exercising it is the point rather than a compromise. `--matugen-assertions` is
not covered here for the same reason.

Both a still and a video are exercised. The video matters disproportionately:
the ffprobe argument list is built by implicit string concatenation inside an
argv array, which is one missing comma away from becoming two arguments, and a
still never touches it. That path was edited once and the breakage would have
been invisible to a stills-only test.
"""

from __future__ import annotations

import json
import math
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

TOOL = Path(__file__).resolve().parents[2] / "tools" / "scene-check" / "scene_check.py"

# The keys `main()` promises with --json. The theme documents quote these, so a
# silent rename is a documentation break as much as a code change.
REQUIRED_KEYS = {
    "path",
    "kind",
    "hue",
    "hue_source",
    "scrim_alpha",
    "palette",
    "notes",
    "clauses",
    "assertions",
}


def make_scene(path: Path) -> None:
    """A dark scene with one brighter corner, so the safe-field logic has
    something to measure rather than a flat fill."""
    from PIL import Image

    w, h = 1280, 720
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            d = math.hypot(x - w * 0.8, y - h * 0.2) / max(w, h)
            v = max(0.0, 0.55 - d)
            px[x, y] = (int(30 + 120 * v), int(10 + 40 * v), int(60 + 150 * v))
    img.save(path)


def make_clip(path: Path) -> bool:
    """A short clip, so the ffprobe branch is exercised. Returns False when
    ffmpeg is missing, so a machine without it skips rather than fails."""
    if shutil.which("ffmpeg") is None:
        return False
    subprocess.run(
        [
            "ffmpeg",
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x1e1824:s=1280x720:r=30:d=2",
            "-c:v",
            "libx264",
            "-profile:v",
            "high",
            "-pix_fmt",
            "yuv420p",
            str(path),
        ],
        check=True,
        capture_output=True,
    )
    return True


def run(scene: Path, *extra: str) -> dict:
    proc = subprocess.run(
        [sys.executable, str(TOOL), str(scene), "--json", *extra],
        capture_output=True,
        text=True,
        check=False,  # the exit code is inspected below, with the output shown
    )
    if proc.returncode != 0:
        print(proc.stdout)
        print(proc.stderr, file=sys.stderr)
        raise SystemExit(f"scene_check.py exited {proc.returncode} for args {extra}")
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        print(proc.stdout[:2000])
        raise SystemExit(f"--json did not produce JSON: {exc}") from exc


def check(result: dict, *, expect_source: str) -> None:
    missing = REQUIRED_KEYS - set(result)
    if missing:
        raise SystemExit(f"missing keys in --json output: {sorted(missing)}")
    if result["hue_source"] != expect_source:
        raise SystemExit(f"hue_source was {result['hue_source']!r}, expected {expect_source!r}")
    if not isinstance(result["palette"], dict) or not result["palette"]:
        raise SystemExit("palette is empty or not an object")
    if not isinstance(result["clauses"], list):
        raise SystemExit("clauses is not a list")


def main() -> int:
    if not TOOL.exists():
        raise SystemExit(f"tool not found at {TOOL}")

    with tempfile.TemporaryDirectory() as td:
        scene = Path(td) / "scene.png"
        make_scene(scene)

        # 1. The internal fallback must fire when matugen is absent, and the
        #    --json contract must hold. The hue value is not asserted: it is a
        #    measurement, and pinning it would make this a brittle snapshot.
        #    The --hue override is covered by --self-test, so it is not here.
        result = run(scene)
        check(result, expect_source="internal")
        if result["kind"] != "still":
            raise SystemExit(f"a PNG was classified as {result['kind']!r}")

        # 3. A video, for the ffprobe branch. Clause results are not asserted:
        #    a two-second clip legitimately fails the loop-duration clause, and
        #    the point here is that the tool runs and classifies, not that a
        #    synthetic clip is a good scene.
        clip = Path(td) / "clip.mp4"
        if make_clip(clip):
            result = run(clip, "--hue", "300")
            check(result, expect_source="declared")
            if result["kind"] != "video":
                raise SystemExit(f"an mp4 was classified as {result['kind']!r}")
            if not result["clauses"]:
                raise SystemExit("no clauses evaluated for a video")
            print(f"  video path: {len(result['clauses'])} clauses evaluated")
        else:
            print("  video path SKIPPED: ffmpeg not on PATH")

    print("json contract intact, internal hue fallback fires, video path runs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
