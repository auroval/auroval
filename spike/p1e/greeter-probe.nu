#!/usr/bin/env nu
#
# P1/E — run `auroval-greet probe` as the `greeter` user, which is the only
# account whose answer counts.
#
# `probe` run as you says almost nothing about deployment: you own a home, a
# cache and a session, and the greeter owns none of those. Run as `greeter`
# with `env -i`, it answers the two questions this piece exists to settle:
#
#   * is /var/lib/auroval writable — R4's memory, which currently reports
#     `writable no — Permission denied (os error 13)` and will keep doing so
#     until /etc/tmpfiles.d/auroval.conf is installed
#   * does the C15 cache exist and belong to the greeter
#
# This is the P0 method (`runuser -u greeter` with `env -i`, so no session, no
# XDG_RUNTIME_DIR, no D-Bus, no driver hints) pointed at the real binary.
#
# It needs root, because only root can become another user. Run it yourself:
#
#     sudo nu spike/p1e/greeter-probe.nu
#
# It touches nothing. It reads, it runs one binary read-only except for the
# store's own write probe, and it prints.

const BIN   = "/usr/local/lib/auroval/auroval-greet"
const CACHE = "/var/cache/auroval"
const STATE = "/var/lib/auroval"

def main [--bin: string = ""] {
    let bin = if ($bin | is-empty) { $BIN } else { $bin }
    if (id -u | into int) != 0 {
        print "must run as root: sudo nu spike/p1e/greeter-probe.nu"
        print "\(it only uses root to become `greeter`; it changes nothing\)"
        exit 2
    }
    let total = 4
    print $"  1/($total)  the greeter user"
    if (^id -u greeter | complete).exit_code != 0 {
        print "         there is no `greeter` user — is greetd installed?"
        exit 2
    }
    let groups = (^id -nG greeter | str trim | split row " ")
    print $"         groups: ($groups | str join ', ')"
    if not ("video" in $groups) {
        print "         `video` is MISSING — cage will not get DRM master."
        print "         Fix with: sudo usermod -aG video greeter"
    }

    print $"  2/($total)  the directories"
    for d in [$STATE $CACHE] {
        if not ($d | path exists) {
            print $"         ($d) does NOT exist — install /etc/tmpfiles.d/auroval.conf"
            continue
        }
        let own = (^stat -c '%U:%G %a' $d | str trim)
        let ok = ($own | str starts-with "greeter:greeter")
        print $"         ($d)  ($own)  (if $ok { 'owned by greeter' } else { 'WRONG OWNER' })"
    }

    print $"  3/($total)  the binary"
    if not ($bin | path exists) {
        print $"         ($bin) is missing — run: nu deploy/install.nu"
        exit 2
    }
    print $"         ($bin)"

    print $"  4/($total)  probe, as greeter, with an empty environment"
    # `env -i` on purpose: anything inherited from your root shell would be a
    # variable the real greeter does not have, and the whole value of this
    # probe is that it lies about nothing.
    let env_args = [$"XDG_CACHE_HOME=($CACHE)" $"GST_REGISTRY=($CACHE)/gst-registry.bin"]
    let r = (do { ^runuser -u greeter -- env -i ...$env_args $bin probe } | complete)
    print ""
    print ($r.stdout)
    if ($r.stderr | str trim | is-not-empty) { print $r.stderr }

    print ""
    if ($r.stdout | str contains "writable     yes") {
        print "R4's memory is writable by the greeter. That box can be ticked."
    } else {
        print "R4's memory is NOT writable by the greeter. /var/lib/auroval is"
        print "missing or wrongly owned; see /etc/tmpfiles.d/auroval.conf."
    }
}
