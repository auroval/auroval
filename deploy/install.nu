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
    --vt: int = 1         # VT the staged greetd config points at
    --frame-dump: string = ""   # bake AUROVAL_FRAME_DUMP=<dir> into the startup script
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
    # greetd clears the environment, so a debug variable exported in the
    # shell that starts greetd never reaches the greeter. The only place it
    # can be set is the script greetd itself runs, which is why this is a
    # flag here rather than something to remember on the console.
    if $frame_dump != "" {
        # Checked and quoted, because this string is written into a file that
        # root installs and `greeter` then executes on every start. It used to
        # be interpolated bare: `--frame-dump '/tmp/x; curl ... | sh'` put that
        # semicolon straight into the startup script. A debugging flag is still
        # a flag somebody types in a hurry, and the blast radius here is the
        # login path.
        if ($frame_dump !~ '^/[A-Za-z0-9._/@+-]*$') {
            print "         --frame-dump must be an absolute path of letters,"
            print "         digits and . _ / @ + - — this one is not, and it"
            print "         would be written into a root-installed script."
            exit 2
        }
        let line = $"\n# --frame-dump: staged for one sitting. Not for a deployed greeter.\nAUROVAL_FRAME_DUMP='($frame_dump)'\nexport AUROVAL_FRAME_DUMP\n"
        let body = (open --raw $"($stage)/auroval-greet.sh")
        # The anchor is checked, because it stopped matching and nothing said
        # so. This was `str replace "exec cage"`, and on 2026-11-08 the
        # startup script gained `systemd-cat` in front of cage -- so the line
        # became `exec systemd-cat --identifier=auroval-greet \` and the
        # replacement silently matched nothing. `--frame-dump` has been
        # accepting a path, printing that it was baked in, and doing nothing
        # at all ever since. A substitution that cannot find its anchor must
        # fail, not shrug: that is the same class of bug as a constant that
        # used to be true, and it is three lines to make impossible.
        let anchor = ($body | lines | where ($it | str starts-with "exec ") | length)
        if $anchor != 1 {
            print $"         cannot stage --frame-dump: deploy/auroval-greet.sh has ($anchor)"
            print "         lines starting with `exec `, and this needs exactly one to"
            print "         insert before. Fix the script or this flag, not both."
            exit 2
        }
        $body
        | lines
        | each {|l| if ($l | str starts-with "exec ") { $"($line)\n($l)" } else { $l } }
        | str join "\n"
        | save -f $"($stage)/auroval-greet.sh"
    }
    cp $"($deploy)/auroval.tmpfiles.conf" $"($stage)/auroval.tmpfiles.conf"
    # The checked-in greetd config says vt = 1. Substitute rather than ask the
    # maintainer to edit a file he is about to copy with root privileges.
    #
    # The default is 1 because `greetd.service` carries
    # `Conflicts=getty@tty1.service`: the unit takes tty1 from its getty in
    # order to put a login screen there, and a config pointing elsewhere
    # leaves tty1 with neither. It was 3, inherited from the spikes, which run
    # alongside a display manager that owns 1 and pass `--vt 3` for
    # themselves.
    open --raw $"($deploy)/greetd-auroval.toml"
    | str replace -r '(?m)^vt = \d+$' $"vt = ($vt)"
    | save -f $"($stage)/greetd-auroval.toml"
    ^chmod 0755 $"($stage)/auroval-greet" $"($stage)/auroval-greet.sh"
    print $"         ($stage)"
    if $frame_dump != "" {
        print $"         AUROVAL_FRAME_DUMP=($frame_dump) is baked in — re-run"
        print "         without --frame-dump to take it out again."
    }

    # What the installed config says today, if there is one. Compared rather
    # than assumed, because the failure this catches is silent: on 2026-11-09
    # an install without `--vt 1` staged vt = 3 over a file that said 1, while
    # the greetd started before it went on serving tty1. The two disagreed for
    # eleven hours and the only place it would have shown is a reboot.
    let installed = "/etc/greetd/auroval.toml"
    if ($installed | path exists) {
        let now = (open --raw $installed | parse -r '(?m)^\s*vt\s*=\s*(\d+)' | get -o capture0.0)
        if ($now | is-not-empty) and $now != ($vt | into string) {
            print ""
            print $"         ($installed) says vt = ($now); this stages vt = ($vt)."
            print "         The root script below will overwrite it. If that is not what"
            print $"         you meant, re-run with --vt ($now)."
            print "         A running greetd keeps the VT it started with, so the two can"
            print "         disagree until the next restart or reboot."
            print ""
        }
    }

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

    # Digests, taken NOW, of exactly what was staged above.
    #
    # The window this closes was found by an adversarial review on
    # 2026-11-09, and it is structural rather than a slip: this script is
    # written as the maintainer and run as root, and between those two moments
    # root's inputs sit in `target/auroval-stage`, which is `drwxr-xr-x
    # dirk dirk`. Anything already running as the maintainer -- a `build.rs`
    # from a dependency, for one, which step 1/5 runs by the hundred -- could
    # append a line to the staged tmpfiles config, and `systemd-tmpfiles
    # --create` would then run it as uid 0. The root script stayed
    # byte-identical, so the reading the maintainer is told to do found
    # nothing.
    #
    # WHAT THIS IS AND IS NOT. It is not a fix. An attacker who can write the
    # stage can usually write this script too, and could edit both. What it
    # buys is that a silent edit of a data file is no longer enough: the
    # tampering has to reach the one artefact the maintainer is told to read,
    # and the digests are printed to the terminal as well so the two can be
    # compared without trusting either alone. The real fix is a root-owned
    # staging directory, which needs root before root runs, and that is the
    # open half.
    let staged_files = [
        "auroval.tmpfiles.conf"
        "auroval-greet"
        "auroval-greet.sh"
        "greetd-auroval.toml"
    ]
    let digests = ($staged_files | each {|f|
        {name: $f, sha: (open --raw $"($stage)/($f)" | hash sha256)}
    })
    print "         digests of what was staged:"
    for d in $digests {
        print $"           ($d.sha)  ($d.name)"
    }

    [
        "#!/bin/sh"
        "# Written by deploy/install.nu. Read it before you run it."
        "# It installs auroval's greeter files and NOTHING else: it does not"
        "# enable greetd, does not touch /etc/greetd/config.toml, and does not"
        "# stop, disable or reconfigure SDDM."
        "set -eux"
        ""
        "# Nothing below this block runs if anything in the staging directory"
        "# has changed since install.nu wrote this file. The directory belongs"
        "# to the maintainer and this script runs as root; see install.nu for"
        "# what that window is and what this does and does not close."
        "auroval_verify() {"
        '    printf "%s  %s\n" "$2" "$1" | sha256sum -c --status && return 0'
        '    echo "auroval: $1 is not what install.nu staged. REFUSING." >&2'
        '    echo "auroval: nothing has been installed. Re-run deploy/install.nu." >&2'
        "    exit 3"
        "}"
        ""
        "# Collected rather than printed where they happen: see the end."
        "auroval_notes=\"\""
        "auroval_note() {"
        '    echo "$1"'
        '    auroval_notes="$auroval_notes  - $1'
        '"'
        "}"
    ]
    | append ($digests | each {|d| $"auroval_verify \"($stage)/($d.name)\" ($d.sha)" })
    | append [
        ""
        "# C15 and R4: the two directories the greeter owns."
        $"install -m 0644 \"($stage)/auroval.tmpfiles.conf\" /etc/tmpfiles.d/auroval.conf"
        "systemd-tmpfiles --create /etc/tmpfiles.d/auroval.conf"
        ""
        "# The greeter and its startup path."
        "install -d -m 0755 /usr/local/lib/auroval"
        $"install -m 0755 \"($stage)/auroval-greet\"    /usr/local/lib/auroval/auroval-greet"
        $"install -m 0755 \"($stage)/auroval-greet.sh\" /usr/local/lib/auroval/auroval-greet.sh"
        ""
        "# greetd's config for auroval, alongside its own, never replacing it."
        "install -d -m 0755 /etc/greetd"
        $"install -m 0644 \"($stage)/greetd-auroval.toml\" /etc/greetd/auroval.toml"
        ""
        "# C15, and issue #77: warm the plugin registry here, as `greeter`, so"
        "# that the first login does not pay for it."
        "#"
        "# Measured 2026-11-09 under cage on a real VT at 2560x1440: a cold"
        "# greeter start reaches first frame in 1874 ms against R16's 500 ms"
        "# budget, and 19 ms with the registry warm. A cold gst-inspect scan"
        "# measures 0.9-1.0 s on this machine and writes 1.5 MB, so this line"
        "# costs the install about the second it takes off the first login."
        "#"
        "# It does NOT account for all of the 1874 ms, and that gap is open on"
        "# #77 rather than guessed at here. What is certain is that a process"
        "# starting with a warm registry reaches first frame in 19 ms, ten"
        "# times over, which is what the warm runs of cage-measure.nu are."
        "#"
        "# Last, and guarded. `set -eux` above would otherwise abort the whole"
        "# install over a missing gst-inspect-1.0 or a greeter user that cannot"
        "# run it -- and an unwarmed registry is a slow first login, not a"
        "# broken one. Deleting the registry is always safe: it is a cache in"
        "# the FHS sense and the next start rebuilds it."
        "if command -v gst-inspect-1.0 >/dev/null 2>&1; then"
        "    runuser -u greeter -- env XDG_CACHE_HOME=/var/cache/auroval \\"
        "        GST_REGISTRY=/var/cache/auroval/gst-registry.bin \\"
        "        gst-inspect-1.0 >/dev/null \\"
        '        || auroval_note "registry not warmed; the first login pays ~1.9 s once"'
        "else"
        '    auroval_note "gst-inspect-1.0 not found; registry not warmed"'
        "fi"
        ""
        "# R7/Q5, asked by the user who will actually be doing it. videoprobe"
        "# decodes a file into this renderer's own GL textures and prints what"
        "# was negotiated -- `memory:GLMemory` on the hardware path, system"
        "# memory on a software fallback that plays just as well at fourteen"
        "# times the CPU. Until now that question has only ever been asked as"
        "# the maintainer, who has a session and a seat. `greeter` is in the"
        "# `video` group and has neither, and whether the zero-copy path"
        "# survives that is not a thing to assume."
        "#"
        "# NOT a cache warming, despite how it looks next to the lines above."
        "# Measured 2026-11-09 (#77): this compiles two GL programs, 60 KB,"
        "# while the real greeter under cage compiles three, 352 KB. The"
        "# program that costs the second needs a real DRM surface, which"
        "# nothing available at install time has. See R16 and C15."
        "#"
        "# Theme media is not installed by this script (#63), so with nothing"
        "# to decode this says so and moves on rather than failing."
        "for v in /usr/local/share/auroval/*.mp4; do"
        '    [ -e "$v" ] || { echo "no theme media installed; decode path unverified"; break; }'
        # `env -u DISPLAY -u WAYLAND_DISPLAY`, and `EGL_PLATFORM=surfaceless`.
        #
        # The first time this step ever ran -- 2026-11-09, after the backslash
        # above was fixed -- it failed with `no off-screen GL context: EGL is
        # not initialized`, preceded by four `Authorization required` lines.
        # Those were the maintainer's own `DISPLAY=:0` leaking through `sudo`
        # into a user who may not use it. Isolated immediately afterwards, as
        # the maintainer, against the same clip:
        #
        #     with a display                      GLMemory
        #     no display                          no off-screen GL context
        #     no display, EGL_PLATFORM=surfaceless  GLMemory
        #
        # So it was never about being `greeter`. With no display server EGL
        # has no platform to guess and refuses; naming one answers it. The
        # greeter proper is unaffected -- it runs under cage and has a
        # Wayland display -- so this belongs here and not in the binary.
        #
        # What this probe now establishes is that the decode reaches a GL
        # texture in a context `greeter` can create without a seat. It is
        # still NOT the surface cage gives it, so it does not stand in for
        # R7's figure; it answers the cheaper question, which had no answer
        # at all until tonight.
        "    runuser -u greeter -- env -u DISPLAY -u WAYLAND_DISPLAY \\"
        "        EGL_PLATFORM=surfaceless XDG_CACHE_HOME=/var/cache/auroval \\"
        "        GST_REGISTRY=/var/cache/auroval/gst-registry.bin \\"
        # ONE backslash, and nushell is why this is worth a comment. A
        # single-quoted string here is literal, so `\\` emitted two
        # backslashes into the root script: `sh` then read an escaped
        # backslash rather than a line continuation, the next line began with
        # a bare `|`, and the whole thing died with `syntax error near
        # unexpected token '|'`. `sh` executes as it parses, so every
        # `install` above had already run -- the script mutated /etc and
        # /usr/local and then exited 2, which nobody noticed. Found by an
        # adversarial review on 2026-11-09, after it had been shipping for a
        # day; the videoprobe below, whose whole point is that the seatless
        # decode path "is not a thing to assume", had never once run.
        '        /usr/local/lib/auroval/auroval-greet videoprobe "$v" 1 \'
        "        | grep -E '^memory|^result' \\"
        '        || auroval_note "videoprobe failed as greeter; the decode path is unverified"'
        "    break"
        "done"
        ""
        "# Said once more, at the end, with the tracing off.",
        "#"
        "# Everything above that can fail without breaking an install says so"
        "# and carries on, which is right -- an unwarmed registry is a slow"
        "# first login and an unverified decode path is a question, neither is"
        "# a broken system. But `set -x` means those messages scroll past in"
        "# two hundred lines of tracing, and the one time this step actually"
        "# ran and failed (2026-11-09) it was noticed only because the"
        "# maintainer pasted the whole output to be read. A note nobody sees"
        "# is not a note."
        "set +x"
        'if [ -n "$auroval_notes" ]; then'
        '    echo ""'
        '    echo "auroval: installed, with notes:"'
        '    printf "%s\n" "$auroval_notes"'
        '    echo ""'
        '    echo "auroval: the install itself succeeded; the lines above are"'
        '    echo "auroval: things that could not be checked, not failures of it."'
        "else"
        '    echo ""'
        '    echo "auroval: installed, nothing to report."'
        "fi"
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
    # Asked rather than asserted. This paragraph said "greetd is still not
    # enabled. SDDM is still the way in." unconditionally, which was true of
    # every machine until one of them switched -- and then it was a line
    # telling the maintainer the opposite of what his own machine was doing.
    let enabled = ((^systemctl is-enabled greetd.service | complete).stdout | str trim)
    if $enabled == "enabled" {
        print "greetd is ENABLED on this machine: this greeter is the way in, and the"
        print "files above have just been staged over the ones it will use next time it"
        print "starts. A running greetd keeps what it read at startup, so nothing"
        print "changes until it restarts or the machine reboots."
        print "The way back out is deploy/TRIAL.md section 4."
    } else {
        print $"greetd is ($enabled). SDDM is still the way in. To see the"
        print "greeter under greetd once it can draw:"
        print "  sudo nu spike/p1e/greetd-session.nu --vt 3 --seconds 60"
        print "  \(that one does need root — greetd opens a VT and talks to PAM —"
        print "   but it starts greetd by hand with auroval's own config and stops"
        print "   it again. It never enables the service and never touches SDDM.)"
    }
}
