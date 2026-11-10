#!/usr/bin/env nu
#
# Issue #77 — the four behaviours of deploy/auroval-warm-registry.sh, checked
# without root and without pacman.
#
# The script itself cannot be run here: it writes /var/cache/auroval as the
# `greeter` user, which needs uid 0. So each case runs a COPY with the two
# privilege checks removed and `CACHE` pointed at a scratch directory, and
# says so. What that leaves untested is `runuser` and the ownership, which is
# one line each and visible in the install's own output. What it does test is
# everything that decides whether the registry ends up warm, including the two
# readers that exist because an exit code is not evidence.
#
# NO SUDO. NO PACMAN. It builds nothing and installs nothing.
#
#   nu spike/p1e/warm-registry-check.nu

def variant [src: string, scratch: string, edits: list<record<from: string, to: string>>]: nothing -> string {
    mut s = (open --raw $src)
    $s = ($s | str replace "CACHE=/var/cache/auroval" $"CACHE=($scratch)/cache")
    # The two things a non-root caller cannot satisfy, and nothing else.
    $s = ($s | str replace '[ "$(id -u)" = 0 ] || fail "this has to run as root; it writes as the greeter user"' ":")
    $s = ($s | str replace 'id greeter >/dev/null 2>&1 || fail "there is no greeter user"' ":")
    for e in $edits { $s = ($s | str replace $e.from $e.to) }
    let out = $"($scratch)/variant.sh"
    $s | save -f $out
    $out
}

def run-case [name: string, script: string]: nothing -> record {
    let r = (do { ^sh $script } | complete)
    { case: $name, code: $r.exit_code, first: ($r.stderr | lines | get -o 0 | default "") }
}

def main [] {
    let repo = ($env.FILE_PWD | path dirname | path dirname)
    let src = $"($repo)/deploy/auroval-warm-registry.sh"
    if not ($src | path exists) {
        print $"($src) is missing"
        exit 2
    }
    if (which gst-inspect-1.0 | is-empty) {
        print "gst-inspect-1.0 is not installed, so case 2 cannot run"
        exit 2
    }
    let scratch = (mktemp -d)
    mkdir $"($scratch)/cache" $"($scratch)/plugins"
    print $"scratch ($scratch)"

    mut results = []
    let total = 4

    # 1 — not applicable. No /var/cache/auroval means auroval's greeter was
    # never installed here, and pacman must not be told anything is wrong.
    print $"  1/($total)  no cache directory — expect exit 0, nothing on stderr"
    let s1 = (variant $src $"($scratch)/absent-cache" [])
    mkdir $"($scratch)/absent-cache"
    $results = ($results | append (run-case "not installed here" $s1))

    # 2 — the real thing, as far as it can go without uid 0: a cold scan into
    # an empty cache. This is the second that the hook exists to spend here
    # rather than at a login.
    print $"  2/($total)  a cold scan — expect exit 0 and a registry on disk"
    let s2 = (variant $src $scratch [
        { from: "runuser -u greeter -- env \\", to: "env \\" }
    ])
    let t0 = (date now)
    $results = ($results | append (run-case "a cold scan warms it" $s2))
    let ms = (((date now) - $t0) | into int) / 1000000
    let reg = $"($scratch)/cache/gst-registry.bin"
    let wrote = ($reg | path exists) and ((ls $reg | get 0.size) > 0b)
    print $"           ($ms) ms, registry written: ($wrote)"

    # 3 — the freshness reader. A registry that did not get rewritten is the
    # failure this whole script exists to prevent, and it is invisible: a
    # stale registry and a fresh one look identical until someone logs in.
    print $"  3/($total)  a plugin newer than the registry — expect exit 1"
    touch $"($scratch)/plugins/libgstfake.so"
    let s3 = (variant $src $scratch [
        { from: "PLUGINS=/usr/lib/gstreamer-1.0", to: $"PLUGINS=($scratch)/plugins" }
        { from: "runuser -u greeter -- env \\", to: "env \\" }
    ])
    $results = ($results | append (run-case "a stale registry is caught" $s3))

    # 4 — the other way an exit code lies: gst-inspect returns 0 having
    # written nothing at all.
    print $"  4/($total)  a scan that returns 0 and writes nothing — expect exit 1"
    rm -f $reg
    let s4 = (variant $src $scratch [
        { from: "runuser -u greeter -- env \\\n    XDG_CACHE_HOME=\"$CACHE\" \\\n    GST_REGISTRY=\"$REGISTRY\" \\\n    gst-inspect-1.0 >/dev/null 2>&1 \\", to: "true \\" }
    ])
    $results = ($results | append (run-case "an empty registry is caught" $s4))

    let want = [0 0 1 1]
    let table = ($results | enumerate | each {|r|
        let ok = ($r.item.code == ($want | get $r.index))
        { case: $r.item.case, exit: $r.item.code, want: ($want | get $r.index), verdict: (if $ok { "pass" } else { "FAIL" }) }
    })
    print ""
    print ($table | table)
    rm -rf $scratch
    if ($table | any { |r| $r.verdict == "FAIL" }) {
        print "a case did not behave as the script documents"
        exit 1
    }
    print "all four behave as deploy/auroval-warm-registry.sh documents"
}
