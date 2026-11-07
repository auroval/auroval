#!/usr/bin/env nu
#
# P0 spike, phase 3: a real greetd login, on a real VT, with the video playing.
#
# Phase 1 measured decode as greetd's own user. Phase 2 showed video on screen
# under `cage`. Neither proved the two together, because a nested `cage` has no
# `GREETD_SOCK`, so the auth loop was always exercised separately. This closes
# that gap and is the last unticked box on issue #1.
#
# greetd is a daemon, not a one-shot. When a session ends it starts a FRESH
# greeter and switches back to its own VT — which is correct behaviour for a
# display manager and makes a naive test unusable: every time you switch away,
# the next greeter yanks you back. So this script runs greetd in the background,
# waits for the authenticated session to leave its proof, and then stops greetd
# itself. It never relies on greetd exiting of its own accord, because it will
# not.
#
# What this does NOT do, deliberately:
#   * it does not touch /etc/greetd/config.toml — it writes its own config
#   * it does not enable or start the greetd service
#   * it does not disable, stop or reconfigure SDDM
#   * it does not remove hyprlock
# SDDM stays the way in for the whole test. greetd is run in the foreground on
# a spare VT and dies when you Ctrl-C it or when the test session ends.
#
# Escape route: your own session stays on VT 1. Ctrl+Alt+F1 returns to it at any
# point, whatever the greeter is doing.
#
# Run with:  sudo nu spike/p0/tty-login-test.nu --user dirk
# Clean up:  sudo nu spike/p0/tty-login-test.nu --clean

const STAGE = "/var/lib/auroval-spike"
const CONFIG = "/etc/greetd/auroval-spike.toml"

def main [
    --user: string = "dirk"   # the account you will log in as on the test VT
    --vt: int = 3             # which VT to put the greeter on; must be free
    --wait: int = 180         # seconds to allow for the login before giving up
    --clean                   # remove the spike's own config and exit
] {
    if $clean {
        if ($CONFIG | path exists) { rm -f $CONFIG }
        print $"removed ($CONFIG) — /etc/greetd/config.toml was never touched"
        return
    }

    if (id -u | into int) != 0 {
        print "must run as root (sudo nu spike/p0/tty-login-test.nu)"
        exit 2
    }

    # Refuse to run on an occupied VT. Taking the VT out from under a live
    # session is exactly the kind of mistake this test must not make.
    let active = (open /sys/class/tty/tty0/active | str trim)
    if $active == $"tty($vt)" {
        print $"VT ($vt) is the active console. Pick a free one with --vt."
        exit 2
    }
    let occupied = (
        loginctl list-sessions --no-legend
        | lines
        | each { |l| $l | split row -r '\s+' }
        | where { |f| ($f | any { |x| $x == $"tty($vt)" }) }
    )
    if ($occupied | is-not-empty) {
        print $"VT ($vt) already has a login session. Pick a free one with --vt."
        exit 2
    }

    # Phase 1's staging has to be in place; this reuses it rather than
    # duplicating the copying logic.
    for f in [$"($STAGE)/spike-p0" $"($STAGE)/media"] {
        if not ($f | path exists) {
            print $"($f) is missing — run spike/p0/greeter-decode-test.nu first"
            exit 2
        }
    }
    if (glob $"($STAGE)/media/*h264*.mp4" | is-empty) {
        print $"no H.264 clip in ($STAGE)/media — run the phase 1 script first"
        exit 2
    }

    let repo = ($env.FILE_PWD | path expand)
    cp $"($repo)/greeter-launch.sh" $"($STAGE)/greeter-launch.sh"
    cp $"($repo)/session-proof.sh" $"($STAGE)/session-proof.sh"
    ^chmod 0755 $"($STAGE)/greeter-launch.sh" $"($STAGE)/session-proof.sh"

    let cache = $"($STAGE)/cache"
    mkdir $cache
    ^chown -R greeter:greeter $cache

    # greetd needs the greeter in `video` to get DRM master for cage. Arch's
    # package already does this; check rather than assume, and say so either way.
    let groups = (^id -nG greeter | str trim | split row " ")
    if not ("video" in $groups) {
        print $"the `greeter` user is not in `video` \(groups: ($groups | str join ', ')\)."
        print "cage will not get DRM master. Add it with:"
        print "  sudo usermod -aG video greeter"
        exit 2
    }
    print $"greeter groups: ($groups | str join ', ') — `video` present, cage can take DRM master"

    # Our own config file. /etc/greetd/config.toml is left exactly as shipped.
    let cfg = $"[terminal]
vt = ($vt)

[default_session]
command = \"/var/lib/auroval-spike/greeter-launch.sh\"
user = \"greeter\"
"
    $cfg | save -f $CONFIG
    ^chmod 0644 $CONFIG
    print $"wrote ($CONFIG):"
    print $cfg

    let log = $"($STAGE)/tty-login.log"
    let proof = $"/home/($user)/auroval-p0-login-proof.txt"
    if ($proof | path exists) { rm -f $proof }

    print ""
    print "────────────────────────────────────────────────────────────────"
    print $"greetd is about to start on VT ($vt). Then:"
    print ""
    print $"  1. switch to it with  Ctrl+Alt+F($vt)"
    print $"  2. the background should be moving — that is the point"
    print $"  3. type ($user)'s password and press Enter"
    print "  4. you land on a plain screen; the proof file is written there"
    print $"  5. switch back with Ctrl+Alt+F1 — greetd stops by itself once"
    print "     the proof appears, so the console will not be grabbed again"
    print ""
    print "SDDM is untouched and still owns VT 1. Ctrl-C here aborts."
    print "────────────────────────────────────────────────────────────────"
    print ""

    # greetd is started detached, with its output going straight to the log
    # file rather than being buffered until exit. An earlier version captured
    # with `complete`, which meant aborting the test threw away every line the
    # spike had printed — exactly what happened the first time it was run.
    #
    # `sh -c` is used for the one thing nushell has no clean equivalent for:
    # detaching a process with its streams redirected to a file.
    ^sh -c $"setsid greetd --config ($CONFIG) > ($log) 2>&1 < /dev/null &"
    sleep 2sec

    if (^pgrep -x greetd | complete).exit_code != 0 {
        print "greetd did not start. Log:"
        if ($log | path exists) { open $log | print }
        exit 1
    }
    print $"greetd started on VT ($vt); output streaming to ($log)"
    print $"waiting up to ($wait) seconds for a completed login…"
    print ""

    # Poll for the proof file the authenticated session writes. The moment it
    # appears the login has succeeded, and greetd can be stopped before it
    # restarts the greeter and grabs the console again.
    mut waited = 0
    mut ok = false
    while $waited < $wait {
        if ($proof | path exists) { $ok = true; break }
        sleep 1sec
        $waited = $waited + 1
    }

    # Stop greetd either way. This is the step whose absence made the first run
    # switch the console back to the greeter every few seconds.
    ^pkill -x greetd
    sleep 1sec
    if (^pgrep -x greetd | complete).exit_code == 0 { ^pkill -9 -x greetd }
    print $"greetd stopped after ($waited)s."

    let out = (if ($log | path exists) { open $log } else { "" })
    print ""
    print "── the spike's own lines ──"
    $out | lines | where { |l| $l =~ '^\[spike\]' } | each { |l| print $l }

    print ""
    if ($proof | path exists) {
        print $"── PROOF: login completed, from ($proof) ──"
        open $proof | print
    } else {
        print $"no proof file at ($proof) — the login did not complete."
        print "The log above should say why."
    }

    print ""
    print $"remove the spike's greetd config with: sudo nu spike/p0/tty-login-test.nu --clean"
}
