#!/usr/bin/env python3
"""scene-check — measure a nightshade scene candidate and render it for judging.

nightshade refuses nothing (tenet 6). This tool therefore reports; it does not
veto. Every clause of the media contract is measured, and a clause that is not
met names the degradation that applies instead.

See docs/themes/nightshade.md. Clause numbers below follow that document.

Usage (nushell):
    python3 tools/scene-check/scene_check.py scene.png
    python3 tools/scene-check/scene_check.py loop.mp4 --preview /tmp/out.png
    python3 tools/scene-check/scene_check.py scene.png --json
    python3 tools/scene-check/scene_check.py --matugen-assertions
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import colour as C  # noqa: E402

# --------------------------------------------------------------------------
# Contract constants (docs/themes/nightshade.md §2, §4.3)
# --------------------------------------------------------------------------

SAFE_W, SAFE_H = 0.60, 0.70          # §2.2 central safe field
SAFE_MEAN_LUM_MAX = 0.06
SAFE_BRIGHT_FRAC_MAX = 0.02
SAFE_BRIGHT_LEVEL = 0.25

MIN_SHORT_SIDE = 1440                # §2.2
TARGET_ASPECT = 16 / 9
ASPECT_TOLERANCE = 0.02

LOOP_MIN_S, LOOP_MAX_S = 10.0, 30.0  # §2.3
SEAM_MAX_RATIO = 2.0

HW_CODECS = {                        # §2.1 — formats, never decoders
    "h264": {"High", "Main", "Constrained Baseline"},
    "hevc": {"Main", "Main 10"},
}

# §4.3 — nightshade supplies tone and chroma; the media supplies hue only.
# N1 is open: these are the starting point, not a settled signature.
#
# The accent is specified as "the most chromatic colour available at a
# lightness that still clears the contrast floor", not as a fixed C*. A fixed
# C* is not realisable: sRGB's gamut varies enormously by hue, and C* 75 at
# L* 57.5 is reachable for magenta and impossible for cyan. Seeking the cusp
# within a lightness band gives every hue the most neon it can actually be.
ACCENT_L_MIN, ACCENT_L_MAX = 50.0, 70.0
ACCENT_L_STEP = 0.5
ACCENT_C_CAP = 110.0
SURFACE_L, SURFACE_C = 12.0, 6.0
ON_SURFACE_L, ON_SURFACE_C = 92.0, 8.0
OUTLINE_L, OUTLINE_C = 58.0, 8.0
ERROR_HEX = "ffb4ab"                 # fixed by the theme; never tracks the media

TEXT_CONTRAST_FLOOR = 4.5            # §2.4 scrim target
UI_PERCENTILE = 90                   # the worst part of a region, not its average
MATUGEN_FALLBACK = "adc6ff"          # what matugen returns when extraction fails


@dataclass
class Clause:
    ref: str
    name: str
    met: bool
    measured: str
    degradation: str = ""


@dataclass
class Result:
    path: str
    kind: str
    clauses: list[Clause] = field(default_factory=list)
    hue: float = 0.0
    hue_source: str = ""
    palette: dict = field(default_factory=dict)
    assertions: list[Clause] = field(default_factory=list)
    scrim_alpha: float = 0.0
    regions: dict = field(default_factory=dict)
    safe_field: tuple = (0.0, 0.0)
    notes: list[str] = field(default_factory=list)


# --------------------------------------------------------------------------
# Probing
# --------------------------------------------------------------------------


def _run(cmd: list[str]) -> str:
    return subprocess.run(cmd, capture_output=True, text=True, check=False).stdout


def is_video(path: Path) -> bool:
    return path.suffix.lower() in {".mp4", ".mkv", ".mov", ".webm", ".m4v"}


def probe_video(path: Path) -> dict:
    out = _run(
        [
            "ffprobe", "-v", "error", "-select_streams", "v:0",
            "-show_entries",
            "stream=codec_name,profile,width,height,pix_fmt,color_range,"
            "color_primaries,nb_frames,avg_frame_rate:format=duration",
            "-of", "json", str(path),
        ]
    )
    try:
        d = json.loads(out)
    except json.JSONDecodeError:
        return {}
    s = (d.get("streams") or [{}])[0]
    s["duration"] = float((d.get("format") or {}).get("duration") or 0.0)
    return s


def canonical_frame(path: Path, tmp: Path) -> Image.Image:
    """The frame a scene's palette and safe field are measured from.

    For a loop this is the midpoint: a first frame can be a fade-in and is not
    representative of what you actually look at.
    """
    if not is_video(path):
        return Image.open(path).convert("RGB")
    info = probe_video(path)
    mid = max(0.0, info.get("duration", 0.0) / 2)
    out = tmp / "canonical.png"
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-ss", f"{mid}", "-i", str(path),
         "-frames:v", "1", str(out)],
        check=False,
    )
    if not out.exists():
        raise SystemExit(f"could not extract a frame from {path}")
    return Image.open(out).convert("RGB")


# --------------------------------------------------------------------------
# Measurement
# --------------------------------------------------------------------------


def _linear(arr: np.ndarray) -> np.ndarray:
    return np.where(arr <= 0.04045, arr / 12.92, ((arr + 0.055) / 1.055) ** 2.4)


def luminance_map(img: Image.Image) -> np.ndarray:
    a = np.asarray(img, dtype=np.float64) / 255.0
    lin = _linear(a)
    return 0.2126 * lin[..., 0] + 0.7152 * lin[..., 1] + 0.0722 * lin[..., 2]


def safe_field_box(w: int, h: int) -> tuple[int, int, int, int]:
    bw, bh = int(w * SAFE_W), int(h * SAFE_H)
    return (w - bw) // 2, (h - bh) // 2, (w + bw) // 2, (h + bh) // 2


def ui_regions(W: int, H: int) -> dict[str, tuple[int, int, int, int]]:
    """The boxes the greeter's own text and input field occupy.

    Measuring these, rather than one large safe field, is the difference
    between predicting legibility and averaging it away: a bright disc behind
    the date line barely moves the mean over 60x70% of the frame.
    """
    cx, cy = W // 2, H // 2
    pad = 14

    def text_box(sample: str, size_px: int, centre_y: int) -> tuple[int, int, int, int]:
        f = _font(size_px)
        d = ImageDraw.Draw(Image.new("RGB", (1, 1)))
        x0, y0, x1, y1 = d.textbbox((0, 0), sample, font=f)
        w, h = x1 - x0, y1 - y0
        return (cx - w // 2 - pad, centre_y - h // 2 - pad,
                cx + w // 2 + pad, centre_y + h // 2 + pad)

    fw, fh = 340, 56
    return {
        "clock": text_box("00:00", 110, cy - 220),
        "date": text_box("Saturday, 7 November", 22, cy - 130),
        "user": text_box("dirk", 16, cy - 40),
        "input field": (cx - fw // 2 - pad, cy + 40 - fh // 2 - pad,
                        cx + fw // 2 + pad, cy + 40 + fh // 2 + pad),
    }


def measure_regions(img: Image.Image) -> dict[str, float]:
    """Luminance at the UI_PERCENTILE of each region: the bright part."""
    W, H = img.size
    lum = luminance_map(img)
    out = {}
    for name, (x0, y0, x1, y1) in ui_regions(W, H).items():
        x0, y0 = max(0, x0), max(0, y0)
        x1, y1 = min(W, x1), min(H, y1)
        patch = lum[y0:y1, x0:x1]
        out[name] = float(np.percentile(patch, UI_PERCENTILE)) if patch.size else 0.0
    return out


def measure_safe_field(img: Image.Image) -> tuple[float, float]:
    w, h = img.size
    x0, y0, x1, y1 = safe_field_box(w, h)
    lum = luminance_map(img)[y0:y1, x0:x1]
    return float(lum.mean()), float((lum > SAFE_BRIGHT_LEVEL).mean())


def hue_via_matugen(src: Path, frame: Image.Image, tmp: Path) -> tuple[float, str, str] | None:
    """Extract hue the way the shipped palette does.

    --dry-run is not optional and is hard-coded: without it, `matugen image`
    writes through the user's config.toml and re-themes the running desktop.
    """
    if not shutil.which("matugen"):
        return None
    if is_video(src):
        # A re-encode changes the answer, so the frame handed to matugen is
        # produced by exactly one pinned command and never touched again.
        p = tmp / "canonical.png"
        if not p.exists():
            frame.save(p)
    else:
        p = src  # never re-encode a still: see the bit-depth note below
    out = _run(
        ["matugen", "image", str(p), "--dry-run", "--prefer", "saturation",
         "--json", "hex", "-q"]
    )
    try:
        d = json.loads(out)
        v = d["colors"]["primary"]["dark"]
        hexv = (v["color"] if isinstance(v, dict) else v).lstrip("#")
    except (json.JSONDecodeError, KeyError, TypeError):
        return None
    _, _, h = C.rgb_to_lch(C.hex_to_rgb(hexv))
    return h, "matugen", hexv


def hue_internal(img: Image.Image) -> tuple[float, str, str]:
    """Chroma-weighted circular mean hue. Used only when matugen is absent."""
    small = img.resize((160, 90))
    a = np.asarray(small, dtype=np.float64) / 255.0
    flat = a.reshape(-1, 3)
    labs = np.array([C.rgb_to_lab(tuple(p)) for p in flat])
    chroma = np.hypot(labs[:, 1], labs[:, 2])
    keep = chroma > 10
    if not keep.any():
        return 0.0, "internal (no chromatic pixels)", ""
    ang = np.arctan2(labs[keep, 2], labs[keep, 1])
    wts = chroma[keep]
    h = math.degrees(math.atan2((np.sin(ang) * wts).sum(), (np.cos(ang) * wts).sum())) % 360
    return h, "internal", ""


def max_chroma_at(L: float, h: float) -> float:
    _, used = C.lch_to_rgb_clipped(L, ACCENT_C_CAP, h)
    return used


def seek_accent(hue: float, surface_hex: str) -> tuple[float, float]:
    """The sRGB cusp for this hue, within the lightness band, holding contrast.

    Returns (L, C). Lightness is free inside the band precisely because chroma
    is the thing the theme cares about; a neon accent that had to be pastel to
    hit a lightness number would be the wrong trade.
    """
    srgb = C.hex_to_rgb(surface_hex)
    best = (ACCENT_L_MIN, 0.0)
    L = ACCENT_L_MIN
    while L <= ACCENT_L_MAX + 1e-9:
        c = max_chroma_at(L, hue)
        rgb, _ = C.lch_to_rgb_clipped(L, c, hue)
        if C.contrast(rgb, srgb) >= 4.5 and c > best[1]:
            best = (L, c)
        L += ACCENT_L_STEP
    return best


def build_palette(hue: float) -> dict:
    def mk(L: float, Cc: float, h: float = hue) -> dict:
        rgb, used = C.lch_to_rgb_clipped(L, Cc, h)
        return {"hex": C.rgb_to_hex(rgb), "L": round(L, 1),
                "C_requested": round(Cc, 1), "C_used": round(used, 1)}

    surface = mk(SURFACE_L, SURFACE_C)
    aL, aC = seek_accent(hue, surface["hex"])
    pal = {
        "primary": mk(aL, aC),
        "secondary": mk(min(aL + 6, ACCENT_L_MAX), aC * 0.45),
        "tertiary": mk(aL + 4, max_chroma_at(aL + 4, (hue + 60) % 360) * 0.9,
                       (hue + 60) % 360),
        "surface": surface,
        "on_surface": mk(ON_SURFACE_L, ON_SURFACE_C),
        "outline": mk(OUTLINE_L, OUTLINE_C),
    }
    pal["error"] = {"hex": ERROR_HEX, "L": round(C.rgb_to_lch(C.hex_to_rgb(ERROR_HEX))[0], 1),
                    "C_requested": None, "C_used": None}
    return pal


def palette_assertions(pal: dict) -> list[Clause]:
    rgb = {k: C.hex_to_rgb(v["hex"]) for k, v in pal.items()}
    out = []

    def add(ref, name, value, ok, fmt):
        out.append(Clause(ref, name, ok, fmt.format(value)))

    s = rgb["surface"]
    add("§4.3", "surface darkness", pal["surface"]["L"],
        pal["surface"]["L"] <= 20, "L* {:.1f} (≤ 20)")
    add("§4.3", "on_surface vs surface", C.contrast(rgb["on_surface"], s),
        C.contrast(rgb["on_surface"], s) >= 7, "{:.2f}:1 (≥ 7)")
    add("§4.3", "primary vs surface", C.contrast(rgb["primary"], s),
        C.contrast(rgb["primary"], s) >= 4.5, "{:.2f}:1 (≥ 4.5)")
    add("§4.3", "error vs surface", C.contrast(rgb["error"], s),
        C.contrast(rgb["error"], s) >= 4.5, "{:.2f}:1 (≥ 4.5)")
    cusp = max_chroma_at(pal["primary"]["L"], hue_of(pal["primary"]["hex"]))
    out.append(Clause(
        "§4.3", "accent at the sRGB cusp", pal["primary"]["C_used"] >= cusp - 1.0,
        f"C* {pal['primary']['C_used']:.1f} at L* {pal['primary']['L']:.1f} "
        f"(sRGB maximum for this hue at this lightness: {cusp:.1f})"))
    return out


def hue_of(hexv: str) -> float:
    return C.rgb_to_lch(C.hex_to_rgb(hexv))[2]


def scrim_alpha_for(lum: float, fg_hex: str) -> float:
    """Alpha of a black scrim needed to reach the text contrast floor.

    Compositing black at alpha a multiplies linear luminance by (1 - a).
    """
    lt = C.relative_luminance(C.hex_to_rgb(fg_hex))
    target = (lt + 0.05) / TEXT_CONTRAST_FLOOR - 0.05
    if lum <= target or lum <= 0:
        return 0.0
    return float(min(0.90, max(0.0, 1 - target / lum)))


def measure_seam(path: Path, info: dict, tmp: Path) -> tuple[float, float, float] | None:
    """(seam diff, ordinary consecutive diff, ratio) in 0-255 mean abs terms."""
    dur = info.get("duration", 0.0)
    if dur <= 0:
        return None
    fps = 30.0
    try:
        n, d = info.get("avg_frame_rate", "30/1").split("/")
        if float(d):
            fps = float(n) / float(d)
    except (ValueError, ZeroDivisionError):
        pass
    step = 1.0 / fps

    def grab(t: float, name: str) -> np.ndarray | None:
        out = tmp / name
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-ss", f"{max(0.0, t)}", "-i", str(path),
             "-frames:v", "1", "-vf", "scale=480:-1", str(out)],
            check=False,
        )
        if not out.exists():
            return None
        return np.asarray(Image.open(out).convert("RGB"), dtype=np.float64)

    first = grab(0.0, "f_first.png")
    last = grab(max(0.0, dur - step), "f_last.png")
    a = grab(dur / 2, "f_a.png")
    b = grab(dur / 2 + step, "f_b.png")
    if any(x is None for x in (first, last, a, b)):
        return None
    seam = float(np.abs(last - first).mean())
    ordinary = float(np.abs(b - a).mean())
    ratio = seam / ordinary if ordinary > 1e-6 else float("inf")
    return seam, ordinary, ratio


# --------------------------------------------------------------------------
# Clause evaluation
# --------------------------------------------------------------------------


def evaluate(path: Path, tmp: Path, hue_override: float | None = None) -> Result:
    kind = "video" if is_video(path) else "still"
    res = Result(path=str(path), kind=kind)
    frame = canonical_frame(path, tmp)
    w, h = frame.size

    if kind == "video":
        info = probe_video(path)
        codec = info.get("codec_name", "?")
        profile = info.get("profile", "?")
        ok = profile in HW_CODECS.get(codec, set())
        res.clauses.append(Clause(
            "§2.1", "codec", ok, f"{codec} / {profile}",
            "" if ok else "runtime fallback per R9: still, then solid colour"))

        prim = info.get("color_primaries") or "unset"
        rng = info.get("color_range") or "unset"
        ok = prim in ("bt709", "unknown") and rng != "unset"
        res.clauses.append(Clause(
            "§2.1", "colour primaries / range", ok, f"{prim} / {rng}",
            "" if ok else "assume limited range, and say so in this report"))

        dur = info.get("duration", 0.0)
        ok = LOOP_MIN_S <= dur <= LOOP_MAX_S
        res.clauses.append(Clause(
            "§2.3", "loop duration", ok, f"{dur:.1f} s ({LOOP_MIN_S:.0f}–{LOOP_MAX_S:.0f})",
            "" if ok else "played as-is"))

        seam = measure_seam(path, info, tmp)
        if seam:
            s, o, r = seam
            res.clauses.append(Clause(
                "§2.3", "loop seam", r <= SEAM_MAX_RATIO,
                f"{r:.2f}× an ordinary frame step (seam {s:.2f}, ordinary {o:.2f})",
                "" if r <= SEAM_MAX_RATIO else "played as-is; the cut is visible"))
        bits = info.get("pix_fmt", "")
        res.notes.append(f"pixel format {bits or 'unknown'}"
                         + ("" if "10" in bits else " — 8-bit banding risk on a near-black field"))

    short = min(w, h)
    ok = short >= MIN_SHORT_SIDE
    res.clauses.append(Clause(
        "§2.2", "resolution", ok, f"{w}×{h} (short side {short} ≥ {MIN_SHORT_SIDE})",
        "" if ok else "upscaled; it will look soft"))

    aspect = w / h
    ok = abs(aspect - TARGET_ASPECT) <= ASPECT_TOLERANCE
    res.clauses.append(Clause(
        "§2.2", "aspect", ok, f"{aspect:.3f} (16:9 = {TARGET_ASPECT:.3f})",
        "" if ok else "centre-cropped to cover the output"))

    mean_lum, bright = measure_safe_field(frame)
    comp_ok = mean_lum <= SAFE_MEAN_LUM_MAX and bright <= SAFE_BRIGHT_FRAC_MAX
    res.clauses.append(Clause(
        "§2.2", "composition (dark field)", comp_ok,
        f"mean luminance {mean_lum:.4f} (≤ {SAFE_MEAN_LUM_MAX}), "
        f"{bright * 100:.2f}% above {SAFE_BRIGHT_LEVEL} (≤ {SAFE_BRIGHT_FRAC_MAX * 100:.0f}%)",
        "" if comp_ok else "no scrim on its own: this is a look clause, not a legibility one"))
    res.safe_field = (mean_lum, bright)

    got = None if hue_override is not None else hue_via_matugen(path, frame, tmp)
    if hue_override is not None:
        res.hue, res.hue_source, extracted = hue_override % 360, "declared", ""
    elif got is None:
        res.hue, res.hue_source, extracted = hue_internal(frame)
    else:
        res.hue, res.hue_source, extracted = got
    if extracted and extracted.lower() == MATUGEN_FALLBACK:
        res.notes.append(
            "matugen extracted no source colour and returned its fallback "
            f"#{MATUGEN_FALLBACK} — the hue below is matugen's, not this image's")

    res.palette = build_palette(res.hue)
    res.assertions = palette_assertions(res.palette)

    # Legibility, measured where the UI actually lands.
    fg = {"clock": res.palette["on_surface"]["hex"],
          "date": res.palette["outline"]["hex"],
          "user": res.palette["primary"]["hex"],
          "input field": res.palette["primary"]["hex"]}
    measured = measure_regions(frame)
    worst = 0.0
    for name, lum in measured.items():
        need = scrim_alpha_for(lum, fg[name])
        res.regions[name] = (lum, need)
        worst = max(worst, need)
        res.clauses.append(Clause(
            "§2.2", f"legibility · {name}", need == 0.0,
            f"background at p{UI_PERCENTILE} = {lum:.4f}",
            "" if need == 0.0 else f"scrim {need * 100:.0f}% behind this element"))
    res.scrim_alpha = worst
    return res


# --------------------------------------------------------------------------
# Preview — the part you actually judge
# --------------------------------------------------------------------------


FONT_CANDIDATES = [
    "/usr/share/fonts/TTF/MesloLGSNerdFont-Regular.ttf",
    "/usr/share/fonts/TTF/MesloLGSNerdFontMono-Regular.ttf",
]


def _font(size: int) -> ImageFont.FreeTypeFont:
    for p in FONT_CANDIDATES:
        if Path(p).exists():
            return ImageFont.truetype(p, size)
    return ImageFont.load_default(size)


def _rgba(hexv: str, alpha: float) -> tuple[int, int, int, int]:
    r, g, b = C.hex_to_rgb(hexv)
    return int(r * 255), int(g * 255), int(b * 255), int(alpha * 255)


def render_preview(path: Path, res: Result, out: Path, tmp: Path,
                   size: tuple[int, int] = (2560, 1440), debug: bool = False) -> None:
    """Composite the greeter's own UI over the candidate, at nightshade geometry.

    This exists because a scene cannot be judged on its own. What matters is
    whether the clock and the input field survive on top of it.
    """
    frame = canonical_frame(path, tmp)
    W, H = size
    scale = max(W / frame.width, H / frame.height)
    frame = frame.resize((max(1, round(frame.width * scale)), max(1, round(frame.height * scale))),
                         Image.LANCZOS)
    left, top = (frame.width - W) // 2, (frame.height - H) // 2
    base = frame.crop((left, top, left + W, top + H)).convert("RGBA")

    if res.scrim_alpha > 0:
        x0, y0, x1, y1 = safe_field_box(W, H)
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        rx, ry = (x1 - x0) / 2, (y1 - y0) / 2
        yy, xx = np.mgrid[0:H, 0:W]
        d = np.sqrt(((xx - cx) / (rx * 1.35)) ** 2 + ((yy - cy) / (ry * 1.35)) ** 2)
        a = np.clip(1 - d, 0, 1) ** 1.6 * res.scrim_alpha
        scrim = Image.fromarray(
            np.dstack([np.zeros((H, W, 3), dtype=np.uint8),
                       (a * 255).astype(np.uint8)]), "RGBA")
        base = Image.alpha_composite(base, scrim)

    pal = res.palette
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    cx, cy = W // 2, H // 2

    import datetime as _dt
    now = _dt.datetime.now()

    def centred(text: str, y: int, size_px: int, hexv: str, alpha: float = 1.0) -> None:
        f = _font(size_px)
        w = d.textbbox((0, 0), text, font=f)[2]
        d.text((cx - w // 2 + 3, y + 3), text, font=f, fill=(17, 17, 27, int(170 * alpha)))
        d.text((cx - w // 2, y), text, font=f, fill=_rgba(hexv, alpha))

    centred(now.strftime("%H:%M"), cy - 220 - 70, 110, pal["on_surface"]["hex"])
    centred(now.strftime("%A, %-d %B"), cy - 130 - 14, 22, pal["outline"]["hex"])
    centred("dirk", cy - 40 - 10, 16, pal["primary"]["hex"])

    fw, fh, r = 340, 56, 12
    x0, y0 = cx - fw // 2, cy + 40 - fh // 2
    d.rounded_rectangle([x0, y0, x0 + fw, y0 + fh], radius=r,
                        fill=_rgba(pal["surface"]["hex"], 0.80),
                        outline=_rgba(pal["primary"]["hex"], 1.0), width=2)
    f = _font(15)
    d.text((x0 + 18, y0 + fh // 2 - 10), "Password", font=f,
           fill=_rgba(pal["outline"]["hex"], 0.9))

    if debug:
        bx = safe_field_box(W, H)
        d.rectangle(bx, outline=_rgba(pal["error"]["hex"], 0.8), width=3)
        d.text((bx[0] + 10, bx[1] + 10), "safe field 60×70", font=_font(18),
               fill=_rgba(pal["error"]["hex"], 0.9))

    sw = 120
    for i, k in enumerate(("primary", "secondary", "tertiary", "surface",
                           "on_surface", "outline", "error")):
        d.rectangle([40 + i * (sw + 8), H - 110, 40 + i * (sw + 8) + sw, H - 50],
                    fill=_rgba(pal[k]["hex"], 1.0))
        d.text((40 + i * (sw + 8), H - 44), f"{k} {pal[k]['hex']}", font=_font(13),
               fill=_rgba("ffffff", 0.75))

    Image.alpha_composite(base, layer).convert("RGB").save(out, quality=92)


# --------------------------------------------------------------------------
# matugen regression assertions (issue #5)
# --------------------------------------------------------------------------


def matugen_assertions() -> int:
    """Re-run the measurement that killed the gate design.

    If matugen ever stops fixing tones, nightshade's whole palette argument
    changes and this is where it is noticed.
    """
    if not shutil.which("matugen"):
        print("matugen not installed — skipping")
        return 0
    cases = {
        "flat grey": Image.new("RGB", (640, 360), (128, 128, 128)),
        "near white": Image.new("RGB", (640, 360), (240, 238, 236)),
    }
    grad = Image.new("RGB", (640, 360))
    gd = ImageDraw.Draw(grad)
    for y in range(360):
        t = y / 359
        gd.line([(0, y), (640, y)],
                fill=(int(154 + t * 26), int(160 + t * 24), int(164 + t * 23)))
    cases["washed gradient"] = grad

    ok = True
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        print(f"{'source':18} {'surface relLum':>15} {'on/surf':>9} {'prim/surf':>10} "
              f"{'prim L*':>8} {'prim C*':>8}")
        for name, im in cases.items():
            p = tmp / "x.png"
            im.save(p)
            out = _run(["matugen", "image", str(p), "--dry-run", "--prefer",
                        "saturation", "--json", "hex", "-q"])
            try:
                cols = json.loads(out)["colors"]
            except (json.JSONDecodeError, KeyError):
                print(f"{name:18}  could not parse matugen output")
                ok = False
                continue

            def g(k: str) -> tuple[float, float, float]:
                v = cols[k]["dark"]
                return C.hex_to_rgb((v["color"] if isinstance(v, dict) else v))

            s, o, pr = g("surface"), g("on_surface"), g("primary")
            lum = C.relative_luminance(s)
            c_os, c_ps = C.contrast(o, s), C.contrast(pr, s)
            L, Ch, _ = C.rgb_to_lch(pr)
            print(f"{name:18} {lum:15.4f} {c_os:8.2f}:1 {c_ps:9.2f}:1 {L:8.1f} {Ch:8.1f}")
            for label, val, lo, hi in (
                ("surface luminance", lum, 0.004, 0.010),
                ("on_surface contrast", c_os, 13.5, 15.5),
                ("primary contrast", c_ps, 10.0, 12.0),
                ("primary L*", L, 76.0, 84.0),
                ("primary C*", Ch, 25.0, 40.0),
            ):
                if not lo <= val <= hi:
                    print(f"    FAIL {label}: {val:.3f} outside [{lo}, {hi}]")
                    ok = False
    print("\nMaterial You still fixes tones; only hue varies." if ok else
          "\nmatugen's behaviour has CHANGED — nightshade §4.2 needs re-measuring.")
    return 0 if ok else 1


# --------------------------------------------------------------------------


def report(res: Result) -> None:
    print(f"\n  {res.path}  ({res.kind})\n")
    print("  CONTRACT (§2) — nightshade refuses nothing; a miss names its degradation\n")
    for c in res.clauses:
        mark = "✓" if c.met else "→"
        print(f"   {mark} {c.ref:6} {c.name:26} {c.measured}")
        if not c.met and c.degradation:
            print(f"       {'':6} {'':26} ↳ {c.degradation}")
    if res.scrim_alpha > 0:
        print(f"\n       scrim required: {res.scrim_alpha * 100:.0f}% black behind the UI")

    print(f"\n  PALETTE (§4.3) — hue {res.hue:.1f}° from the media ({res.hue_source}), "
          f"tone and chroma from the theme\n")
    for k, v in res.palette.items():
        extra = ""
        if v["C_requested"] is not None and v["C_used"] < v["C_requested"] - 0.5:
            extra = f"  (chroma clipped from {v['C_requested']:.0f} to fit sRGB)"
        print(f"     {k:11} #{v['hex']}   L* {v['L']:>5}{extra}")

    print("\n  ASSERTIONS\n")
    for c in res.assertions:
        print(f"   {'✓' if c.met else '✗'} {c.name:26} {c.measured}")

    for n in res.notes:
        print(f"\n  note: {n}")
    print()


def main() -> int:
    ap = argparse.ArgumentParser(description="Measure a nightshade scene candidate.")
    ap.add_argument("path", nargs="?", type=Path)
    ap.add_argument("--preview", type=Path, help="render the greeter UI over it, for judging")
    ap.add_argument("--debug-overlay", action="store_true", help="draw the safe field box")
    ap.add_argument("--hue", type=float,
                    help="declare the scene's hue in degrees instead of extracting it")
    ap.add_argument("--json", action="store_true", dest="as_json")
    ap.add_argument("--matugen-assertions", action="store_true",
                    help="re-run the §4.2 measurement and check it still holds")
    a = ap.parse_args()

    if a.matugen_assertions:
        return matugen_assertions()
    if a.path is None:
        ap.error("a media path is required")
    if not a.path.exists():
        raise SystemExit(f"no such file: {a.path}")

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        res = evaluate(a.path, tmp, hue_override=a.hue)
        if a.preview:
            render_preview(a.path, res, a.preview, tmp, debug=a.debug_overlay)
        if a.as_json:
            print(json.dumps({
                "path": res.path, "kind": res.kind, "hue": res.hue,
                "hue_source": res.hue_source, "scrim_alpha": res.scrim_alpha,
                "palette": res.palette, "notes": res.notes,
                "clauses": [vars(c) for c in res.clauses],
                "assertions": [vars(c) for c in res.assertions],
            }, indent=2))
        else:
            report(res)
            if a.preview:
                print(f"  preview → {a.preview}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
