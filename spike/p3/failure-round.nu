#!/usr/bin/env nu
# P3c, the half that needs nobody at the keyboard: what does the locker do
# when the things around it break?
#
# WHY THIS RUNS BEFORE ANY REAL LOCK. Every case here is a way the locker can
# fail on a desktop you are sitting in, and on that desktop the cost of each
# one is the session. `ext-session-lock-v1` fails closed on purpose (L11), so
# a locker that dies holding the screen does not hand it back.
#
# What makes the round safe is a thing this script found rather than assumed,
# and it is worth reading before the code:
#
#   * A locker killed while it holds the lock does NOT leave a black screen.
#     Hyprland draws its own "the lockscreen app died" page, which names the
#     way out: `hyprctl --instance 0 eval 'hl.clear_crashed_lockscreen()'`
#     from another tty.
#   * That call works. It releases the lock and the desktop comes back.
#   * A SECOND locker cannot take over. Started against a compositor that
#     still holds a dead client's lock, `auroval-lock` is sent `finished` and
#     locks nothing. So a supervisor that restarts the binary is NOT a way
#     out of a crash -- it would restart a process that is refused. The way
#     out is the compositor's, and it needs a second tty.
#
# That is why this script can run case after case against one nested
# compositor: kill the locker, clear the crashed lock, go again.
#
# WHAT IT CANNOT ESTABLISH. Every requirement here ends in "...and the
# session still unlocks", and unlocking needs a password typed by a person.
# This script never types one: L10 forbids retrying, `pam_faillock` counts to
# three, and a wrong password left behind by a harness is a tally nobody
# asked for. So each case asserts everything up to the field -- that the
# locker reported the failure, fell back, reached `locked` and presented a
# frame -- and the attended step the script prints at the end is where the
# unlock half gets its evidence.
#
# KEEP THE NESTED WINDOW IN FRONT WHILE THIS RUNS. Hyprland does not render
# a window on a workspace you are not looking at, and a nested compositor
# that is not being rendered never finishes the `ext-session-lock-v1`
# handshake -- so the locker sits at `waiting for the compositor's locked`
# for ever and every assertion about it comes out false. On 2026-11-10 that
# printed six crosses against L7 and nothing was wrong with L7. Cases now
# report VOID rather than FAIL when they never reached `locked`, which is the
# reader that was missing, but the round still needs that window visible to
# produce an answer at all.
#
# usage:
#   Hyprland -c spike/p3/nested.lua          # from the session you are in
#   nu spike/p3/failure-round.nu wayland-2
#
# It refuses to run against the compositor you are sitting in, for the reason
# `lock-measure.nu` refuses: `rehearse` locks whatever it is pointed at.

# The instance signature of the nested compositor, which is how `hyprctl` is
# told which Hyprland to talk to. Taken from the socket name rather than from
# `--instance <n>`: the index is an ordering and the ordering is not ours.
def nested-signature [display: string]: nothing -> string {
    let out = (^hyprctl instances | lines)
    mut sig = ""
    for line in $out {
        if ($line | str starts-with "instance ") {
            $sig = ($line | str replace "instance " "" | str trim --right --char ':')
        }
        if ($line | str contains $"wl socket: ($display)") { return $sig }
    }
    ""
}

# Kill ONE locker, by one pid. `kill $(pgrep ...)` with two matches is how an
# earlier round of this work spent ten minutes killing nothing.
def kill-locker []: nothing -> bool {
    let found = (ps | where name == "auroval-lock" | get pid)
    if ($found | is-empty) { return false }
    ^kill -9 ($found | first | into string)
    sleep 1sec
    true
}

# The compositor's own way out of a held lock whose client is gone.
def clear-lock [sig: string] {
    ^hyprctl --instance $sig eval 'hl.clear_crashed_lockscreen()' | ignore
    sleep 1sec
}

def check [log: string, pattern: string, want: bool, what: string]: nothing -> record {
    let hit = ((open $log | lines | where ($it =~ $pattern) | length) > 0)
    {what: $what, ok: ($hit == $want), saw: $hit}
}

# Presence is not the claim L7 makes. "The loop was still turning AFTER the
# context went" is an ordering, and two greps that both say yes can still be
# describing a log where the second line came first. So this reads the line
# numbers.
def check-order [log: string, first: string, then: string, what: string]: nothing -> record {
    let lines = (if ($log | path exists) { open $log | lines } else { [] })
    let i = ($lines | enumerate | where ($it.item =~ $first) | get -o 0.index)
    let j = ($lines | enumerate | where ($it.item =~ $then) | reverse | get -o 0.index)
    {what: $what, ok: ($i != null and $j != null and $j > $i), saw: ($i != null and $j != null)}
}

def main [display: string, --seconds: int = 14] {
    if $display == ($env.WAYLAND_DISPLAY? | default "") {
        print $"refusing: ($display) is this session's own compositor."
        print "`auroval-lock rehearse` would lock the screen you are reading this on."
        exit 2
    }
    if not ($"($env.XDG_RUNTIME_DIR)/($display)" | path exists) {
        print $"no such Wayland socket: ($env.XDG_RUNTIME_DIR)/($display)"
        exit 2
    }
    let bin = "./target/release/auroval-lock"
    if not ($bin | path exists) {
        print $"missing ($bin) -- `cargo build --workspace --release` first"
        exit 2
    }
    let sig = (nested-signature $display)
    if $sig == "" {
        print $"no Hyprland instance is serving ($display); only Hyprland is recoverable here."
        exit 2
    }

    let work = (mktemp -d -t 'auroval-fail-XXXXXX')
    let clip = "/usr/local/share/auroval/nightshade-01.mp4"
    let still = "/usr/local/share/auroval/nightshade-01.png"

    # A config that parses, naming a clip this round owns a copy of, so a case
    # may break the file without touching what the machine ships.
    let mine = $"($work)/clip.mp4"
    cp $clip $mine
    [ 'return {' '  background = {' $'    "($mine)",' $'    "($still)",' '  },' '}' ]
    | str join "\n" | save -f $"($work)/good.lua"

    # Line 2 is wrong on purpose and the report should say so.
    [ 'return {' '  background = { oops' '}' ] | str join "\n" | save -f $"($work)/broken.lua"

    # Parses, and unreadable to its owner. Root would walk straight through
    # this; the locker does not run as root.
    "return { background = {} }" | save -f $"($work)/unreadable.lua"
    ^chmod 000 $"($work)/unreadable.lua"

    # An empty plugin path with a registry of its own: GStreamer caches what
    # it found last time, so pointing only the paths at nothing still finds
    # every decoder through the cache in ~/.cache/gstreamer-1.0.
    mkdir $"($work)/no-plugins"

    let cases = [
        {
            name: "L6  config with a syntax error"
            cfg: $"($work)/broken.lua"
            env: {}
            break_clip: false
            want: [
                [pattern want what];
                ['auroval-lock: .*broken\.lua' true  "names the file it could not read"]
                ['locked at'                   true  "still reached `locked`"]
                ['first locked frame at'       true  "still presented a frame"]
            ]
        }
        {
            name: "L6  config absent"
            cfg: $"($work)/not-here.lua"
            env: {}
            break_clip: false
            want: [
                [pattern want what];
                ['locked at'             true "reached `locked` with no config at all"]
                ['first locked frame at' true "presented a frame"]
            ]
        }
        {
            name: "L6  config unreadable (mode 000)"
            cfg: $"($work)/unreadable.lua"
            env: {}
            break_clip: false
            want: [
                [pattern want what];
                ['auroval-lock: .*unreadable\.lua' true "names the file it could not read"]
                ['locked at'                       true "still reached `locked`"]
                ['first locked frame at'           true "still presented a frame"]
            ]
        }
        {
            name: "L5  no GStreamer plugins"
            cfg: $"($work)/good.lua"
            env: {
                GST_PLUGIN_SYSTEM_PATH: $"($work)/no-plugins"
                GST_PLUGIN_PATH: $"($work)/no-plugins"
                GST_REGISTRY: $"($work)/registry.bin"
                GST_REGISTRY_UPDATE: "no"
            }
            break_clip: false
            want: [
                [pattern want what];
                ['\[background\].*(trying|falling back)' true "said what it fell back to"]
                ['locked at'                             true "still reached `locked`"]
                ['first locked frame at'                 true "still presented a frame"]
            ]
        }
        {
            name: "L7  the GL context destroyed under a working lock"
            cfg: $"($work)/good.lua"
            env: {
                # Three frames in, so the lock was working and a frame had
                # reached the screen before anything was taken away.
                AUROVAL_LOCK_BREAK_GL: "3"
                # The per-second line is the evidence that the loop kept
                # turning once drawing became impossible -- `draws 0 of N`.
                AUROVAL_LOG_DRAWS: "1"
            }
            break_clip: false
            want: [
                [pattern want what];
                ['locked at'                             true  "reached `locked` before anything broke"]
                ['first locked frame at'                 true  "presented a frame first"]
                ['destroying the GL context on purpose'  true  "the hook fired"]
                ['the GL context is gone'                true  "noticed, and said so once"]
                ['draws 0 of [0-9]+ turns'               true  "kept turning the loop with nothing to draw"]
                ['AUROVAL_LOCK_SECONDS elapsed'          true  "reached its own deadline after the loss"]
                ['panic'                                 false "did not panic"]
            ]
            order: [
                [first then what];
                ['the GL context is gone' 'draws 0 of [0-9]+ turns' "the turning came AFTER the loss"]
                ['the GL context is gone' 'AUROVAL_LOCK_SECONDS elapsed' "the deadline came AFTER the loss"]
            ]
        }
        {
            name: "L5  clip truncated mid-playback"
            cfg: $"($work)/good.lua"
            env: {}
            break_clip: true
            want: [
                [pattern want what];
                ['locked at'             true  "reached `locked` before the file went"]
                ['first locked frame at' true  "presented a frame"]
                ['panic'                 false "did not panic when the stream ended"]
            ]
        }
    ]

    print $"compositor   ($display)  instance ($sig | str substring 0..8)…"
    print $"work         ($work)"
    print $"window       ($seconds) s per case"
    print ""

    mut results = []
    for case in $cases {
        let log = $"($work)/(($case.name | str replace --all ' ' '-')).log"
        let env_ = ({
            WAYLAND_DISPLAY: $display
            AUROVAL_CONFIG: $case.cfg
            AUROVAL_LOCK_REHEARSAL: "1"
            AUROVAL_LOCK_SECONDS: ($seconds | into string)
        } | merge $case.env)
        let job = (job spawn {||
            do -i { with-env $env_ { ^timeout ($seconds + 20) $bin rehearse } err> $log }
        })

        # Let it get all the way to a lock surface before anything is broken
        # under it: a clip that fails before `locked` is the previous case.
        if $case.break_clip {
            mut locked = false
            for _ in 1..40 {
                sleep 500ms
                if (($log | path exists) and ((open $log | lines | where ($it =~ 'locked at') | length) > 0)) {
                    $locked = true
                    break
                }
            }
            sleep 3sec
            # Truncate rather than delete. Unlinking an open file changes
            # nothing a reader can see -- the descriptor still points at the
            # data -- so a round that deleted it would prove only that Linux
            # works. Truncating takes the data away from a pipeline that is
            # still reading it.
            ^truncate -s 0 $mine
        }

        for _ in 1..($seconds + 25) {
            if ((ps | where name == "auroval-lock" | length) == 0) { break }
            sleep 1sec
        }
        let leftover = (kill-locker)
        clear-lock $sig
        cp $clip $mine

        let checks = (
            ($case.want | each {|w| (check $log $w.pattern $w.want $w.what) })
            | append (($case.order? | default []) | each {|o| (check-order $log $o.first $o.then $o.what) })
        )
        # A case that never reached `locked` has not failed its requirement;
        # it has not asked it. Said once and loudly, because the first time
        # this happened it printed six ✗ against L7 and the requirement was
        # fine -- the nested compositor had stopped answering.
        #
        # Two ways that happens, both found on 2026-11-10 and neither the
        # locker's doing:
        #
        #   * the nested window is not on the workspace you are looking at,
        #     so Hyprland stops rendering it, so it never finishes the lock
        #     handshake. The round only works while that window is VISIBLE.
        #   * the compositor is still holding a lock from an earlier run. The
        #     locker is then sent `finished` and exits 3, which this names
        #     separately because it is L8's behaviour working correctly.
        let asked = (((check $log 'locked at' true "") | get saw)
            or ((check $log 'first locked frame at' true "") | get saw))
        let declined = ((check $log '`finished`' true "") | get saw)
        let verdict = (if $asked {
            if (($checks | where not ok | length) == 0) { "PASS" } else { "FAIL" }
        } else if $declined {
            "VOID"
        } else {
            "VOID"
        })
        let passed = ($verdict == "PASS")
        print $"-- ($case.name)   ($verdict)"
        if not $asked {
            print "   !  this case never reached `locked`, so it asked its requirement"
            print "      NOTHING. Not a failure of the locker."
            if $declined {
                print "      The compositor sent `finished` (exit 3): it is still holding a"
                print "      lock from an earlier run. Clear it and run again:"
                print $"        hyprctl --instance ($sig) eval 'hl.clear_crashed_lockscreen\(\)'"
            } else {
                print "      The compositor never answered at all. The usual cause is the"
                print "      nested window not being on the workspace you are looking at --"
                print "      Hyprland stops rendering it, so it never completes the"
                print "      handshake. Put that window in front and run again."
            }
        }
        for c in $checks {
            print $"   (if $c.ok { '✓' } else { '✗' })  ($c.what)"
        }
        if $leftover {
            print "   !  it was still running at the deadline and was killed"
        }
        print (open $log | lines
            | where ($it =~ '^auroval-lock: ' or $it =~ '^\[background\]')
            | where ($it !~ 'REHEARSAL|There is NO escape|No signal unlocks|font |locale ')
            | each {|l| $"      ($l)" }
            | str join "\n")
        print ""
        $results = ($results | append {case: $case.name, result: $verdict})
    }

    # The recovery sequence, asserted rather than remembered. This is the
    # one case whose finding is about the compositor and not the locker.
    print "-- recovery  a locker killed while it holds the lock"
    let rlog = $"($work)/recovery.log"
    let renv = {
        WAYLAND_DISPLAY: $display
        AUROVAL_CONFIG: $"($work)/good.lua"
        AUROVAL_LOCK_REHEARSAL: "1"
        AUROVAL_LOCK_SECONDS: "600"
    }
    # `do -i`: the kill below is this script's own, and a job that reported
    # `terminated by SIGKILL` as an error was reporting the plan working.
    let rjob = (job spawn {|| do -i { with-env $renv { ^timeout 120 $bin rehearse } err> $rlog } })
    mut locked = false
    for _ in 1..60 {
        sleep 500ms
        if (($rlog | path exists) and ((open $rlog | lines | where ($it =~ 'locked at') | length) > 0)) {
            $locked = true
            break
        }
    }
    print $"   (if $locked { '✓' } else { '✗' })  it held the lock"
    kill-locker | ignore
    sleep 2sec

    # Can anything take over? This is the supervisor question, and the answer
    # decides whether a restart-on-crash unit is worth writing.
    let second = (with-env $renv { ^timeout 40 $bin rehearse | complete })
    let slog = ($second.stderr)
    let declined = ($slog =~ 'finished')
    print $"   (if $declined { '✓' } else { '✗' })  a second locker is DECLINED — no supervisor can rescue this"
    print $"   (if ($second.exit_code != 0) { '✓' } else { '✗' })  it exits non-zero on `finished` — L8 asks for this; exit was ($second.exit_code)"

    clear-lock $sig
    let after = (with-env {WAYLAND_DISPLAY: $display} { ^$bin probe | complete })
    let cleared = ($after.stdout =~ 'the protocol is available')
    print $"   (if $cleared { '✓' } else { '✗' })  `hl.clear_crashed_lockscreen\(\)` released it"
    print ""

    print "summary:"
    print ($results | table)
    print ""
    print "What no unattended round can show: that any of these screens UNLOCKS."
    print "That needs a password typed by a person. The attended step, once:"
    print ""
    print $"  Hyprland -c spike/p3/nested.lua"
    print $"  with-env {WAYLAND_DISPLAY: wayland-2, AUROVAL_CONFIG: ($work)/broken.lua,"
    print "            AUROVAL_LOCK_REHEARSAL: '1'} { ./target/release/auroval-lock rehearse }"
    print ""
    print "Type your password into the broken-config screen. It should unlock."
    print "If it does not, the way out is from another tty:"
    print $"  hyprctl --instance ($sig) eval 'hl.clear_crashed_lockscreen\(\)'"
}
