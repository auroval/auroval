#!/usr/bin/env nu
#
# P1/E — R16's real number: first frame, at native resolution, on a real VT.
#
# R16's budget was provisional until 2026-11-07, set from one P0 measurement
# of 243 ms at 1255x674 through a NESTED cage inside a live Hyprland session.
# Both halves of that are wrong for a greeter: the surface was a window, not a
# 2560x1440 monitor, and the machine underneath it was a running desktop. Q5
# then measured the same binary at 44% of a core on a quiet desktop and 60% on
# a busy one, which is the whole argument for why a greeter cannot be judged
# from inside a session.
#
# This script runs the real binary under a real cage on a real VT, warm, ten
# times, and prints the median. It has been run: median 8 ms, worst 9 ms, at
# 2560x1440, receipt in cage-run-2026-11-07-2326.txt. 243 against 8 is the
# cost of the nesting, not of the greeter.
#
# It is kept, and still the thing to run, because that number covers only the
# still background. A video wallpaper and R23's audio both pull in GStreamer,
# which nothing on this path touches yet — which is also why C15's 0.9 s
# registry rescan does not show up here at all: cold and warm both came out at
# 8 ms, a rescan that never happened rather than a cheap one. When the media
# path lands this gets taken again, and the COLD run is the one that matters.
#
# NO SUDO. cage takes DRM master through logind from whichever VT you are
# logged in on, so a TTY login is all this needs. That is also exactly why it
# refuses to run inside your desktop session: wlroots would nest inside it and
# measure the desktop all over again.
#
# It touches no system configuration. It runs one binary and writes one file.
#
# Run it like this:
#
#   1. Ctrl+Alt+F3, log in as yourself
#   2. auroval-vt          (written by deploy/install.nu; or the long form:
#                           cd ~/Documents/projects/auroval
#                           nu spike/p1e/cage-measure.nu)
#   3. STAY ON THIS VT until it prints `written to ...` — about twenty
#      seconds. cage can only hold DRM master while its session is the
#      active one, so switching to your desktop mid-run makes every
#      remaining run fail after a ten-second wait for a device it will
#      never get. The first real run of this script lost eight of eleven
#      runs that way.
#   4. Ctrl+Alt+F1 to come back to your desktop
#
# Escape route: your desktop session is untouched on its own VT the whole
# time, and nothing here will switch you back to this one. Nothing can hold
# the screen either: every single run is wrapped in `timeout`, so the worst
# case is one black frame for that many seconds and then your console back.

const OUT = "~/auroval-p1e-first-frame.txt"
const BUDGET_MS = 500

def main [
    --runs: int = 10            # warm runs; the median of these is the number
    --bin: string = ""          # override the binary under test
    --cache: string = ""        # cache dir for the run; default is a private one
    --expect: string = "2560x1440"  # the surface size this measurement claims
    --timeout: int = 30         # seconds to allow one run before giving up
] {
    let repo = ($env.FILE_PWD | path dirname | path dirname)
    let bin = if ($bin | is-empty) { $"($repo)/target/release/auroval-greet" } else { $bin }
    let cache = if ($cache | is-empty) {
        ("~/.cache/auroval-p1e" | path expand)
    } else { $cache }
    let out = ($OUT | path expand)

    # ── the refusal that is the point of the script ──────────────────────
    if ($env | get -o WAYLAND_DISPLAY | is-not-empty) {
        print "WAYLAND_DISPLAY is set, so cage would nest inside your desktop"
        print "and measure exactly what this script exists to avoid. The P0"
        print "number this run is meant to replace was taken that way."
        print "Log in on a free VT (Ctrl+Alt+F3) and run it there."
        exit 2
    }
    if ($env | get -o DISPLAY | is-not-empty) {
        print "DISPLAY is set — an X session is in the way. Use a free VT."
        exit 2
    }

    let steps = 4 + 1 + $runs
    mut step = 0
    let started = (date now | format date "%Y-%m-%dT%H:%M:%S%:z")
    mut lines = [
        "auroval P1/E — first frame under cage on a real VT (R16, C15)"
        $started
        ""
        $"binary    ($bin)"
        $"cache     ($cache)"
        $"runs      ($runs) warm, 1 cold"
        $"budget    ($BUDGET_MS) ms  \(settled 2026-11-07: median 8 ms at 2560x1440)"
        $"host      (sys host | get hostname)  kernel (sys host | get kernel_version)"
        $"tty       (do { ^tty } | complete | get stdout | str trim)"
        ""
    ]
    # Written before the first run, not after the last: a measurement you can
    # abort halfway and still have evidence from is a measurement you will
    # actually repeat. P0 lost a whole run to output flushed only on exit.
    $lines | str join "\n" | save -f $out

    # ── 1: the binary ────────────────────────────────────────────────────
    $step = $step + 1
    print $"  ($step)/($steps)  binary"
    if not ($bin | path exists) {
        print $"         ($bin) is missing — run: cargo build --release -p greet"
        exit 2
    }
    if (which cage | is-empty) {
        print "         cage is not installed — pacman -S cage"
        exit 2
    }
    print $"         ($bin)"

    # ── 2: the cache (C15) ───────────────────────────────────────────────
    $step = $step + 1
    print $"  ($step)/($steps)  cache"
    mkdir $cache
    let registry = $"($cache)/gst-registry.bin"
    $lines = ($lines | append $"cache dir ($cache)")
    # The deployed greeter gets /var/cache/auroval, owned by greeter and so
    # not writable by you. This run uses its own directory and says so, rather
    # than pretending to measure the deployed path from the wrong account.
    if ("/var/cache/auroval" | path exists) {
        print "         /var/cache/auroval exists (the deployed path, greeter-owned)"
        $lines = ($lines | append "deployed  /var/cache/auroval exists")
    } else {
        print "         /var/cache/auroval does NOT exist — the deployed greeter"
        print "         would have nowhere to cache, which C15 measured at ~0.9 s"
        print "         per login. Run: nu deploy/install.nu"
        $lines = ($lines | append "deployed  /var/cache/auroval MISSING — C15 not satisfied in deployment")
    }
    $lines | str join "\n" | save -f $out

    # ── 3: probe, outside cage ───────────────────────────────────────────
    $step = $step + 1
    print $"  ($step)/($steps)  probe, no compositor"
    let p1 = (do { ^$bin probe } | complete)
    $lines = ($lines | append ["" "── probe, outside cage, as you ──" $p1.stdout $p1.stderr])
    $lines | str join "\n" | save -f $out
    print ($p1.stdout | lines | each { |l| $"         ($l)" } | str join "\n")

    # ── 4: probe, under cage ─────────────────────────────────────────────
    # Not redundant with step 3. This is the first thing in the run that needs
    # DRM master, so if the `greeter` user is missing from `video`, or another
    # compositor already holds the device, it fails here with a clear cause
    # rather than inside a timing loop.
    $step = $step + 1
    print $"  ($step)/($steps)  probe, under cage"
    let p2 = (do { ^cage -s -- $bin probe } | complete)
    if $p2.exit_code != 0 {
        print $"         cage exited ($p2.exit_code):"
        print ($p2.stderr | lines | last 10 | each { |l| $"         ($l)" } | str join "\n")
        $lines = ($lines | append ["" $"cage failed, exit ($p2.exit_code)" $p2.stderr])
        $lines | str join "\n" | save -f $out
        exit 1
    }
    print "         cage took the device"
    $lines = ($lines | append ["" "── probe, under cage ──" $p2.stdout])
    $lines | str join "\n" | save -f $out

    # ── 5..: the measurement ─────────────────────────────────────────────
    # Cold first: registry deleted, so the run pays the rescan C15 measured.
    # Then the warm runs, which are the ones R16 is about — a greeter that
    # rescans on every boot has already failed the budget in deployment.
    mut results = []
    let modes = (["cold"] | append (1..$runs | each { "warm" }))
    for mode in $modes {
        $step = $step + 1
        if $mode == "cold" and ($registry | path exists) { rm -f $registry }
        # Counted, not just named: ten near-identical fullscreen runs with no
        # sense of how many are left reads as an infinite loop, and someone
        # nearly interrupted a good measurement on exactly that.
        print $"  ($step)/($steps)  ($mode) run…"
        let r = (run-once $bin $cache $timeout)
        $results = ($results | append { mode: $mode, frame: $r.frame, size: $r.size, wall: $r.wall, code: $r.code })
        let shown = (if $r.frame == null { $"wall ($r.wall) ms \(no first-frame line)" } else { $"first frame ($r.frame) ms" })
        print $"           ($shown)  surface ($r.size)  exit ($r.code)"
        # The failure that actually happened, named the moment it happens
        # rather than left as a bare exit 1 in a table. cage can only take
        # DRM master while its session is the ACTIVE one, so the instant you
        # press Ctrl+Alt+F1 every remaining run dies — after a ten-second
        # wait for a device it will never get. The first real run of this
        # script lost eight of eleven runs exactly that way.
        if $r.frame == null and $r.code == 1 and ($results | any { |x| $x.frame != null }) {
            print "           ^ a run that worked already, so the binary is fine."
            print "             cage needs this VT to be the active one. If you have"
            print "             switched to your desktop, come back to this VT and"
            print "             run it again — every run from here will fail."
        }
        $lines = ($lines | append $"($mode)  first frame ($r.frame | default '—') ms  wall ($r.wall) ms  surface ($r.size)  exit ($r.code)")
        $lines | str join "\n" | save -f $out
    }

    # ── the verdict ──────────────────────────────────────────────────────
    let warm = ($results | where mode == "warm")
    let frames = ($warm | get frame | where { |v| $v != null })
    let sizes = ($results | get size | uniq | where { |s| $s != "—" })
    $lines = ($lines | append ["" "── verdict ──"])

    if ($frames | is-empty) {
        # This branch used to be the normal one, when `auroval-greet` had no
        # Wayland shell to draw with. Now it means something went wrong: the
        # wall times are the process's whole lifetime, an upper bound on
        # nothing useful, recorded so a run is never silently empty rather
        # than so R16 can be settled from them.
        let walls = ($warm | get wall | math median)
        let codes = ($results | get code | uniq)
        print ""
        print "  NO FIRST-FRAME LINE in any run."
        print "  The binary printed no `first frame N ms, surface WxH`, so R16"
        print "  is NOT measured by this run. This used to be the normal"
        print "  outcome, before the greeter had a shell to draw with; now it"
        print "  means something is wrong. Check the probe output above."
        print $"  Median warm wall time, for what little it is worth: ($walls) ms"
        $lines = ($lines | append [
            "R16 NOT MEASURED — the binary printed no first-frame line."
            "This is now a fault, not the expected state — see the probe output above."
            $"median warm wall time ($walls) ms — process lifetime, not first frame"
        ])
        if not (0 in $codes) {
            # "No first-frame line" and "the binary refused to start" deserve
            # to be told apart on the console: the first is a greeter that ran
            # and drew nothing, the second is a greeter that never ran. Exit 2
            # is the usage message, which once meant the binary had no shell
            # and now means it was handed no mode.
            print $"  Every run exited non-zero \(($codes | str join ', ')\) — the binary did"
            print "  not even start a greeter. Exit 2 is its usage message:"
            print "  check that this script passes a mode (`preview`)."
            $lines = ($lines | append $"every run exited non-zero (($codes | str join ', ')) — no greeter was started")
        }
    } else {
        let median = ($frames | math median)
        let worst = ($frames | math max)
        let size = ($sizes | str join ", ")
        print ""
        print $"  median first frame  ($median) ms   \(worst ($worst) ms)"
        print $"  surface             ($size)"
        $lines = ($lines | append [
            $"median first frame ($median) ms over ($frames | length) warm runs"
            $"worst ($worst) ms"
            $"surface ($size)"
        ])
        # A median over three runs and a median over ten are not the same
        # claim, and the file has to say which one it is making.
        let lost = ($warm | where frame == null | length)
        if $lost > 0 {
            print $"  ($lost) of ($warm | length) warm runs produced no frame. Almost always this"
            print "  means the VT stopped being the active one part-way through:"
            print "  cage cannot hold DRM master from a background session. The"
            print "  median above rests on the runs that did draw."
            $lines = ($lines | append $"($lost) of ($warm | length) warm runs produced no frame — VT not active; median rests on ($frames | length) runs")
        }
        if $size != $expect {
            print $"  surface is ($size), not ($expect) — this is NOT the native-"
            print "  resolution number R16 asked for. Check the monitor mode."
            $lines = ($lines | append $"WARNING surface ($size) != expected ($expect) — not a native-resolution result")
        }
        if $median <= $BUDGET_MS {
            print $"  the ($BUDGET_MS) ms budget HOLDS at native resolution."
            $lines = ($lines | append $"R16 budget ($BUDGET_MS) ms HOLDS — median ($median) ms, worst ($worst) ms")
        } else {
            print $"  the ($BUDGET_MS) ms budget does NOT hold. Move it, with this number."
            $lines = ($lines | append $"R16 budget ($BUDGET_MS) ms FAILS — median ($median) ms, worst ($worst) ms")
        }
        let cold = ($results | where mode == "cold" | get frame | first)
        if $cold != null {
            $lines = ($lines | append $"cold first frame ($cold) ms — the cost of an unwarmed cache \(C15)")
        }
    }

    $lines | append "" | str join "\n" | save -f $out
    print ""
    print $"written to ($out)"
    print "Ctrl+Alt+F1 to go back to your desktop."
}

# One run under cage. Returns the first-frame time in ms when the binary
# reports one, the surface size when it reports one, and the wall time always.
def run-once [bin: string, cache: string, timeout: int] {
    let t0 = (date now)
    let r = (do {
        with-env {
            XDG_CACHE_HOME: $cache
            GST_REGISTRY: $"($cache)/gst-registry.bin"
            # One frame and out. Without this, `preview` draws forever and
            # every run has to be killed by the timeout above: eleven runs is
            # five and a half minutes of black screen on a VT you have to stay
            # on, and switching away costs you every remaining run.
            AUROVAL_EXIT_AFTER_FIRST_FRAME: "1"
        } { ^timeout $"($timeout)s" cage -s -- $bin preview }
    } | complete)
    let wall = ((date now) - $t0 | into int) / 1000000
    let text = $"($r.stdout)\n($r.stderr)"
    # One line from the greeter carries both numbers:
    #     auroval-greet: first frame 14 ms, surface 2560x1440
    # The P0 spike's older spellings are kept as a fallback so a run against
    # that binary still parses.
    let frame = (
        $text
        | parse -r '(?m)first frame (?:after )?([\d.]+)\s*ms'
        | get -o capture0.0
        | default ($text | parse -r '(?m)first presented[^:]*:\s*([\d.]+)\s*ms after start' | get -o capture0.0)
    )
    let size = (
        $text
        | parse -r '(?m)surface (\d+x\d+)'
        | get -o capture0.0
        | default ($text | parse -r '(?m)configured\s*:?\s*(\d+x\d+)' | get -o capture0.0)
        | default "—"
    )
    {
        frame: (if ($frame | is-empty) { null } else { $frame | into float })
        size: $size
        wall: ($wall | math round -p 1)
        code: $r.exit_code
    }
}
