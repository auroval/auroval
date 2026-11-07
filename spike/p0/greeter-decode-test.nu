#!/usr/bin/env nu
#
# P0 spike, phase 1: does hardware video decode work as greetd's own user?
#
# PRD C6 records hardware decode as available on this machine but marks as
# unverified whether it works "in the greeter's context, where there is no user
# session, no XDG_RUNTIME_DIR of the logging-in user, and the process runs as
# greetd's own unprivileged user". Phase 1 isolates the *user* variable; the
# session variable is measured separately by stripping the environment.
#
# This script changes nothing about how the machine boots. It does not start
# greetd, does not touch /etc/greetd/config.toml, and leaves SDDM alone.
#
# Run with:  sudo AUROVAL_MEDIA=<dir of .mp4> nu spike/p0/greeter-decode-test.nu
# Clean up:  sudo nu spike/p0/greeter-decode-test.nu --clean
#
# Everything printed is also written to <stage>/report.md, world-readable, so
# the result is a durable receipt rather than terminal scrollback.

const STAGE = "/var/lib/auroval-spike"

def main [
    --clean             # remove the staging directory and exit
    --seconds: int = 20 # measurement window per run
] {
    if $clean {
        if ($STAGE | path exists) { rm -rf $STAGE }
        print $"removed ($STAGE)"
        return
    }

    if (id -u | into int) != 0 {
        print "must run as root (sudo nu spike/p0/greeter-decode-test.nu)"
        exit 2
    }
    if (^getent passwd greeter | complete).exit_code != 0 {
        print "no `greeter` user; is greetd installed?"
        exit 2
    }

    let media = ($env.AUROVAL_MEDIA? | default "")
    if ($media | is-empty) or not ($media | path exists) {
        print "set AUROVAL_MEDIA to a directory of test videos, e.g."
        print "  sudo AUROVAL_MEDIA=/path/to/media nu spike/p0/greeter-decode-test.nu"
        exit 2
    }
    let sources = (glob ($media | path join "*.mp4"))
    if ($sources | is-empty) {
        print $"no .mp4 files in ($media)"
        exit 2
    }

    let repo = ($env.FILE_PWD | path join .. .. | path expand)
    let src_bin = ($repo | path join target release spike-p0)
    if not ($src_bin | path exists) {
        print $"build it first: cargo build -p spike-p0 --release \(looked in ($src_bin)\)"
        exit 2
    }

    # Staging lives in /var/lib and not in a home directory: C10 says the
    # greeter cannot traverse a 0700 home, and R15 makes that a requirement
    # rather than an accident.
    mkdir $STAGE
    let bin = ($STAGE | path join spike-p0)
    cp $src_bin $bin
    let dest = ($STAGE | path join media)
    mkdir $dest
    for f in $sources { cp $f $dest }
    ^chmod 0755 $STAGE $dest $bin
    for f in (glob ($dest | path join "*.mp4")) { ^chmod 0644 $f }

    # The greeter user's home is `/`, which is not writable. That makes both
    # fontconfig and the GStreamer plugin registry uncacheable, so every start
    # rescans every plugin — a cost that lands straight on R16's first-frame
    # budget. A writable cache directory is therefore part of deploying a
    # greeter, not an optimisation, and this script measures both cases.
    let cache = ($STAGE | path join cache)
    mkdir $cache
    ^chown -R greeter:greeter $cache
    ^chmod 0700 $cache
    let registry = ($cache | path join "gst-registry.bin")

    # `env -i` is deliberately harsher than greetd itself: greetd gives its
    # greeter a logind session and therefore its own XDG_RUNTIME_DIR, so a pass
    # with no environment at all is a pass under any real configuration.
    let base = [
        env -i "PATH=/usr/bin:/bin" "HOME=/" "USER=greeter" "LOGNAME=greeter"
        $"XDG_CACHE_HOME=($cache)" $"GST_REGISTRY=($registry)"
    ]
    # The same, minus anywhere to write a cache: the naive deployment.
    let nocache = [ env -i "PATH=/usr/bin:/bin" "HOME=/" "USER=greeter" "LOGNAME=greeter" ]

    mut report = [
        "# P0 spike, phase 1 — hardware decode as greetd's own user"
        ""
        $"Run ((date now | format date '%Y-%m-%d %H:%M:%S %z')) on ((sys host).hostname)."
        ""
        "Method: the spike binary is run via `runuser -u greeter` with `env -i`,"
        "so it has uid 950, no session, no XDG_RUNTIME_DIR, no D-Bus and no"
        "driver hints. Media and binary are staged world-readable under"
        $"`($STAGE)`, outside any home directory."
        ""
        "## Versions"
        ""
        "```"
        (^uname -sr | str trim)
        (^pacman -Q greetd cage gstreamer gst-plugins-bad gst-plugin-va libva-nvidia-driver nvidia-utils | str trim)
        (^rustc --version | str trim)
        "```"
        ""
    ]

    print "######## process context, as user `greeter` ########"
    let ctx = (^runuser -u greeter -- ...$base $bin probe | complete)
    print $ctx.stdout
    $report = ($report | append ["## Context and GStreamer registry" "" "```" ($ctx.stdout | str trim) "```" ""])

    for video in (glob ($dest | path join "*.mp4")) {
        for path in [auto sw] {
            let label = $"($video | path basename), path=($path), realtime"
            print $"######## ($label) ########"
            let r = (^runuser -u greeter -- ...$base $bin decode $video --path $path --seconds $seconds | complete)
            print $r.stdout
            if ($r.stderr | str length) > 0 { print $r.stderr }
            $report = ($report | append [$"## ($label)" "" "```" ($r.stdout | str trim) "```" ""])
        }
    }

    let first = (glob ($dest | path join "*h264*.mp4") | first)
    print "######## headroom, as `greeter` ########"
    let hr = (^runuser -u greeter -- ...$base $bin decode $first --path auto --headroom --seconds $seconds | complete)
    print $hr.stdout
    $report = ($report | append ["## Headroom (sync=false, decode ceiling)" "" "```" ($hr.stdout | str trim) "```" ""])

    # R16: what the plugin registry costs at startup. Cold means the registry
    # file is absent and every plugin is rescanned, which is what a greeter with
    # no writable cache does on every single boot.
    print "######## startup cost of the GStreamer registry ########"
    if ($registry | path exists) { rm -f $registry }
    let cold = (timeit { ^runuser -u greeter -- ...$base $bin probe | complete })
    let warm = (timeit { ^runuser -u greeter -- ...$base $bin probe | complete })
    let uncached = (timeit { ^runuser -u greeter -- ...$nocache $bin probe | complete })
    let timing = [
        $"cold \(registry absent, must rescan\): ($cold)"
        $"warm \(registry cached\)             : ($warm)"
        $"no writable cache at all           : ($uncached)"
    ]
    for l in $timing { print $l }
    $report = ($report | append [
        "## Startup: GStreamer registry cost (R16)"
        ""
        "Process start to GStreamer ready, measured three ways. `cold` has a"
        "writable cache but no registry yet; `warm` reuses it; the third has"
        "nowhere to write, which is what greetd's greeter does by default"
        "because its home is `/`."
        ""
        "```"
        ...$timing
        "```"
        ""
    ])

    let out = ($STAGE | path join report.md)
    $report | str join "\n" | save -f $out
    ^chmod 0644 $out

    print ""
    print $"report written to ($out)"
    print "nothing about the boot path was changed; SDDM is untouched."
    print $"clean up with: sudo nu spike/p0/greeter-decode-test.nu --clean"
}
