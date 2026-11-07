#!/usr/bin/env nu
#
# P1/E — start greetd with auroval's own config, on a free VT, alongside SDDM.
#
# What this does NOT do, deliberately:
#   * it does not touch /etc/greetd/config.toml
#   * it does not enable or start the greetd.service unit
#   * it does not stop, disable or reconfigure SDDM
#   * it does not change which display manager this machine boots into
# SDDM stays the way in. That is the last box of P1 and it is not this
# script's to tick.
#
# greetd is a daemon, not a one-shot, and that is the trap this script exists
# to avoid. When a session ends it starts a FRESH greeter and switches the
# console back to its own VT — correct behaviour for a display manager, and
# unusable in a test: an earlier run of the P0 spike kept yanking the console
# back every few seconds. So greetd is started detached with a deadline, and
# this script stops it. It never waits for greetd to exit on its own, because
# it will not.
#
# Needs root: only root can open a VT and talk to PAM. Run it yourself:
#
#     sudo nu spike/p1e/greetd-session.nu --vt 3 --seconds 60
#
# Escape route: your desktop session is on its own VT and is untouched.
# Ctrl+Alt+F1 returns to it at any point, whatever the greeter is doing, and
# greetd stops by itself when the deadline passes.

const CONFIG = "/etc/greetd/auroval.toml"
const LOG    = "/var/log/auroval-greetd.log"

def main [
    --vt: int = 3          # a FREE console; 1 and 2 belong to SDDM and you
    --seconds: int = 60    # how long greetd may run before it is stopped
    --config: string = ""  # override the config file
] {
    let config = if ($config | is-empty) { $CONFIG } else { $config }
    let total = 5

    if (id -u | into int) != 0 {
        print "must run as root: sudo nu spike/p1e/greetd-session.nu"
        exit 2
    }

    print $"  1/($total)  config"
    if not ($config | path exists) {
        print $"         ($config) is missing — run: nu deploy/install.nu"
        print "         then the root script it prints."
        exit 2
    }
    # The one thing worth refusing outright. If this file ever becomes
    # greetd's default config, greetd becomes the display manager by accident.
    if ($config | path expand) == "/etc/greetd/config.toml" {
        print "         that is greetd's own config. This project never writes it."
        exit 2
    }
    print $"         ($config)"

    print $"  2/($total)  the VT"
    # Taking a VT out from under a live session is exactly the mistake this
    # must not make, so both checks, not just the cheap one.
    let active = (open /sys/class/tty/tty0/active | str trim)
    if $active == $"tty($vt)" {
        print $"         VT ($vt) is the active console. Pick a free one with --vt."
        exit 2
    }
    let occupied = (
        ^loginctl list-sessions --no-legend | lines
        | each { |l| $l | split row -r '\s+' }
        | where { |f| ($f | any { |x| $x == $"tty($vt)" }) }
    )
    if ($occupied | is-not-empty) {
        print $"         VT ($vt) already has a login session. Pick a free one with --vt."
        exit 2
    }
    let want = (open $config | parse -r '(?m)^\s*vt\s*=\s*(\d+)' | get -o capture0.0)
    if ($want | is-not-empty) and $want != ($vt | into string) {
        print $"         ($config) says vt = ($want) but you asked for ($vt)."
        print $"         Edit the file, or re-run deploy/install.nu --vt ($vt)."
        exit 2
    }
    print $"         VT ($vt) is free"

    print $"  3/($total)  the greeter"
    let cmd = (open $config | parse -r '(?m)^\s*command\s*=\s*"([^"]+)"' | get -o capture0.0 | default "")
    if ($cmd | is-empty) or not ($cmd | path exists) {
        print $"         the config's command \(($cmd)) does not exist — run deploy/install.nu"
        exit 2
    }
    for d in ["/var/lib/auroval" "/var/cache/auroval"] {
        if not ($d | path exists) {
            print $"         ($d) is missing — R4's memory or C15's cache will fail"
        }
    }
    print $"         ($cmd)"

    print $"  4/($total)  greetd, for ($seconds) s on VT ($vt)"
    print ""
    print "────────────────────────────────────────────────────────────────"
    print $"  switch to it with  Ctrl+Alt+F($vt)"
    print $"  come back with     Ctrl+Alt+F1"
    print $"  greetd stops by itself in ($seconds) s; Ctrl-C here stops it now"
    print "  SDDM is untouched and still owns the console you came from."
    print "────────────────────────────────────────────────────────────────"
    print ""
    # Detached, with output going straight to a file rather than being
    # buffered until exit. An earlier spike captured with `complete` and
    # aborting the run threw away every line it had printed.
    ^sh -c $"setsid greetd --config ($config) > ($LOG) 2>&1 < /dev/null &"
    sleep 2sec
    if (^pgrep -x greetd | complete).exit_code != 0 {
        print "greetd did not start. Log:"
        if ($LOG | path exists) { open $LOG | print }
        exit 1
    }
    print $"greetd running; output streaming to ($LOG)"

    mut waited = 2
    while $waited < $seconds {
        sleep 1sec
        $waited = $waited + 1
        if ($waited mod 10) == 0 {
            print $"  ($waited)/($seconds) s"
        }
    }

    print $"  5/($total)  stopping greetd"
    # The step whose absence made the first P0 run switch the console back to
    # the greeter every few seconds. Unconditional, and it checks.
    ^pkill -x greetd
    sleep 1sec
    if (^pgrep -x greetd | complete).exit_code == 0 { ^pkill -9 -x greetd }
    print "         stopped. The console is yours again."

    print ""
    if ($LOG | path exists) {
        print $"── ($LOG) ──"
        open $LOG | print
    }
}
