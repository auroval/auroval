#!/usr/bin/env nu
#
# Q5, follow-up: measure the two render paths in the greeter's own context.
#
# The first Q5 measurement ran inside a live Hyprland session and the absolute
# figures moved by 2x with whatever else was on screen. The ratio survived that
# and settled the renderer choice; the numbers did not, so R7's whole-path
# budget was left without a figure. This script is how that figure gets taken.
#
# NO SUDO. cage takes DRM master through logind from whichever VT you are
# logged in on, so this needs a TTY login and nothing more. That is also why it
# cannot be run from inside your desktop session: wlroots would either nest
# inside it (measuring the desktop again) or fail to take the device.
#
# What it does NOT do: it does not touch greetd, SDDM, hyprlock or any system
# config. It runs a throwaway binary under cage and writes one file.
#
# Run it like this:
#
#   1. Ctrl+Alt+F3, log in as yourself
#   2. cd ~/Documents/projects/auroval
#   3. nu spike/q5/cage-measure.nu
#   4. Ctrl+Alt+F1 (or F2) to come back to your desktop
#
# Escape route: your desktop session is untouched on its own VT the whole time.

const CLIP = "/var/lib/auroval-spike/media/loop1440p60.h264.mp4"
const OUT  = "~/auroval-q5-cage.txt"

def main [
    --runs: int = 5        # pairs of runs; they alternate so drift hits both
    --seconds: int = 10    # measurement window; must be under the clip's 12 s
    --clip: string = ""    # override the clip
] {
    let clip = if ($clip | is-empty) { $CLIP } else { $clip }
    let bin = "target/release/spike-q5"

    if not ($bin | path exists) {
        print $"($bin) is missing — run: cargo build -p spike-q5 --release"
        exit 2
    }
    if not ($clip | path exists) {
        print $"clip not found: ($clip)"
        exit 2
    }
    if ($env | get -o WAYLAND_DISPLAY | is-not-empty) {
        print "WAYLAND_DISPLAY is set, so cage would nest inside your desktop"
        print "and measure exactly what this script exists to avoid."
        print "Log in on a free VT (Ctrl+Alt+F3) and run it there."
        exit 2
    }

    let out = ($OUT | path expand)
    let started = (date now | format date "%Y-%m-%dT%H:%M:%S%:z")
    mut lines = [
        $"auroval Q5 — render paths measured under cage"
        $started
        ""
        $"clip      ($clip)"
        $"runs      ($runs) pairs, ($seconds) s each"
        $"host      (sys host | get hostname)  kernel (sys host | get kernel_version)"
        ""
    ]

    let total = ($runs * 2 * ($seconds + 3))
    print $"  ($runs) pairs, about ($total) s in total. Writing to ($out)."
    print "  Ctrl+Alt+F1 returns to your desktop at any point; the run just"
    print "  carries on here and nothing will switch you back."
    print ""
    $lines | str join "\n" | save -f $out

    # Alternate the modes so that any drift in the machine lands on both.
    for i in 1..$runs {
        for mode in ["gl" "shm"] {
            print $"  pair ($i): ($mode)…"
            let r = (do { ^cage -- $"($env.PWD)/($bin)" $mode $clip ($seconds | into string) }
                     | complete)
            let text = $"($r.stdout)\n($r.stderr)"
            let cpu = ($text | parse -r '(?m)cpu\s+[\d.]+ s\s+\(([\d.]+)% of one core\)'
                       | get -o capture0.0 | default "?")
            let fps = ($text | parse -r '(?m)presented\s+\d+ frames\s+\(([\d.]+) fps\)'
                       | get -o capture0.0 | default "?")
            let caps = ($text | parse -r '(?m)negotiated\s+(.+)' | get -o capture0.0 | default "?")
            $lines = ($lines | append $"pair ($i)  ($mode)  cpu ($cpu)%  fps ($fps)")
            if $mode == "gl" {
                $lines = ($lines | append $"           negotiated: ($caps)")
            }
            print $"           cpu ($cpu)%  fps ($fps)"
            # Saved after every pair, not at the end: P0 lost a whole run's
            # evidence to output that was only flushed on exit, and a measurement
            # you cannot abort halfway is a measurement you will not repeat.
            $lines | str join "\n" | save -f $out
        }
    }

    $lines = ($lines | append ["" "raw output follows, so nothing above has to be trusted" ""])
    $lines | str join "\n" | save -f $out
    print ""
    print $"written to ($out)"
    print "Ctrl+Alt+F1 to go back to your desktop."
}
