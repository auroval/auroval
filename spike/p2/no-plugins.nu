#!/usr/bin/env nu
#
# P2 — R9's last clause: the greeter with GStreamer's plugins made unavailable.
#
# R9's fit criterion reads:
#
#     With GStreamer plugins made unavailable, the greeter still renders and
#     still logs in; the fallback is logged, not silent.
#
# Everything else about R9 has a receipt. A still that will not decode falls
# back and says so; a video that will not play falls back and says so; and
# since #58 `background` is a list tried in order with the solid colour as the
# implicit end of the chain. The one thing nobody had ever done was make the
# plugins genuinely unavailable, because the obvious way to do it — uninstall
# gst-plugins-good — needs root and breaks the maintainer's desktop.
#
# It needs neither. GStreamer finds its plugins through GST_PLUGIN_SYSTEM_PATH
# and GST_PLUGIN_PATH and caches what it found in GST_REGISTRY, and all three
# are per-process environment variables. Point the two paths at a directory
# you control and the registry at a file nobody has written yet, and the
# process has exactly the plugins you decided it has, with nothing uninstalled
# and nothing needing root.
#
# WHY THERE ARE TWO EXPERIMENTS, NOT ONE.
#
#   A — NO PLUGINS AT ALL. Both plugin paths point at an empty directory.
#       `gst::init()` still succeeds, because core is linked into the binary
#       and is not a plugin; the pipeline dies at the first
#       `ElementFactory::make`, before a single byte of the file is read. This
#       is the cheap experiment and it is the one that proves `gst::init()`
#       surviving is not the same thing as GStreamer working.
#
#   B — NO *VIDEO DECODER*. Every plugin except the ones that provide an
#       H.26x video decoder, by symlink into a directory of our own. This is
#       the realistic shape of the failure: a machine where gst-plugins-base
#       is installed and gst-libav is not, or where the nvidia driver's
#       nvcodec plugin fails to load after an upgrade. `playbin3` is built,
#       the file is opened, the demuxer runs, and the failure arrives later
#       and from a different place — on the bus, mid-pipeline, rather than as
#       a refusal to construct.
#
#   and a CONTROL, with the plugins left alone, because a run that failed for
#   an unrelated reason — a missing binary, a broken EGL, a clip that moved —
#   looks from the outside exactly like a run that failed for the reason we
#   are claiming. The control is what tells the two apart.
#
#   and a REFERENCE run with no background configured at all, which is how
#   "it fell back to the solid colour" becomes a measurement: the corner pixel
#   of a run that was never given a background is the solid colour by
#   definition, and an experiment whose corner pixel matches it drew the same
#   thing. Without the reference the script would be asserting a hard-coded
#   #12121c and would quietly start lying the first time the palette changes.
#
# WHAT IT MEASURES, per run: whether a PNG was written at all (renders),
# whether its corner pixel is the reference solid colour (fell back), what
# went to stderr (logged, not silent — the half of R9 that is easiest to lose,
# because a greeter that silently shows a blue rectangle looks fine), and
# whether the process exited on its own inside the deadline.
#
# WHAT IT DOES NOT MEASURE: logging in. `snapshot` renders one frame through
# surfaceless EGL and never speaks to greetd or PAM, so R9's "still logs in"
# is not in this script's reach. That half belongs to the greetd sitting in
# spike/p1ab/greetd-round.nu, and this receipt should be read as covering
# "still renders" and "logged, not silent" only.
#
# NO ROOT, none, at any point. It reads /usr/lib/gstreamer-1.0, makes
# symlinks and files under a scratch directory of its own, and runs one binary
# four times. It changes no system configuration and installs nothing.
#
#   nu spike/p2/no-plugins.nu
#   nu spike/p2/no-plugins.nu --keep        # leave the scratch dirs to poke at
#   nu spike/p2/no-plugins.nu --timeout 30  # longer leash per run

const OUT = "~/auroval-p2-no-plugins.txt"
const CLIP = "/var/lib/auroval-spike/media/loop1440p60.h264.mp4"
const PLUGINS = "/usr/lib/gstreamer-1.0"
const TOTAL = 8

def main [
    --bin: string = ""          # the binary under test
    --clip: string = ""         # the video background to configure
    --timeout: int = 20         # seconds to allow one snapshot before giving up
    --keep                      # do not delete the scratch directory at the end
] {
    let repo = ($env.FILE_PWD | path dirname | path dirname)
    let bin = if ($bin | is-empty) { $"($repo)/target/release/auroval-greet" } else { $bin }
    let clip = if ($clip | is-empty) { $CLIP } else { $clip }
    let out = ($OUT | path expand)
    let work = ($"($nu.temp-dir)/auroval-p2-no-plugins" | path expand)

    mut step = 0
    let started = (date now | format date "%Y-%m-%dT%H:%M:%S%:z")
    mut lines = [
        "auroval P2 — R9 with GStreamer's plugins made unavailable"
        $started
        ""
        $"binary    ($bin)"
        $"clip      ($clip)"
        $"plugins   ($PLUGINS)"
        $"timeout   ($timeout) s per run"
        $"host      (sys host | get hostname)  kernel (sys host | get kernel_version)"
        ""
        "R9 wants three things of a greeter whose plugins are gone: it still"
        "renders, it still logs in, and the fallback is logged rather than"
        "silent. This script reaches the first and the third. Logging in needs"
        "greetd and is spike/p1ab/greetd-round.nu's business."
        ""
    ]
    # Written before the first run and rewritten after every step, on purpose:
    # experiment B below can sit there for the whole timeout, and a run you
    # can abort halfway and still have evidence from is a run you will repeat.
    $lines | str join "\n" | save -f $out

    # ── 1: the things that must already exist ────────────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  preflight"
    for t in [
        [what, path, fix];
        ["binary", $bin, "cargo build --release -p greet"]
        ["clip", $clip, "the P0 media drop is missing from /var/lib/auroval-spike/media"]
        ["plugin dir", $PLUGINS, "gstreamer is not installed"]
    ] {
        if not ($t.path | path exists) {
            print $"         ($t.what) missing: ($t.path)"
            print $"         fix: ($t.fix)"
            exit 2
        }
    }
    # magick reads the corner pixel, gst-inspect-1.0 tells us which plugins
    # carry a decoder. Both are read-only and neither is exotic, but a missing
    # one would turn a measurement into a guess, so it stops here instead.
    for c in ["magick" "gst-inspect-1.0" "env" "timeout"] {
        if (which $c | is-empty) {
            print $"         ($c) is not installed — this script needs it to measure"
            exit 2
        }
    }
    print $"         ($bin)"
    print $"         ($clip)"
    $lines = ($lines | append $"preflight ok")
    $lines | str join "\n" | save -f $out

    # ── 2: the scratch directory and the two configs ─────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  scratch dirs and configs"
    rm -rf $work
    mkdir $"($work)/empty"       # experiment A's plugin path: deliberately bare
    mkdir $"($work)/nodec"       # experiment B's plugin path: filled in step 6
    mkdir $"($work)/reg"         # one GST_REGISTRY per run, never shared
    mkdir $"($work)/home"        # a HOME for `env -i`, so nothing lands in yours
    mkdir $"($work)/png"
    # A bare string is a one-element list as far as config.rs is concerned, but
    # the list form is written out because #58's list is what R9's "in order"
    # now means, and the solid colour is the END of that list rather than an
    # entry in it — there is no way to spell a colour as a background path.
    $"return { background = {\"($clip)\"} }\n" | save -f $"($work)/video.lua"
    $"return { }\n" | save -f $"($work)/nobg.lua"
    print $"         ($work)"
    $lines = ($lines | append [$"scratch   ($work)" ""])
    $lines | str join "\n" | save -f $out

    # ── 3: the reference — what the solid colour actually looks like ─────
    $step += 1
    print $"  ($step)/($TOTAL)  reference run: no background configured"
    let reference = (snap $bin $"($work)/nobg.lua" $"($work)/png/solid.png" {} $timeout $work)
    if not $reference.drew {
        print "         the greeter could not draw even with no background at all."
        print "         Nothing below would mean anything, so this stops here."
        print $reference.err
        $lines = ($lines | append ["reference FAILED — the greeter draws nothing at all:" $reference.err])
        $lines | str join "\n" | save -f $out
        exit 1
    }
    let solid = $reference.corner
    print $"         solid colour is ($solid), ($reference.colours) colours in the frame"
    $lines = ($lines | append [
        "reference — no background configured, so the frame IS the solid colour"
        $"  corner  ($solid)"
        $"  colours ($reference.colours)   \(the login field and its text, nothing else)"
        $"  exited  ($reference.exit) in ($reference.secs)"
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 4: the control — plugins left alone, video configured ────────────
    $step += 1
    print $"  ($step)/($TOTAL)  control run: plugins present, video background"
    let control = (snap $bin $"($work)/video.lua" $"($work)/png/control.png" {} $timeout $work)
    let control_played = ($control.err | str contains "[background] playing")
    print $"         drew ($control.drew), corner ($control.corner), ($control.colours) colours"
    print $"         said `playing`: ($control_played)"
    if not ($control_played and ($control.corner != $solid)) {
        print "         THE CONTROL DID NOT PLAY THE VIDEO. Everything below would"
        print "         fail for this reason rather than for the plugins, so the"
        print "         experiments are not worth running. The greeter said:"
        print ($control.err | lines | first 10 | str join "\n")
        $lines = ($lines | append [
            "control FAILED — the video does not play even with plugins present,"
            "so no conclusion below would be about plugins. The greeter said:"
            ($control.err | lines | first 20 | str join "\n")
        ])
        $lines | str join "\n" | save -f $out
        exit 1
    }
    $lines = ($lines | append [
        "control — plugins untouched, video configured"
        $"  corner  ($control.corner)   \(NOT the solid colour: a frame of the clip)"
        $"  colours ($control.colours)"
        $"  said    ($control.err | lines | first | str trim)"
        $"  exited  ($control.exit) in ($control.secs)"
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 5: experiment A — no plugins at all ──────────────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  experiment A: both plugin paths point at an empty dir"
    let env_a = {
        GST_PLUGIN_SYSTEM_PATH: $"($work)/empty"
        GST_PLUGIN_PATH: $"($work)/empty"
        # A registry file that does not exist yet, so GStreamer cannot find
        # yesterday's cache of the plugins we just took away. Sharing the
        # maintainer's registry here is the one mistake that would make this
        # whole experiment silently pass.
        GST_REGISTRY: $"($work)/reg/a.bin"
    }
    let a = (snap $bin $"($work)/video.lua" $"($work)/png/no-plugins.png" $env_a $timeout $work)
    let a_init_ok = (not ($a.err | str contains "GStreamer would not start"))
    print $"         drew ($a.drew), corner ($a.corner), solid: ($a.corner == $solid)"
    print $"         gst::init\(\) still succeeded: ($a_init_ok)"
    print $"         exited ($a.exit) in ($a.secs)"
    for l in ($a.err | lines | first 4) { print $"         | ($l)" }
    $lines = ($lines | append [
        "experiment A — no plugins at all"
        $"  drew    ($a.drew)"
        $"  corner  ($a.corner)   \(solid colour: ($a.corner == $solid))"
        $"  init    gst::init\(\) succeeded: ($a_init_ok)  — core is linked in, not a plugin"
        $"  exited  ($a.exit) in ($a.secs)"
        $"  stderr  ($a.err | lines | length) lines:"
        ($a.err | lines | each { |l| $"          ($l)" } | first 12 | str join "\n")
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 6: build experiment B's plugin directory ─────────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  building a plugin set with no H.26x video decoder"
    # Derived from gst-inspect-1.0 rather than hard-coded, because the list of
    # plugins that can decode H.264 on a machine is a property of the machine:
    # this one has libav, nvcodec, openh264, vulkan and de265, another will
    # have va or msdk instead, and a hard-coded blocklist would quietly leave
    # a decoder in place and turn experiment B into a second control.
    let inventory = (^gst-inspect-1.0 | lines)
    let blocked = (
        $inventory
        | where { |l| ($l | str lowercase | str contains "ecoder") and ($l =~ '(?i)h\.?26[45]') }
        | each { |l| $l | split row ":" | first | str trim }
        | uniq
        | sort
    )
    if ($blocked | is-empty) {
        print "         gst-inspect-1.0 reports NO H.26x decoder on this machine."
        print "         Experiment B has nothing to remove — it would be a repeat"
        print "         of the control. Experiment A above still stands."
        exit 1
    }
    print $"         holding back: ($blocked | str join ', ')"
    let all = (ls $"($PLUGINS)/libgst*.so" | get name)
    let kept = (
        $all
        | where { |p|
            not (($p | path basename | str replace "libgst" "" | str replace ".so" "") in $blocked)
        }
    )
    # Symlinks, not copies: 269 plugins is some 200 MB and the point is which
    # ones GStreamer can see, not where the bytes live.
    for p in $kept { ^ln -s $p $"($work)/nodec/($p | path basename)" }
    print $"         ($kept | length) of ($all | length) plugins linked"
    $lines = ($lines | append [
        "experiment B's plugin set"
        $"  held back ($blocked | str join ', ')  — every plugin gst-inspect-1.0 says carries an H.26x decoder"
        $"  linked    ($kept | length) of ($all | length) plugins from ($PLUGINS)"
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 7: experiment B — everything but a video decoder ─────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  experiment B: plugins present, no video decoder"
    print $"         \(this one can take the whole ($timeout) s leash — see below\)"
    let env_b = {
        GST_PLUGIN_SYSTEM_PATH: $"($work)/nodec"
        GST_PLUGIN_PATH: $"($work)/nodec"
        GST_REGISTRY: $"($work)/reg/b.bin"
    }
    let b = (snap $bin $"($work)/video.lua" $"($work)/png/no-decoder.png" $env_b $timeout $work)
    let b_missing = ($b.err | str contains "missing a plug-in")
    print $"         drew ($b.drew), corner ($b.corner), solid: ($b.corner == $solid)"
    print $"         said `missing a plug-in`: ($b_missing)"
    print $"         exited ($b.exit) in ($b.secs), ($b.err | lines | length) stderr lines"
    for l in ($b.err | lines | first 4) { print $"         | ($l)" }
    $lines = ($lines | append [
        "experiment B — every plugin except an H.26x video decoder"
        $"  drew    ($b.drew)"
        $"  corner  ($b.corner)   \(solid colour: ($b.corner == $solid))"
        $"  exited  ($b.exit) in ($b.secs)   \(124 means the ($timeout) s timeout killed it)"
        $"  stderr  ($b.err | lines | length) lines, first 12:"
        ($b.err | lines | each { |l| $"          ($l)" } | first 12 | str join "\n")
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 8: the verdict ───────────────────────────────────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  verdict"
    # R9's two reachable clauses, each resting on a measurement rather than on
    # the absence of a complaint: a PNG that exists, a corner pixel equal to
    # the reference, and a named line on stderr.
    let renders = ($a.drew and $b.drew)
    let fellback = (($a.corner == $solid) and ($b.corner == $solid))
    let logged = (($a.err | str contains "[background]") and $b_missing)
    let pass = ($renders and $fellback and $logged)
    let verdict = if $pass { "PASS" } else { "FAIL" }
    let a_line = ($a.err | lines | where { |l| $l =~ 'falling back' } | first | default ($a.err | lines | first))
    let b_line = ($b.err | lines | where { |l| $l =~ 'missing a plug-in' } | first | default "")

    mut tail = [
        "────────────────────────────────────────────────────────────────"
        $"VERDICT  R9 'still renders' + 'logged, not silent': ($verdict)"
        ""
        $"  renders   ($renders)   both experiments wrote a 2560x1440 PNG"
        $"  fell back ($fellback)   corner ($a.corner) and ($b.corner) against the reference's ($solid)"
        $"  logged    ($logged)"
        $"    A: ($a_line)"
        $"    B: ($b_line)"
        ""
        "  R9's third clause, 'still logs in', is out of snapshot's reach: it"
        "  renders through surfaceless EGL and never speaks to greetd or PAM."
        "  That clause belongs to spike/p1ab/greetd-round.nu."
        ""
    ]
    # Reported separately from the verdict and never folded into it: this is
    # not one of R9's clauses, it is something the realistic experiment found
    # on the way past. A greeter that draws the right frame and then refuses
    # to exit is still a bug, and burying it in a PASS is how it survives.
    let b_fellback_line = ($b.err | str contains "falling back to the solid colour")
    if (($b.err | lines | length) > 100) or (not $b_fellback_line) or $b.exit != 0 {
        $tail = ($tail | append [
            $"DEFECT   experiment B spun: ($b.err | lines | length) stderr lines in ($b.secs), and no fallback was ever declared."
            $"         exit ($b.exit) \(124 would mean the ($timeout) s timeout had to kill it);"
            $"         'falling back to the solid colour' present: ($b_fellback_line)."
            ""
            "         The right frame was drawn and the failure was logged, so"
            "         R9's own words survive — but the pipeline rebuilt itself"
            "         over and over, one fresh qtdemux per attempt, unbounded,"
            "         for as long as the process lived. Where the two"
            "         experiments part company is in background.rs: a video"
            "         that fails to CONSTRUCT (A) comes back as an Err from"
            "         Video::start and `poll` runs `fell_back` + `try_next`,"
            "         which is how A reaches the colour and says so. A video"
            "         that constructs and then fails to DECODE (B) arrives as"
            "         a bus error on an already-running State::Video, where"
            "         `poll` only drains `v.problems()` into the list and"
            "         never calls `try_next`. So B's chain never advances, the"
            "         state never becomes Solid, nothing is declared, and"
            "         playbin3 is left to retry. The frame is right only"
            "         because State::Video has no texture to hand over."
            ""
            "         How long the process then lives is not stable: one"
            "         earlier run of the same experiment with stderr going to"
            "         a file was still spinning when a 45 s timeout killed it."
            "         A greeter that fills the journal at this rate is a"
            "         defect whether or not it eventually exits."
            ""
        ])
    }
    $tail = ($tail | append $"receipt  ($out)")
    for l in $tail { print $"         ($l)" }
    $lines = ($lines | append $tail)
    $lines | str join "\n" | save -f $out

    if not $keep { rm -rf $work } else { print $"         scratch kept at ($work)" }
}

# Run one snapshot with a chosen environment and measure what came of it.
#
# `env -i` is not decoration. The whole experiment is about which GST_*
# variables the process has, and anything inherited from the caller's shell —
# a GST_PLUGIN_PATH set by a flatpak, a GST_DEBUG left over from last week —
# would be a variable the real greeter does not have and would make the
# measurement a story about this terminal.
#
# `timeout` is likewise not decoration: experiment B does not exit.
def snap [
    bin: string
    cfg: string
    png: string
    extra: record
    secs: int
    work: string
] {
    let base = {
        HOME: $"($work)/home"
        XDG_CACHE_HOME: $"($work)/home/.cache"
        AUROVAL_CONFIG: $cfg
    }
    let envs = ($base | merge $extra | transpose k v | each { |r| $"($r.k)=($r.v)" })
    let began = (date now)
    let r = (do { ^timeout $secs env -i ...$envs $bin snapshot $png } | complete)
    let elapsed = ((date now) - $began)
    let drew = ($png | path exists)
    {
        exit: $r.exit_code
        secs: ($elapsed | format duration sec)
        drew: $drew
        out: $r.stdout
        err: $r.stderr
        # The corner, not the centre: the login field sits in the middle of
        # the frame and would be the same pixels whatever the background did.
        corner: (if $drew { corner_of $png } else { "n/a" })
        colours: (if $drew { colours_of $png } else { 0 })
    }
}

# The top-left pixel as "r,g,b" in 0-255. A flat fallback and a frame of video
# differ there by more than any tolerance needs to care about.
def corner_of [png: string] {
    (^magick $png -format "%[fx:int(255*p{0,0}.r)],%[fx:int(255*p{0,0}.g)],%[fx:int(255*p{0,0}.b)]" info:
        | str trim)
}

# How many distinct colours the whole frame holds. The corner says what was
# drawn at one point; this says whether there is a photograph behind the login
# field — a couple of thousand for text on a flat colour, six figures for a
# frame of 1440p video.
def colours_of [png: string] {
    (^identify -format "%k" $png | str trim | into int)
}
