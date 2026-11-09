#!/usr/bin/env nu
# P3's one question, asked without authenticating anybody:
#
#   does the shared renderer draw on an `ext-session-lock-v1` surface, does
#   the video still land in a GL texture there, and what does the present
#   cost against R7's 50%-of-one-core ceiling?
#
# It runs each front end in turn against the SAME compositor, the SAME clip
# and the SAME surface size, and reads each one's CPU out of /proc rather
# than trusting either binary's own arithmetic -- `auroval-greet` has no CPU
# column in its draw log at all, so an external instrument was needed for
# one of them and it would be unsound to use two.
#
# THE PAIRING IS THE POINT. An absolute figure from a nested compositor
# measures the nesting: R16's note in docs/prd/greet.md was bitten by exactly
# this, where "a nested cage at 1255x674 inside a running Hyprland session
# was measuring the nesting". The difference between two surfaces on one
# compositor at one size is the question P3 actually has -- whether the lock
# protocol costs anything the greeter's xdg_shell surface does not. The
# 1440p-on-a-free-VT figure that R7's ceiling is stated in needs a real VT
# and someone present; this does not claim to be it.
#
# SAFETY. `auroval-lock rehearse` locks whatever compositor it is pointed at,
# and ext-session-lock-v1 fails closed. So this script refuses to run against
# the session you are sitting in, and passes AUROVAL_LOCK_SECONDS so that
# a run is bounded without anyone at the keyboard. Since 2026-11-09 that
# timer EXITS WITHOUT UNLOCKING (L9), so the nested compositor is left locked
# at the end of a run and has to be closed. The measurement is unaffected:
# this script waits for the process to exit, which it still does.
#
# even a mistake releases itself.
#
# usage:
#   nu spike/p3/lock-measure.nu <nested WAYLAND_DISPLAY> [seconds]
#
# Bring the nested compositor up first, from the session you are in:
#   Hyprland -c spike/p3/nested.conf
# and read its socket out of `ls $env.XDG_RUNTIME_DIR | where name =~ wayland`
# -- it is the one that was not there before.

# utime + stime for one pid, in seconds.
#
# Counted from after the LAST ')' in /proc/<pid>/stat, because the comm field
# may itself contain spaces and parentheses and splitting from the left gets
# the wrong columns for a process whose name has either.
def cpu-seconds [pid: int]: nothing -> float {
    # -1 for a process that has gone, rather than an error: a run that ends
    # early is a result worth printing, and the caller says so.
    if not ($"/proc/($pid)/stat" | path exists) { return (-1.0) }
    let stat = (open --raw $"/proc/($pid)/stat" | decode utf-8)
    let f = ($stat | split row ')' | last | split row ' ' | where $it != '')
    # After the ')' the fields are state, ppid, ... so utime and stime sit at
    # index 11 and 12 here. USER_HZ is 100 on Linux for this purpose.
    ((($f | get 11 | into float) + ($f | get 12 | into float)) / 100.0)
}

def pid-of [name: string]: nothing -> int {
    let found = (ps | where name == $name | get pid)
    if ($found | is-empty) { -1 } else { ($found | first) }
}

def main [display: string, seconds: int = 40] {
    if $display == ($env.WAYLAND_DISPLAY? | default "") {
        print $"refusing: ($display) is this session's own compositor."
        print "`auroval-lock rehearse` would lock the screen you are reading this on."
        exit 2
    }
    if not ($"($env.XDG_RUNTIME_DIR)/($display)" | path exists) {
        print $"no such Wayland socket: ($env.XDG_RUNTIME_DIR)/($display)"
        exit 2
    }
    for b in ["./target/release/auroval-lock" "./target/release/auroval-greet"] {
        if not ($b | path exists) {
            print $"missing ($b) -- `cargo build --workspace --release` first"
            exit 2
        }
    }

    # R9's chain, in the format `main` understands: a flat list of paths. The
    # pool-of-chains format is on feat/nightshade-pack and not here yet, which
    # is why this writes its own config instead of using /etc/auroval/greet.lua.
    let cfg = (mktemp -t 'auroval-lock-XXXXXX.lua')
    [
        'return {'
        '  background = {'
        '    "/usr/local/share/auroval/nightshade-01.mp4",'
        '    "/usr/local/share/auroval/nightshade-01.png",'
        '  },'
        '}'
    ] | str join "\n" | save -f $cfg

    print $"compositor   ($display)"
    print $"clip         /usr/local/share/auroval/nightshade-01.mp4"
    print $"window       ($seconds) s per front end"
    print ""
    print "-- probe: is the protocol there at all? This locks nothing."
    with-env {WAYLAND_DISPLAY: $display, AUROVAL_CONFIG: $cfg} {
        ^./target/release/auroval-lock probe
    }
    print ""

    # ORDER MATTERS, and the first version of this script got it wrong.
    # `greet` ran second, read 0.3% of a core, and looked like a triumph --
    # it was drawing into a session the previous run had left LOCKED, where
    # the protocol says its surface is not displayed. The locker had been
    # killed before its own timer fired, and it failed closed exactly as
    # ext-session-lock-v1 promises. So the greeter goes first, and each run
    # is allowed to end by itself rather than being killed.
    let runs = [
        {
            name: "greet  xdg_shell"
            proc: "auroval-greet"
            cmd: "./target/release/auroval-greet"
            args: ["preview"]
            # `greet preview` has no deadline of its own, so `timeout` is it.
            extra: {}
            timeout: $seconds
        }
        {
            name: "lock   ext-session-lock-v1"
            proc: "auroval-lock"
            cmd: "./target/release/auroval-lock"
            args: ["rehearse"]
            # Its own deadline, so it releases the lock on the way out
            # instead of leaving the nested session locked for whatever runs
            # next. `timeout` is only the backstop behind it.
            extra: {AUROVAL_LOCK_REHEARSAL: "1", AUROVAL_LOCK_SECONDS: ($seconds | into string)}
            timeout: ($seconds + 15)
        }
    ]

    mut rows = []
    for run in $runs {
        let log = (mktemp -t 'auroval-run-XXXXXX.log')
        let env_ = ({WAYLAND_DISPLAY: $display, AUROVAL_CONFIG: $cfg, AUROVAL_LOG_DRAWS: "1"}
            | merge $run.extra)
        # `err>` rather than `complete`, so the log is on disk while the
        # process is still running: `complete` buffers until exit, and an
        # earlier version of this script lost every log line to that.
        let job = (job spawn {||
            with-env $env_ { ^timeout $run.timeout $run.cmd ...$run.args } err> $log
        })

        mut pid = -1
        for _ in 1..60 {
            sleep 250ms
            $pid = (pid-of $run.proc)
            if $pid > 0 { break }
        }
        if $pid < 1 {
            print $"($run.name): the process never appeared; see ($log)"
            continue
        }
        # Settle first. The opening second is GStreamer starting and, for the
        # locker, the handshake up to `locked` -- averaging those into a
        # steady-state figure is how a present cost comes out half of what it
        # is. The locker's own first version of this reported 15% while the
        # instantaneous cost was over 30%, for exactly that reason.
        sleep 8sec
        let t0 = (date now)
        let c0 = (cpu-seconds $pid)
        sleep (($seconds - 12) * 1sec)
        let c1 = (cpu-seconds $pid)
        let wall = (((date now) - $t0) / 1sec)
        if $c1 < 0 {
            print $"-- ($run.name)"
            print (open $log | lines | str join "\n")
            print "ENDED EARLY -- no steady-state figure. If the log says"
            print "`finished`, another client already holds a lock on this"
            print "compositor: restart it and run again."
            print ""
            continue
        }
        let pct = (100.0 * ($c1 - $c0) / $wall)

        # Let it finish on its own, so the locker releases the lock and the
        # next run is not drawing into a locked session.
        for _ in 1..60 {
            if (pid-of $run.proc) < 1 { break }
            sleep 1sec
        }

        print $"-- ($run.name)"
        print (open $log | lines
            | where ($it =~ 'memory|first (locked )?frame|surfaces|negotiat|problem|^auroval-lock: locked')
            | str join "\n")
        print $"cpu          ($pct | math round --precision 1)% of one core over ($wall | math round --precision 1) s steady state"
        print ""
        $rows = ($rows | append {surface: $run.name, cpu_pct: ($pct | math round --precision 1)})
    }
    rm -f $cfg

    print "steady state, same compositor, same clip, same surface size:"
    print ($rows | table)
    print ""
    print "R7's ceiling is 50% of one core for the whole present path, stated at"
    print "1440p60 on a free VT. These are NOT that measurement -- see the header."
    print "What they settle is whether ext-session-lock costs more than xdg_shell."
}
