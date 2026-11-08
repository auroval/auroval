#!/usr/bin/env nu
#
# Stage auroval's greeter for a real greetd run, and print the root commands
# that finish the job.
#
# THIS SCRIPT NEVER RUNS sudo. That is a standing rule in this repository and
# it is not stylistic: a failed sudo attempt trips pam_faillock and can lock
# the maintainer out of his own machine. Everything that needs root is written
# into one script, and you run that script.
#
# What it does itself, as you:
#   * builds the greeter in release mode
#   * stages the files root will copy into a directory under target/
#   * writes ~/.local/bin/auroval-vt, the one-word wrapper for the harness,
#     because the clipboard does not survive a VT switch and a long command
#     is a command you will mistype on a console with no scrollback
#
# What it leaves for you to run, as root:
#   * /etc/tmpfiles.d/auroval.conf  + systemd-tmpfiles --create
#   * /usr/local/lib/auroval/{auroval-greet,auroval-greet.sh}
#   * /etc/greetd/auroval.toml
#
# None of that enables greetd or touches SDDM or /etc/greetd/config.toml.
# SDDM stays the way in until the last box of P1.
#
#   nu deploy/install.nu            # build, stage, print the root commands
#   nu deploy/install.nu --no-build # stage what is already built

def main [
    --no-build            # skip cargo, use whatever is in target/release
    --vt: int = 3         # VT the staged greetd config points at
] {
    let repo = ($env.FILE_PWD | path dirname)
    let deploy = $"($repo)/deploy"
    let stage = $"($repo)/target/auroval-stage"
    let bin = $"($repo)/target/release/auroval-greet"
    let total = 5
    print $"auroval — staging the greeter. No sudo is run by this script."
    print ""

    print $"  1/($total)  build"
    if $no_build {
        print "         skipped \(--no-build)"
    } else {
        cd $repo
        ^cargo build --release -p greet
    }
    if not ($bin | path exists) {
        print $"         ($bin) is missing."
        print "         Build it with: cargo build --release -p greet"
        print "         If the crate does not exist yet, PR #32 is the one that adds it."
        exit 2
    }
    print $"         ($bin)"

    print $"  2/($total)  stage"
    rm -rf $stage
    mkdir $stage
    cp $bin $"($stage)/auroval-greet"
    cp $"($deploy)/auroval-greet.sh" $"($stage)/auroval-greet.sh"
    cp $"($deploy)/auroval.tmpfiles.conf" $"($stage)/auroval.tmpfiles.conf"
    # The checked-in greetd config says vt = 3. Substitute rather than ask the
    # maintainer to edit a file he is about to copy with root privileges.
    open --raw $"($deploy)/greetd-auroval.toml"
    | str replace -r '(?m)^vt = \d+$' $"vt = ($vt)"
    | save -f $"($stage)/greetd-auroval.toml"
    ^chmod 0755 $"($stage)/auroval-greet" $"($stage)/auroval-greet.sh"
    print $"         ($stage)"

    print $"  3/($total)  check the greeter user"
    let has_greeter = ((^id -u greeter | complete).exit_code == 0)
    if not $has_greeter {
        print "         there is no `greeter` user — is greetd installed?"
        print "         the root script below will fail on the chown lines."
    } else {
        let groups = (^id -nG greeter | str trim | split row " ")
        print $"         greeter groups: ($groups | str join ', ')"
        if not ("video" in $groups) {
            # cage needs DRM master, which logind only hands to a member of
            # `video`. Arch's greetd package already does this; check rather
            # than assume, because the failure looks like a renderer bug.
            print "         `video` is MISSING — cage will not get DRM master."
            print "         Add it to the root script: usermod -aG video greeter"
        }
    }

    print $"  4/($total)  wrappers"
    # One word each, because the clipboard does not survive a VT switch and a
    # long path is a path you will mistype on a console with no scrollback.
    # That was not a hypothetical: the first console sitting stopped at
    # "how do I get that line onto the other VT".
    #
    # /bin/sh, and each cds to the repo first: nushell resolves a script's
    # siblings relative to the script, but cargo and target/ need the repo.
    let wrappers = [
        [name                 script                          what];
        ["auroval-vt"         "spike/p1e/cage-measure.nu"     "R16, the first-frame measurement"]
        ["auroval-keys"       "spike/p1ab/keyboard-round.nu"  "R6, key repeat and caps lock"]
        ["auroval-greetd"     "spike/p1ab/greetd-round.nu"    "R3 and R1, with greetd"]
    ]
    let bindir = ($"~/.local/bin" | path expand)
    for w in $wrappers {
        let wrapper = ($"~/.local/bin/($w.name)" | path expand)
        mkdir ($wrapper | path dirname)
        [
            "#!/bin/sh"
            $"# auroval — ($w.what). Written by deploy/install.nu."
            "# One word, because the clipboard does not survive a VT switch."
            $"cd ($repo) || exit 1"
            $"exec nu ($w.script) \"$@\""
        ] | append "" | str join "\n" | save -f $wrapper
        ^chmod 0755 $wrapper
        print $"         ($w.name)  — ($w.what)"
    }
    let on_path = (($env.PATH | path expand) | any { |p| $p == $bindir })
    if $on_path {
        print "         ~/.local/bin is on PATH — type the name on any VT."
    } else {
        print "         ~/.local/bin is NOT on PATH — type the full path instead:"
        print $"           ($bindir)/<name>"
    }

    print $"  5/($total)  root script"
    let root_sh = $"($repo)/target/auroval-install-root.sh"
    [
        "#!/bin/sh"
        "# Written by deploy/install.nu. Read it before you run it."
        "# It installs auroval's greeter files and NOTHING else: it does not"
        "# enable greetd, does not touch /etc/greetd/config.toml, and does not"
        "# stop, disable or reconfigure SDDM."
        "set -eux"
        ""
        "# C15 and R4: the two directories the greeter owns."
        $"install -m 0644 ($stage)/auroval.tmpfiles.conf /etc/tmpfiles.d/auroval.conf"
        "systemd-tmpfiles --create /etc/tmpfiles.d/auroval.conf"
        ""
        "# The greeter and its startup path."
        "install -d -m 0755 /usr/local/lib/auroval"
        $"install -m 0755 ($stage)/auroval-greet    /usr/local/lib/auroval/auroval-greet"
        $"install -m 0755 ($stage)/auroval-greet.sh /usr/local/lib/auroval/auroval-greet.sh"
        ""
        "# greetd's config for auroval, alongside its own, never replacing it."
        "install -d -m 0755 /etc/greetd"
        $"install -m 0644 ($stage)/greetd-auroval.toml /etc/greetd/auroval.toml"
    ] | append "" | str join "\n" | save -f $root_sh
    ^chmod 0755 $root_sh
    print $"         ($root_sh)"

    print ""
    print "Run this, and nothing else, as root:"
    print ""
    print $"  sudo sh ($root_sh)"
    print ""
    print "Then, on a free VT \(Ctrl+Alt+F3, log in as yourself):"
    print "  auroval-vt"
    print ""
    print "greetd is still not enabled. SDDM is still the way in. To see the"
    print "greeter under greetd once it can draw:"
    print "  sudo nu spike/p1e/greetd-session.nu --vt 3 --seconds 60"
    print "  \(that one does need root — greetd opens a VT and talks to PAM —"
    print "   but it starts greetd by hand with auroval's own config and stops"
    print "   it again. It never enables the service and never touches SDDM.)"
}
