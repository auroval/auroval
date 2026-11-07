"""Colour maths for nightshade: sRGB <-> CIE Lab <-> LCh, contrast, gamut.

Deliberately dependency-free beyond numpy. Everything here is standard
sRGB/D65; no colour-management library is pulled in for six formulae.
"""

from __future__ import annotations

import math

D65 = (0.95047, 1.00000, 1.08883)


def _srgb_to_linear(u: float) -> float:
    return u / 12.92 if u <= 0.04045 else ((u + 0.055) / 1.055) ** 2.4


def _linear_to_srgb(u: float) -> float:
    return 12.92 * u if u <= 0.0031308 else 1.055 * (u ** (1 / 2.4)) - 0.055


def hex_to_rgb(h: str) -> tuple[float, float, float]:
    h = h.lstrip("#")
    return tuple(int(h[i : i + 2], 16) / 255 for i in (0, 2, 4))  # type: ignore[return-value]


def rgb_to_hex(rgb: tuple[float, float, float]) -> str:
    return "".join(f"{round(max(0.0, min(1.0, c)) * 255):02x}" for c in rgb)


def relative_luminance(rgb: tuple[float, float, float]) -> float:
    """WCAG relative luminance."""
    r, g, b = (_srgb_to_linear(c) for c in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    la, lb = relative_luminance(a), relative_luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def rgb_to_lab(rgb: tuple[float, float, float]) -> tuple[float, float, float]:
    r, g, b = (_srgb_to_linear(c) for c in rgb)
    x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b
    y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b
    z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b

    def f(t: float) -> float:
        return t ** (1 / 3) if t > 216 / 24389 else (841 / 108) * t + 4 / 29

    fx, fy, fz = f(x / D65[0]), f(y / D65[1]), f(z / D65[2])
    return 116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)


def lab_to_rgb(lab: tuple[float, float, float]) -> tuple[float, float, float]:
    L, a, b = lab
    fy = (L + 16) / 116
    fx, fz = fy + a / 500, fy - b / 200

    def fi(t: float) -> float:
        return t**3 if t**3 > 216 / 24389 else (108 / 841) * (t - 4 / 29)

    x, y, z = fi(fx) * D65[0], fi(fy) * D65[1], fi(fz) * D65[2]
    r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z
    g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z
    bl = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z
    return tuple(_linear_to_srgb(c) for c in (r, g, bl))  # type: ignore[return-value]


def lab_to_lch(lab: tuple[float, float, float]) -> tuple[float, float, float]:
    L, a, b = lab
    return L, math.hypot(a, b), math.degrees(math.atan2(b, a)) % 360


def lch_to_lab(lch: tuple[float, float, float]) -> tuple[float, float, float]:
    L, C, h = lch
    r = math.radians(h)
    return L, C * math.cos(r), C * math.sin(r)


def rgb_to_lch(rgb: tuple[float, float, float]) -> tuple[float, float, float]:
    return lab_to_lch(rgb_to_lab(rgb))


def in_gamut(rgb: tuple[float, float, float], eps: float = 1e-4) -> bool:
    return all(-eps <= c <= 1 + eps for c in rgb)


def lch_to_rgb_clipped(L: float, C: float, h: float) -> tuple[tuple[float, float, float], float]:
    """Convert LCh to sRGB, reducing chroma until it fits the gamut.

    Returns (rgb, chroma_actually_used). Lightness and hue are preserved
    exactly; only chroma gives way, which is the trade that keeps a palette's
    tonal structure intact across hues.
    """
    rgb = lab_to_rgb(lch_to_lab((L, C, h)))
    if in_gamut(rgb):
        return tuple(min(1.0, max(0.0, c)) for c in rgb), C  # type: ignore[return-value]
    lo, hi = 0.0, C
    for _ in range(40):
        mid = (lo + hi) / 2
        if in_gamut(lab_to_rgb(lch_to_lab((L, mid, h)))):
            lo = mid
        else:
            hi = mid
    rgb = lab_to_rgb(lch_to_lab((L, lo, h)))
    return tuple(min(1.0, max(0.0, c)) for c in rgb), lo  # type: ignore[return-value]
