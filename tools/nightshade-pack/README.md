# nightshade-pack

Derives a scene's palette **once**, at packaging time, and writes it down.

```nu
python3 tools/nightshade-pack/pack.py SCENE.png OUT/ --surface desktop
python3 tools/nightshade-pack/pack.py LOOP.mp4 OUT/ --surface greeter --hue 24.9
python3 tools/nightshade-pack/pack.py --self-test
```

Writes `palette.toml` (the canonical record) and `colors.lua` (the seven roles
as bare hex, the shape `~/.config/hypr/lua/colors.lua` already has).

## Why this exists

[nightshade.md §4.4](../../docs/themes/nightshade.md) has said since the theme
was written:

> the palette is derived once per scene, from one canonical frame, when the
> theme is packaged. It ships with the scene.

Nothing implemented it. What runs is `awww-wallpaper.sh`, calling matugen on
every wallpaper change, at runtime, on whichever file the rotation reached.

Three findings on 2026-11-08 all pointed at the same missing piece:

| | |
|---|---|
| **§5.5** | matugen's hue is not predictable from the image — not by area, not by chroma. `pond-warm-b-s1` returned 75.2° while its own orange was both the largest region and among the most saturated. Deterministic per file, opaque from outside, so the only safe use is to **look at the result once**. Recomputing it per rotation means nobody ever does. |
| **N8** | the theme's own scenes hold a hue band. Settled by withdrawing a scene after it was generated, measured, installed and lived with — four steps to discover a number §5.4 would have let it declare. |
| **[#63](https://github.com/auroval/auroval/issues/63)** | wallpapers have no install step at all. |

## `--hue` is N8's mechanism, and it is not a workaround

A declared hue skips extraction. That is the point: a scene matugen reads
wrongly, or one sitting in the 180°–250° trough where no accent exceeds
C\* 50, says what it is instead of being rejected after the fact.

Concretely — `hollow-s3` was withdrawn from the rotation for extracting
259.2°. Packed with `--hue 24.9` it yields `#ff304f`, one hex digit from the
scene it was withdrawn in favour of. Selection threw away a finished image to
fix a number. Declaration fixes the number.

## This tool computes nothing of its own

Every value comes from `scene_check.py`, which is the measurement the theme
documents quote. Two implementations of one palette is the bug this project
keeps finding — a greeter that drew one thing and believed another, twice —
and a packaging tool that re-derived the palette "more conveniently" would be
the next one.

For the same reason `--self-test` asserts that the TOML and the Lua carry the
same colours. They are produced by separate functions from one `Result`, and
a later edit to one is exactly the kind of change that silently desyncs them.

## What this does not do yet

It writes a palette next to a scene. It does not **install** one — see
[#63](https://github.com/auroval/auroval/issues/63) — and `awww-wallpaper.sh`
still runs matugen on rotation rather than reading what is shipped. Switching
that consumer over is a `pc-setup` change and is deliberately not bundled
here: this tool has to exist and be trusted first.
