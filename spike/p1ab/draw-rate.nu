#!/usr/bin/env nu
#
# How many frames an idle login screen draws, measured rather than reasoned
# about (#36, R6).
#
# `shell::run` used to call `draw_all()` on every turn of the loop, so a
# screen on which nothing moved was redrawn about sixty times a second. The
# console sitting on 2026-11-08 measured what that cost: SCTK counts the next
# key repeat from when its callback returned, so each of those frames was
# added to the repeat period, and 25/s came back as 20/s.
#
# The repeat rate itself needs a seat on a real VT — `keyboard-round.nu` is
# that sitting, and it cannot be taken from a desktop session. The *frame
# count* can be taken anywhere, which is the point of this one: it runs the
# greeter under a headless cage, where no pixel reaches a screen and no
# compositor of yours is involved, and reads the greeter's own counter.
#
# NO SUDO, and no VT. A headless backend needs neither DRM master nor a seat.
#
#   nu spike/p1ab/draw-rate.nu
#   nu spike/p1ab/draw-rate.nu --bin /some/other/auroval-greet --seconds 70
#
# --seconds 70 is worth asking for once: it crosses a minute boundary, and the
# clock rolling over is the one thing an idle greeter must still draw. Under
# 60 s it cannot tell "draws only on a change" from "never draws".

def main [
    --bin: string = ""          # the binary under test
    --seconds: int = 8          # how long to watch; 70 crosses a minute
] {
    let repo = ($env.FILE_PWD | path dirname | path dirname)
    let steps = 4
    mut step = 0

    # ── 1: the binary ────────────────────────────────────────────────────
    $step = $step + 1
    print $"  ($step)/($steps)  binary"
    let built = $"($repo)/target/release/auroval-greet"
    let bin = if ($bin | is-not-empty) { $bin } else { $built }
    if not ($bin | path exists) {
        print $"         ($bin) is missing. Build it and run this again:"
        print ""
        print "           cargo build --release -p greet"
        exit 2
    }
    print $"         ($bin)"

    # ── 2: cage ──────────────────────────────────────────────────────────
    $step = $step + 1
    print $"  ($step)/($steps)  cage"
    if (which cage | is-empty) {
        print "         cage is not installed — pacman -S cage"
        exit 2
    }
    print "         present, and it will be headless"

    # ── 3: the run ───────────────────────────────────────────────────────
    # WAYLAND_DISPLAY and DISPLAY are removed rather than refused. Nesting is
    # fatal to a repeat-rate measurement because the seat would be the
    # desktop's; it is harmless to a frame count, but a headless backend is
    # still the better instrument — nothing is composited, so nothing else can
    # be paying for the frames.
    $step = $step + 1
    print $"  ($step)/($steps)  run — ($seconds) s, headless, nothing on screen"
    let out = (
        do {
            with-env {
                WAYLAND_DISPLAY: null
                DISPLAY: null
                EGL_PLATFORM: null
                WLR_BACKENDS: "headless"
                WLR_LIBINPUT_NO_DEVICES: "1"
                AUROVAL_LOG_DRAWS: "1"
            } {
                ^timeout ($seconds + 2 | into string) cage -- $bin preview
            }
        } | complete | get stderr
    )
    let ticks = (
        $out
        | lines
        | parse --regex 'draws\s+(?<at>[0-9.]+)\s+drew (?<drew>\d+) of (?<turns>\d+) turns'
        | upsert at { |r| $r.at | into float }
        | upsert drew { |r| $r.drew | into int }
        | upsert turns { |r| $r.turns | into int }
    )
    if ($ticks | is-empty) {
        print "         no `draws` lines came back. The greeter did not start,"
        print "         or this binary predates AUROVAL_LOG_DRAWS:"
        print ""
        print ($out | lines | last 10 | str join "\n")
        exit 1
    }
    print $"         ($ticks | length) seconds of counter"

    # ── 4: the verdict ───────────────────────────────────────────────────
    # The first second is excluded on purpose and not to flatter the number:
    # it contains the first frame, which is a frame the greeter owes, and the
    # configure that sizes the surface.
    $step = $step + 1
    print $"  ($step)/($steps)  verdict"
    print ""
    let idle = ($ticks | skip 1)
    let frames = ($idle | get drew | math sum)
    let turns = ($idle | get turns | math sum)
    let secs = ($idle | length)
    print ($ticks | table)
    print ""
    print $"  after the first second: ($frames) frames in ($turns) turns over ($secs) s"
    let per_s = (if $secs > 0 { $frames / $secs } else { 0 })
    print $"  ($per_s | math round --precision 2) frames/s while nothing moved"
    print ""
    # One frame a minute is the clock, and that is not idleness. The
    # allowance is one per minute of watching, rounded up, because a minute
    # boundary can fall anywhere inside the window — including inside a short
    # one. It is not an allowance for sixty.
    let allowed = ($secs / 60 | math ceil)
    if $frames <= $allowed {
        print $"  PASS — at most ($allowed) frame\(s) were owed \(the clock), and ($frames) were drawn."
    } else {
        print $"  FAIL — ($frames) frames with nothing moving. At most ($allowed) was owed."
        exit 1
    }
}
