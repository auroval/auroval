#!/usr/bin/env nu
#
# R16, read from production rather than from a harness.
#
# Every number in R16's row so far comes from `cage-measure.nu`: the real
# binary under a real cage on a real VT, ten times, median. That is the right
# instrument for setting a budget and the wrong one for knowing whether the
# budget is being met, because nobody runs it on a Monday. The thing that
# actually starts this greeter is `greetd`, and since 2026-11-08 the startup
# script pipes it through `systemd-cat --identifier=auroval-greet`, so every
# real start has already written its own first-frame line to the journal:
#
#     auroval-greet: first frame 7 ms, surface 2560x1440
#
# This reads them back. It needs no VT, no root and no cage -- it is a view of
# starts that already happened, which is the only kind of measurement that
# survives contact with a Monday.
#
# WHAT IT IS NOT. These are single starts, not medians, and each one is
# whatever the machine was doing at the time. A number here that is high is a
# question, not a verdict; the answer is `cage-measure.nu` on a free VT. What
# this is good for is the opposite case: a long run of ordinary starts, which
# is evidence `cage-measure.nu` cannot give because it never runs at boot.
#
# The cold case is the one to watch. R16's budget governs the warm path and
# the cold path is over it by design (#77): after a gstreamer update the
# registry is invalid and the first start pays for the rescan. `install.nu`
# warms it, so a cold number here means either an update landed since the last
# boot, or the install's warming did not take.
#
#   nu spike/p1e/first-frames.nu              # this boot
#   nu spike/p1e/first-frames.nu --since '7 days ago'

def main [
    --since: string = ""   # journalctl --since; the default is this boot
] {
    let args = if $since == "" { ["-b"] } else { ["--since" $since] }
    let out = (^journalctl -t auroval-greet -o short-iso --no-pager ...$args
        | complete)
    if $out.exit_code != 0 {
        print "journalctl refused. The greeter's lines are tagged `auroval-greet`,"
        print "which only exists on a machine where deploy/auroval-greet.sh is installed."
        exit 2
    }

    let starts = ($out.stdout | lines
        | where ($it =~ 'auroval-greet: first frame ')
        | each {|l|
            let when = ($l | split row ' ' | first)
            # "first frame 7 ms, surface 2560x1440"
            let ms = ($l | parse --regex 'first frame (?<ms>[0-9.]+) ms' | get ms.0? | default "")
            let surface = ($l | parse --regex 'surface (?<s>[0-9]+x[0-9]+)' | get s.0? | default "—")
            {
                when: $when
                ms: (if $ms == "" { -1.0 } else { ($ms | into float) })
                surface: $surface
            }
        })

    if ($starts | is-empty) {
        print "No greeter start in this window wrote a first-frame line."
        print ""
        print "That is itself a finding on a machine running greetd: either the"
        print "greeter has not started since the window began, or it is not going"
        print "through deploy/auroval-greet.sh -- the systemd-cat wrapper in that"
        print "script is the only reason these lines reach the journal at all. It"
        print "was added 2026-11-08, so a start older than that has nothing here."
        exit 0
    }

    # R16's own floor, from its row: 100 ms is a fault worth finding long
    # before the 500 ms a person would notice.
    let floor = 100.0
    print ($starts | each {|s|
        $s | insert verdict (
            if $s.ms < 0 { "unreadable" } else if $s.ms <= $floor { "ok" } else { "LOOK" }
        )
    } | table)
    print ""

    let good = ($starts | where ms >= 0)
    if not ($good | is-empty) {
        let worst = ($good | math max | get ms)
        print $"($good | length) start\(s); worst ($worst) ms against R16's 100 ms regression floor and 500 ms budget."
        if $worst > $floor {
            print "A start over the floor is a question for `cage-measure.nu` on a free VT,"
            print "and #77 is where the cold path's 1874 ms is already on record."
        }
    }
}
