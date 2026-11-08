#!/usr/bin/env nu
#
# P1/A and P1/B — the two verification items on issue #36 that nothing in this
# repository can reach without a real console and a real PAM stack.
#
#   R3  A wrong password shows the message, returns to the prompt, and THE
#       PROCESS DOES NOT EXIT. A login screen that dies on a typo is the worst
#       possible failure, so "did not exit" is the half that matters, and it is
#       the half this script can actually measure rather than ask about.
#
#   R1  A PAM stack with an extra pam_exec step completes login WITH NO CODE
#       CHANGE. That is the whole point of UiState being a dumb renderer:
#       greetd's conversation can say anything and the greeter just draws it.
#       See spike/p1ab/PAM.md — including §6, which is honest about pam_exec
#       producing an `info` message and not a prompt.
#
# crates/greet/src/auth.rs is the only module in this project with neither a
# test nor a real run behind it. This sitting is the run.
#
# WHAT THIS SCRIPT IS: AN OBSERVER, NOT A DRIVER.
#
# It cannot start greetd. greetd opens a VT and talks to PAM, so it needs root,
# and this repository never runs root commands on your behalf — a failed sudo
# attempt trips pam_faillock and can lock the maintainer out of his own
# machine. So the shape is: you run this as yourself on one VT, it prints the
# root command for you to run on another, and while you are using the greeter
# on a third it watches from the outside and writes down what it sees.
#
# What it can observe by itself, as you:
#   * every process greetd starts, sampled once a second, with pids
#   * whether the FIRST greeter pid was still the same pid at the end — R3
#   * /var/log/auroval-pam-extra.log, which the PAM helper writes as root with
#     the default umask and is therefore readable by you: one timestamped line
#     per authentication attempt. Cross-referenced against the pid timeline,
#     that turns "the greeter survived the wrong password" from an eye-witness
#     claim into two timestamps and an unchanged pid.
#
# What only you can see, and what it therefore ASKS you rather than claims:
#   * whether the error message appeared on screen, and what it said
#   * whether the prompt came back
#   * whether the extra PAM line was drawn, and for how long
#   * whether the second attempt reached a session
# Those are recorded in the receipt marked `operator:`, never as measurements.
#
# It touches no system configuration, starts no service, and writes one file.
#
# ── the sitting needs four consoles. Set them up before you start. ──────────
#
#   VT 2   Ctrl+Alt+F2 — a root shell (`sudo -i`), opened and then left alone.
#          PAM.md §5 says why. Do this FIRST.
#   VT 4   Ctrl+Alt+F4 — you, logged in as yourself. THIS script runs here.
#   VT 5   Ctrl+Alt+F5 — you, logged in as yourself. The `sudo nu
#          spike/p1e/greetd-session.nu` command runs here, because VT 4 is busy
#          watching.
#   VT 3   Ctrl+Alt+F3 — nobody. greetd takes it; this is where the greeter
#          draws and where you type.
#
# Escape route, at every point: your Hyprland session is on its own VT and is
# untouched. Ctrl+Alt+F1 returns to it. greetd stops itself when
# greetd-session.nu's deadline passes, and SDDM is still `enabled` and still
# the way into this machine.
#
#   nu spike/p1ab/greetd-round.nu              # the sitting
#   nu spike/p1ab/greetd-round.nu --no-pause   # no prompts; for checking it runs
#   nu spike/p1ab/greetd-round.nu --dry-run    # preflight and the script, then stop

const OUT = "~/auroval-p1ab-greetd.txt"
const PAM_LOG = "/var/log/auroval-pam-extra.log"
const PAM_HELPER = "/usr/local/lib/auroval/pam-extra-prompt.sh"
const PAM_FILE = "/etc/pam.d/greetd"
const GREETD_CONFIG = "/etc/greetd/auroval.toml"
const TOTAL = 8

def main [
    --vt: int = 3            # the VT greetd will take. NOT the one you are on
    --seconds: int = 180     # how long to watch. Two login attempts need ~90 s
    --no-pause               # do not wait for Enter anywhere; for a syntax run
    --dry-run                # preflight and print the script, then stop
] {
    let out = ($OUT | path expand)
    mut step = 0
    mut lines = []
    mut problems = []

    # ── 1/8 context: the refusal that is the point of the script ────────────
    $step = $step + 1
    print $"  ($step)/($TOTAL)  context"
    if ($env | get -o WAYLAND_DISPLAY | is-not-empty) {
        print ""
        print "  WAYLAND_DISPLAY is set, so you are inside your Hyprland session."
        print "  Nothing about this sitting can happen from here: greetd needs a"
        print "  free VT, the greeter needs DRM master, and a wrong-password"
        print "  attempt needs a real PAM conversation on a real console. There"
        print "  is nothing to measure from inside a compositor."
        print ""
        print "  Log in on a free console and run it there:"
        print "    Ctrl+Alt+F4, log in as yourself, then"
        print $"    cd (pwd-of-repo) ; nu spike/p1ab/greetd-round.nu"
        print ""
        print "  Ctrl+Alt+F1 brings you back here afterwards."
        exit 2
    }
    if ($env | get -o DISPLAY | is-not-empty) {
        print "         DISPLAY is set — an X session is in the way. Use a free VT."
        exit 2
    }
    let my_tty = (do { ^tty } | complete | get stdout | str trim)
    if $my_tty == $"/dev/tty($vt)" {
        print $"         you are ON /dev/tty($vt), which is the VT greetd is about to take."
        print $"         Move to another console, or pass --vt with a free one."
        exit 2
    }
    let started = (date now | format date "%Y-%m-%dT%H:%M:%S%:z")
    # Whether there is anybody to ask. `tty` prints "not a tty" when stdin is
    # not one, and `input` cannot be made to fail gracefully in that case on
    # every nu build, so the question is settled once, here, rather than
    # discovered by an aborted run with the receipt half written.
    let can_ask = ((not $no_pause) and ($my_tty | str starts-with "/dev/"))
    print $"         ($my_tty), not a compositor. greetd will take tty($vt)."
    if not $can_ask {
        print "         no terminal on stdin (or --no-pause): this run will not pause"
        print "         and will not ask the eye-witness questions in step 8."
    }

    $lines = [
        "auroval P1/A + P1/B — greetd round: R3 (wrong password is not fatal)"
        "                      and R1 (an extra PAM step needs no code change)"
        $started
        ""
        $"host        (sys host | get hostname)  kernel (sys host | get kernel_version)"
        $"this tty    ($my_tty)"
        $"greetd vt   ($vt)"
        $"watching    ($seconds) s"
        $"pam file    ($PAM_FILE)"
        $"pam helper  ($PAM_HELPER)"
        $"pam log     ($PAM_LOG)"
        ""
        "Lines marked `measured:` were observed by this script. Lines marked"
        "`operator:` were typed in by the human who watched the screen and are"
        "NOT measurements. Nothing in this file claims to have seen a pixel."
        ""
    ]
    # Written before anything else, not after the last step: a sitting you can
    # abort halfway and still have evidence from is one you will repeat.
    # spike/p1e learned this by losing a whole run to output flushed on exit.
    $lines | str join "\n" | save -f $out

    # ── 2/8 what it needs ───────────────────────────────────────────────────
    # Everything checked here, so a missing piece is named now rather than
    # found inside a step the operator has already switched VTs for.
    $step = $step + 1
    print $"  ($step)/($TOTAL)  what this needs"
    $lines = ($lines | append "── preflight ──")

    for probe in [
        { what: "greetd binary", kind: "which", target: "greetd", fix: "pacman -S greetd" }
        { what: "cage", kind: "which", target: "cage", fix: "pacman -S cage" }
        { what: "greetd config", kind: "file", target: $GREETD_CONFIG, fix: "nu deploy/install.nu, then its root script" }
        { what: "pam service file", kind: "file", target: $PAM_FILE, fix: "pacman -S greetd (it ships this file)" }
        { what: "pam_exec module", kind: "file", target: "/usr/lib/security/pam_exec.so", fix: "it is part of the `pam` package" }
        { what: "R4 state dir", kind: "dir", target: "/var/lib/auroval", fix: "nu deploy/install.nu, then its root script" }
        { what: "C15 cache dir", kind: "dir", target: "/var/cache/auroval", fix: "nu deploy/install.nu, then its root script" }
        { what: "pam helper", kind: "file", target: $PAM_HELPER, fix: "spike/p1ab/PAM.md §3" }
    ] {
        let ok = if $probe.kind == "which" {
            (which $probe.target | is-not-empty)
        } else {
            ($probe.target | path exists)
        }
        if $ok {
            print $"         ok      ($probe.what)  ($probe.target)"
            $lines = ($lines | append $"measured: ok      ($probe.what) — ($probe.target)")
        } else {
            print $"         MISSING ($probe.what)  ($probe.target)"
            print $"                 fix: ($probe.fix)"
            $lines = ($lines | append $"measured: MISSING ($probe.what) — ($probe.target); fix: ($probe.fix)")
            $problems = ($problems | append $"($probe.what) is missing — ($probe.fix)")
        }
    }

    # The greeter user, and `video`: cage needs DRM master and logind only
    # hands it to a member of that group. deploy/install.nu checks the same
    # thing, and it is checked again here because the failure looks like a
    # renderer bug rather than a group.
    let has_greeter = ((do { ^id -u greeter } | complete).exit_code == 0)
    if $has_greeter {
        let groups = (^id -nG greeter | str trim | split row " ")
        print $"         ok      greeter user, groups: ($groups | str join ', ')"
        $lines = ($lines | append $"measured: greeter groups ($groups | str join ', ')")
        if not ("video" in $groups) {
            print "         WARNING greeter is not in `video` — cage will not get DRM master"
            $problems = ($problems | append "greeter is not in the `video` group — cage cannot take DRM master")
        }
    } else {
        print "         MISSING there is no `greeter` user — is greetd installed?"
        $problems = ($problems | append "no `greeter` user")
    }

    # The extra PAM line. Reported as present or absent rather than demanded:
    # the R3 half of this sitting is worth doing without it, and PAM.md is
    # deliberately a thing the operator may decline.
    let pam_line = (
        if ($PAM_FILE | path exists) {
            open --raw $PAM_FILE | lines | where { |l| ($l | str contains "pam_exec") and not ($l | str starts-with "#") }
        } else { [] }
    )
    if ($pam_line | is-empty) {
        print $"         absent  no pam_exec line in ($PAM_FILE)"
        print "                 R1's message half will NOT be exercised. R3 still will."
        print "                 To add it: read spike/p1ab/PAM.md and run §3 as root."
        $lines = ($lines | append $"measured: NO pam_exec line in ($PAM_FILE) — R1's message half not exercised")
    } else {
        print $"         ok      pam_exec line present:"
        for l in $pam_line { print $"                   ($l | str trim)" }
        $lines = ($lines | append $"measured: pam_exec line present — ($pam_line | str join ' / ')")
    }

    # pam_oath is the half that makes R1 what its fit criterion says: an extra
    # *prompt*, not an extra message. pam_exec cannot ask a question (PAM.md
    # §6), so without this line the sitting proves the message branch and
    # nothing about message count -- which is the thing a fixed two-field
    # greeter actually fails on.
    let oath_line = (
        if ($PAM_FILE | path exists) {
            open --raw $PAM_FILE | lines | where { |l| ($l | str contains "pam_oath") and not ($l | str starts-with "#") }
        } else { [] }
    )
    if ($oath_line | is-empty) {
        print $"         absent  no pam_oath line in ($PAM_FILE)"
        print "                 R1 will be exercised for a MESSAGE but not for an"
        print "                 extra PROMPT. To add it: PAM.md §3b, as root."
        $lines = ($lines | append "measured: NO pam_oath line — R1's extra-prompt half not exercised")
    } else {
        print $"         ok      pam_oath line present:"
        for l in $oath_line { print $"                   ($l | str trim)" }
        $lines = ($lines | append $"measured: pam_oath line present — ($oath_line | str join ' / ')")
        let users_oath = "/etc/users.oath"
        if ($users_oath | path exists) {
            print $"         ok      ($users_oath) exists"
            $lines = ($lines | append $"measured: ($users_oath) exists")
        } else {
            print $"         MISSING ($users_oath) — pam_oath is `required`, so greetd"
            print "                 will refuse every login until it exists. PAM.md §3b."
            $lines = ($lines | append $"measured: ($users_oath) MISSING while pam_oath is required")
        }
        if ((which oathtool | length) > 0) {
            print "         ok      oathtool, for computing the code on another VT"
        } else {
            print "         MISSING oathtool — you will have no way to read the code"
        }
    }
    $lines | str join "\n" | save -f $out

    # ── 3/8 nothing here may change how you log in ──────────────────────────
    # The standing constraint of every spike in this repository, checked rather
    # than asserted. greetd must stay disabled and SDDM must stay enabled; if
    # that is not already true, this script is not the thing that made it so
    # and it should not run on top of it.
    $step = $step + 1
    print $"  ($step)/($TOTAL)  SDDM is still the way in"
    let greetd_state = (do { ^systemctl is-enabled greetd } | complete | get stdout | str trim)
    let sddm_state = (do { ^systemctl is-enabled sddm } | complete | get stdout | str trim)
    print $"         greetd.service  ($greetd_state)"
    print $"         sddm.service    ($sddm_state)"
    $lines = ($lines | append [
        $"measured: greetd.service is-enabled = ($greetd_state)"
        $"measured: sddm.service   is-enabled = ($sddm_state)"
    ])
    if $greetd_state != "disabled" {
        print "         greetd is NOT disabled. This sitting starts greetd by hand with"
        print "         a deadline; a greetd that is also a service will fight it and"
        print "         will keep yanking the console back. Refusing."
        $lines = ($lines | append "measured: REFUSED — greetd.service is not disabled")
        $lines | str join "\n" | save -f $out
        exit 2
    }
    if $sddm_state != "enabled" {
        print "         sddm is not enabled. Nothing here disabled it, and nothing here"
        print "         will re-enable it, but the escape route this sitting assumes"
        print "         \(reboot into a working login screen) is not there. Refusing."
        $lines = ($lines | append "measured: REFUSED — sddm.service is not enabled; no escape route")
        $lines | str join "\n" | save -f $out
        exit 2
    }
    # The one thing worth refusing outright, same as spike/p1e/greetd-session.nu.
    if ($GREETD_CONFIG | path expand) == "/etc/greetd/config.toml" {
        print "         that is greetd's own config. This project never writes it."
        exit 2
    }
    print "         unchanged — greetd disabled, SDDM enabled"

    # ── 4/8 the operator's script, BEFORE anything takes the screen ─────────
    $step = $step + 1
    print $"  ($step)/($TOTAL)  your script for the sitting"
    let user = ($env.USER? | default (^id -un | str trim))
    let has_pam = ($pam_line | is-not-empty)
    print ""
    print "════════════════════════════════════════════════════════════════════"
    print "  READ THIS NOW. You will be on another VT with no scrollback."
    print ""
    print $"  1.  Leave this VT \(($my_tty)) running. It is the observer."
    print ""
    print "  2.  Ctrl+Alt+F5, log in as yourself, and run:"
    print $"        cd (pwd-of-repo)"
    print $"        sudo nu spike/p1e/greetd-session.nu --vt ($vt) --seconds ($seconds)"
    print "      That is the only root command in this sitting and it is yours"
    print "      to run. It starts greetd with auroval's own config, detached,"
    print "      with a deadline, and stops it again. It never enables the"
    print "      service and never touches SDDM."
    print ""
    print $"  3.  Ctrl+Alt+F($vt). The greeter is there."
    print ""
    print $"  4.  Type the username:  ($user)"
    if $has_pam {
        print ""
        print "  5.  WATCH FOR THE EXTRA PAM LINE, before the password prompt:"
        print "        \"auroval P1/A: an extra PAM step is in this stack (R1)\""
        print "      It should sit on screen for about four seconds. This is R1:"
        print "      a message the greeter was never coded for, drawn and"
        print "      answered with no response. There is nothing to type."
        print "      Note whether the four seconds are spent WITH the line"
        print "      showing or blank-then-flash — that tells us whether"
        print "      pam_exec streams or buffers, and the script cannot see it."
    } else {
        print ""
        print "  5.  \(no pam_exec line installed, so nothing extra to look for)"
    }
    if not ($oath_line | is-empty) {
        print ""
        print "  5b. THE ONE-TIME PASSWORD. The greeter will ask a SECOND"
        print "      question, with echo off, something like"
        print "        \"One-time password (OATH) :\""
        print "      Have `oathtool --totp <your secret>` ready on another VT;"
        print "      the code changes every thirty seconds. THIS is R1's real"
        print "      criterion: a greeter with two fixed fields has nowhere to"
        print "      put a second question. Note whether the first prompt's"
        print "      text was replaced rather than appended to."
    }
    print ""
    print "  6.  TYPE THE WRONG PASSWORD. Deliberately. Something you will not"
    print "      mistake for a typo — `wrong-on-purpose` is fine."
    print ""
    print "  7.  This is R3, and it is the whole sitting. Look for, in order:"
    print "        a.  an error message on screen, and read what it says"
    print "        b.  the prompt COMING BACK — a usable field, not a dead screen"
    print "        c.  the greeter still being there at all. If the screen goes"
    print "            black or greetd restarts the greeter from scratch, R3 has"
    print "            FAILED and that is the finding."
    print ""
    print $"  8.  Now type your REAL password and log in. You should land in a"
    print "      session. Then log out of it."
    print ""
    print $"  9.  Ctrl+Alt+F4 \(back here). This script will be waiting with"
    print "      questions only you can answer."
    print ""
    print $"  If anything goes wrong: Ctrl+Alt+F1 is your Hyprland session,"
    print $"  untouched. greetd stops itself after ($seconds) s. Your root shell on"
    print "  VT 2 is still a root shell. SDDM still boots."
    print "════════════════════════════════════════════════════════════════════"
    print ""
    $lines = ($lines | append [
        ""
        "── the script given to the operator ──"
        $"username to type: ($user)"
        $"wrong password first, then the real one; R1 line expected: ($has_pam)"
        $"root command printed, not run: sudo nu spike/p1e/greetd-session.nu --vt ($vt) --seconds ($seconds)"
    ])
    $lines | str join "\n" | save -f $out

    if ($problems | is-not-empty) {
        print "  Before you start, these were missing or wrong:"
        for p in $problems { print $"    * ($p)" }
        print ""
    }

    if $dry_run {
        print "  --dry-run: stopping here. Nothing was started."
        $lines = ($lines | append "measured: --dry-run, the sitting was not held")
        $lines | str join "\n" | save -f $out
        print $"written to ($out)"
        return
    }

    if $can_ask {
        ask "  Press Enter when you have read all nine steps. " | ignore
    }

    # ── 5/8 the baseline, then wait for greetd ──────────────────────────────
    # Taken before greetd exists so that everything the watch step sees is
    # attributable to this sitting and not to something already running.
    $step = $step + 1
    print $"  ($step)/($TOTAL)  baseline, then waiting for greetd"
    let pam_before = (pam-log-lines)
    print $"         ($PAM_LOG) has ($pam_before | length) lines already"
    $lines = ($lines | append $"measured: pam log baseline ($pam_before | length) lines")
    let stray = (greeter-pids)
    if ($stray | is-not-empty) {
        print $"         WARNING a greeter process is already running: ($stray | str join ', ')"
        $lines = ($lines | append $"measured: WARNING pre-existing greeter pids ($stray | str join ', ')")
    }
    $lines | str join "\n" | save -f $out

    print "         go and run the sudo command on VT 5 now."
    mut waited = 0
    let give_up = 300
    while ((greetd-pid) | is-empty) and $waited < $give_up {
        sleep 1sec
        $waited = $waited + 1
        # Counted, not just named. A silent wait of up to five minutes reads as
        # a hang, and someone nearly aborted a good run in spike/p1e on
        # exactly that.
        if ($waited mod 10) == 0 {
            print $"         ($waited)/($give_up) s  no greetd yet"
        }
    }
    if ((greetd-pid) | is-empty) {
        print $"         no greetd appeared in ($give_up) s. Nothing was watched."
        print "         Did the sudo command on VT 5 fail? Its log:"
        print "           sudo cat /var/log/auroval-greetd.log"
        $lines = ($lines | append $"measured: greetd never appeared within ($give_up) s — nothing observed")
        $lines | str join "\n" | save -f $out
        exit 1
    }
    print $"         greetd is up, pid (greetd-pid)"
    $lines = ($lines | append $"measured: greetd appeared after ($waited) s, pid (greetd-pid)")
    $lines | str join "\n" | save -f $out

    # ── 6/8 the watch — R3's real criterion ─────────────────────────────────
    # One sample a second: greetd's pid, and every pid running the greeter.
    # The timeline is the evidence. R3 asks whether the greeter process
    # survives a wrong password, and a pid that is the same pid before and
    # after the attempt is the answer; a pid that changes is greetd having
    # restarted a greeter that died, which is the failure R3 exists to forbid.
    #
    # One pid change IS expected, at the very end: when a session starts,
    # greetd tears the greeter down. That is greetd doing its job, not R3
    # failing, which is why this records a timeline instead of a boolean.
    $step = $step + 1
    print $"  ($step)/($TOTAL)  watching for ($seconds) s"
    mut timeline = []
    mut t = 0
    mut first_greeter = ""
    mut last_seen = []
    while $t < $seconds {
        let g = (greetd-pid)
        let ps = (greeter-pids)
        if ($first_greeter | is-empty) and ($ps | is-not-empty) {
            $first_greeter = ($ps | first)
            print $"         t=($t)s  greeter appeared, pid ($first_greeter)"
            $lines = ($lines | append $"measured: t=($t)s greeter appeared, pid ($first_greeter)")
            $lines | str join "\n" | save -f $out
        }
        if $ps != $last_seen {
            let shown = (if ($ps | is-empty) { "none" } else { $ps | str join ',' })
            $timeline = ($timeline | append { t: $t, greetd: $g, greeter: $shown })
            print $"         t=($t)s  greeter pids: ($shown)"
            $lines = ($lines | append $"measured: t=($t)s greetd=(if ($g | is-empty) { 'gone' } else { $g }) greeter=($shown)")
            $lines | str join "\n" | save -f $out
            $last_seen = $ps
        }
        if ($g | is-empty) {
            print $"         t=($t)s  greetd is gone — the deadline passed or you stopped it."
            $lines = ($lines | append $"measured: t=($t)s greetd gone; watch ends early")
            break
        }
        sleep 1sec
        $t = $t + 1
        if ($t mod 15) == 0 {
            print $"         ($t)/($seconds) s"
        }
    }
    let changes = ($timeline | length)
    print $"         watch over. ($changes) change\(s) in the greeter process set."
    $lines = ($lines | append $"measured: watch ended at t=($t)s with ($changes) greeter-set change\(s)")
    $lines | str join "\n" | save -f $out

    # ── 7/8 the locale probe, because it is nearly free ─────────────────────
    # crates/auroval-ui/src/clock.rs now resolves the date's language from LC_ALL,
    # then LC_TIME, then LANG, the way the C library does. /etc/locale.conf on
    # this machine sets LC_TIME=nl_NL.UTF-8. If the greeter cannot see it, the
    # login screen says "Sunday, 8 November" while everything else on the
    # machine says "zondag, 8 november" — and we want to know that from THIS
    # sitting rather than the next one.
    #
    # The greeter runs as `greeter`, and /proc/<pid>/environ is readable only
    # by the process owner and root, so this cannot be read as you. What CAN be
    # read is each layer the value would have to come through, which is enough
    # to say where it is lost.
    $step = $step + 1
    print $"  ($step)/($TOTAL)  the greeter's view of LC_ALL, LC_TIME, LANG"
    $lines = ($lines | append ["" "── locale (clock.rs, LC_ALL > LC_TIME > LANG) ──"])

    # Layer 1: the file that is supposed to be the source of truth.
    if ("/etc/locale.conf" | path exists) {
        let conf = (open --raw /etc/locale.conf | lines | where { |l| $l =~ '^(LANG|LC_ALL|LC_TIME)=' })
        for l in $conf { print $"         locale.conf   ($l)" }
        $lines = ($lines | append ($conf | each { |l| $"measured: /etc/locale.conf ($l)" }))
    } else {
        print "         locale.conf   absent"
        $lines = ($lines | append "measured: /etc/locale.conf absent")
    }

    # Layer 2: systemd's manager environment — what greetd.service WOULD
    # inherit once greetd is the service. Readable without root.
    let sysenv = (
        do { ^systemctl show-environment } | complete | get stdout | lines
        | where { |l| $l =~ '^(LANG|LC_ALL|LC_TIME)=' }
    )
    if ($sysenv | is-empty) {
        print "         systemd       sets none of LC_ALL, LC_TIME, LANG"
        $lines = ($lines | append "measured: systemd manager env sets none of LC_ALL/LC_TIME/LANG")
    } else {
        for l in $sysenv { print $"         systemd       ($l)" }
        $lines = ($lines | append ($sysenv | each { |l| $"measured: systemd manager env ($l)" }))
    }

    # Layer 3: your own session, for contrast. This is the environment the
    # Dutch date is already correct in, and the whole risk is the two layers
    # disagreeing.
    let mine = (
        ["LC_ALL" "LC_TIME" "LANG"]
        | each { |k| $"($k)=($env | get -o $k | default '—')" }
    )
    print $"         you           ($mine | str join '  ')"
    $lines = ($lines | append $"measured: this login shell ($mine | str join '  ')")

    # Layer 4: the greeter itself. Attempted, and honestly reported as
    # unreadable rather than quietly skipped.
    let gp = (if ($first_greeter | is-empty) { (greeter-pids | get -o 0 | default "") } else { $first_greeter })
    if ($gp | is-empty) {
        print "         greeter       no greeter process was seen; nothing to read"
        $lines = ($lines | append "measured: no greeter pid seen — the greeter's own env was not read")
    } else {
        let envfile = $"/proc/($gp)/environ"
        let r = (do { ^cat $envfile } | complete)
        # Exit code only, deliberately: an environ that READS but is EMPTY is
        # a real and important outcome — a process started with a cleared
        # environment — and it must not be reported as a permission problem.
        if $r.exit_code == 0 {
            let got = (
                $r.stdout | split row (char nul)
                | where { |l| $l =~ '^(LANG|LC_ALL|LC_TIME)=' }
            )
            let shown = (if ($got | is-empty) { "sets NONE of LC_ALL, LC_TIME, LANG" } else { $got | str join '  ' })
            print $"         greeter       ($shown)"
            $lines = ($lines | append $"measured: greeter pid ($gp) environ — ($shown)")
            if ($got | is-empty) {
                print "         ^ the date on the login screen will be ENGLISH while the"
                print "           rest of this machine is Dutch. That is the finding."
                $lines = ($lines | append "measured: FINDING — greeter sees no locale; clock.rs falls back to POSIX (English)")
            }
        } else {
            # The expected branch. /proc/<pid>/environ is 0400 and owned by
            # `greeter`, so this is a permission boundary, not a failure.
            print $"         greeter       /proc/($gp)/environ is not readable as you \(it is"
            print "                       owned by `greeter`, mode 0400). This is the one"
            print "                       number this script cannot get. Run this from your"
            print "                       root shell on VT 2 WHILE greetd is still up:"
            print ""
            print $"                         tr '\\0' '\\n' < /proc/($gp)/environ | grep -E '^LANG|^LC_'"
            print ""
            print "                       An empty result means the greeter sees no locale at"
            print "                       all, and clock.rs falls back to POSIX, i.e. English."
            $lines = ($lines | append [
                $"measured: greeter pid ($gp) environ NOT readable as you \(owned by `greeter`, 0400)"
                $"operator-todo: as root, while greetd runs: tr '\\0' '\\n' < /proc/($gp)/environ | grep -E '^LANG|^LC_'"
            ])
        }
    }
    # The misleading shortcut, named so nobody reaches for it: `sudo -u greeter
    # env` shows a LOGIN environment built by pam_env and the shell, which is
    # not what greetd hands the greeter. greetd clears the environment and
    # passes only the PAM environment list plus a few XDG variables of its own.
    $lines = ($lines | append "note: `sudo -u greeter env` is NOT this answer — greetd clears the environment and passes only the PAM env list plus its own XDG vars.")
    $lines | str join "\n" | save -f $out

    # ── 8/8 the verdict: what was measured, and what only you saw ───────────
    $step = $step + 1
    print $"  ($step)/($TOTAL)  verdict"
    $lines = ($lines | append ["" "── verdict ──"])

    # The PAM log. This is the strongest thing the script has: one line per
    # authentication attempt, written by root, readable by you, timestamped.
    let pam_after = (pam-log-lines)
    let new_pam = ($pam_after | skip ($pam_before | length))
    print ""
    if not $has_pam {
        print "  R1  NOT EXERCISED — no pam_exec line was in the stack."
        $lines = ($lines | append "R1 NOT EXERCISED — no pam_exec line in the PAM stack this sitting")
    } else if ($new_pam | is-empty) {
        print "  R1  the pam_exec line is in the stack but the helper left NO new"
        print $"      lines in ($PAM_LOG). PAM did not run it. Check that the path in"
        print "      the line matches where the helper is installed, and that it is"
        print "      executable and root-owned. PAM.md §7."
        $lines = ($lines | append "R1 FAILED TO RUN — pam_exec line present but the helper logged nothing; PAM never executed it")
    } else {
        print $"  R1  measured: the helper ran ($new_pam | length) time\(s):"
        for l in $new_pam { print $"        ($l)" }
        print "      So PAM really did push an extra message into the conversation."
        print "      Whether the GREETER DREW IT is the half only you saw — below."
        $lines = ($lines | append [
            $"R1 measured: the pam_exec helper ran ($new_pam | length) time\(s) during the sitting:"
        ] | append ($new_pam | each { |l| $"  ($l)" }))
        let users = ($new_pam | each { |l| $l | parse -r 'user=(\S+)' | get -o capture0.0 } | compact | uniq)
        if ($users | length) > 1 {
            print $"      \(users seen: ($users | str join ', ') — PAM.md §5 explains the"
            print "       `greeter` one; it is harmless.)"
            $lines = ($lines | append $"R1 note: users seen in the pam log: ($users | str join ', ')")
        }
    }

    print ""
    # R3, the part that is a measurement. The pid of the first greeter, and
    # whether that same pid was still running after the attempts the pam log
    # proves happened.
    let attempts = ($new_pam | length)
    if ($first_greeter | is-empty) {
        print "  R3  NOT MEASURED — no greeter process was ever seen. greetd was up"
        print "      but nothing ran the greeter, so there was nothing to survive a"
        print "      wrong password. Check /var/log/auroval-greetd.log."
        $lines = ($lines | append "R3 NOT MEASURED — greetd ran but no greeter process was ever observed")
    } else {
        let survived = ($timeline | where { |r| $r.greeter == "none" } | is-empty)
        let same_pid = ($timeline | all { |r| ($r.greeter == "none") or ($r.greeter | str contains $first_greeter) })
        print $"  R3  measured: first greeter pid ($first_greeter)"
        print $"      measured: ($changes) change\(s) in the greeter process set during ($t) s"
        for r in $timeline { print $"        t=($r.t)s  ($r.greeter)" }
        $lines = ($lines | append [
            $"R3 measured: first greeter pid ($first_greeter); ($changes) change\(s) over ($t) s"
        ] | append ($timeline | each { |r| $"  t=($r.t)s greeter=($r.greeter)" }))
        if $survived and $same_pid and $attempts > 0 {
            # The only branch that may say HOLDS, and it needs both halves:
            # the pid never changed AND the pam log proves authentication
            # attempts happened inside that window. A greeter that quietly
            # survived a window in which nobody typed anything has survived
            # nothing, and R3 is about surviving something.
            print "      The greeter process NEVER went away and never changed pid."
            print $"      measured: ($attempts) authentication attempt\(s) are logged above,"
            print "      so attempts really did happen inside that window."
            print "      That is R3's real criterion, and it HOLDS: a wrong password did"
            print "      not exit the process."
            $lines = ($lines | append $"R3 HOLDS \(measured): greeter pid ($first_greeter) unchanged and alive throughout; ($attempts) PAM attempt\(s) logged inside the window")
        } else if $survived and $same_pid {
            # Survived, but the script cannot prove anything was typed. This
            # is the state of a sitting held without the pam_exec line
            # installed: the pid evidence is real and the "survived WHAT?"
            # half rests entirely on the operator's answers below.
            print "      The greeter process never went away and never changed pid —"
            print "      but NO authentication attempt is logged for this window, so"
            print "      this script cannot show that anything was typed at it. Without"
            print "      the PAM helper installed there is no log to read; with it"
            print "      installed, an empty log means PAM never ran it."
            print "      R3 therefore rests on your answers below, not on a measurement."
            $lines = ($lines | append $"R3 PARTIAL: greeter pid ($first_greeter) unchanged and alive for ($t) s, but no authentication attempt is logged in that window — the script cannot show anything was typed. R3 rests on the operator's answers below.")
        } else {
            print "      The greeter process set CHANGED. Read the timeline above against"
            print "      the pam log timestamps:"
            print "        * a change AFTER your successful login is greetd doing its job"
            print "          — it tears the greeter down when a session starts."
            print "        * a change at or just after the WRONG password is R3 FAILING,"
            print "          and it is the finding this sitting exists to catch."
            print "      The script cannot tell these apart on its own; the timestamps can."
            $lines = ($lines | append "R3 INCONCLUSIVE BY SCRIPT: the greeter process set changed — align the timeline above with the pam log timestamps. A change at the wrong-password attempt is a failure; a change after the successful login is expected.")
        }
    }

    # ── the eye-witness half, asked and recorded as such ───────────────────
    print ""
    print "  ── now the half no script can see ──"
    if not $can_ask {
        print "  (not asked this run: there is no terminal to ask from. Every answer"
        print "   below is recorded as unasked, which is the honest value for it.)"
    }
    print "  Answer from what you actually watched. `?` is a fine answer and a"
    print "  better one than a guess: a receipt with a guess in it is worse than"
    print "  one with a gap."
    print ""
    let qs = [
        { k: "R3-error-shown",   q: "Did an error message appear on screen after the wrong password?" }
        { k: "R3-error-text",    q: "What did it say, as near as you can recall?" }
        { k: "R3-prompt-back",   q: "Did a usable prompt come back afterwards?" }
        { k: "R3-second-worked", q: "Did the second attempt, with the real password, reach a session?" }
        { k: "R1-line-drawn",    q: "Was the extra PAM line drawn on screen?" }
        { k: "R1-line-dwell",    q: "Did the ~4 s pass WITH the line showing, or blank-then-flash?" }
        { k: "date-language",    q: "Was the date on the login screen Dutch or English?" }
        { k: "anything-else",    q: "Anything else you noticed?" }
    ]
    $lines = ($lines | append ["" "── operator, by eye. Not measurements. ──"])
    for item in $qs {
        let a = if $can_ask { (ask $"    ($item.q)\n    > ") } else { "(not asked — no terminal, or --no-pause)" }
        let a = (if ($a | str trim | is-empty) { "?" } else { $a | str trim })
        $lines = ($lines | append $"operator: ($item.k): ($a)")
        $lines | str join "\n" | save -f $out
    }

    print ""
    print "  Remember the two things that are still yours to do:"
    print "    * spike/p1ab/PAM.md §4 — take the pam_exec line back out."
    print "    * the root one-liner in step 7 above, if the greeter's locale"
    print "      could not be read. It needs greetd to still be running."
    $lines = ($lines | append [
        ""
        "── still outstanding when this file was written ──"
        "PAM.md §4 — remove the pam_exec line from /etc/pam.d/greetd"
        "the greeter's own LC_*/LANG, if /proc/<pid>/environ was unreadable above"
    ])

    $lines | append "" | str join "\n" | save -f $out
    print ""
    print $"written to ($out)"
    print "Ctrl+Alt+F1 to go back to your desktop."
}

# greetd's pid, or null. `pgrep -x` so a `greetd-session.nu` in the process
# list is not mistaken for greetd itself.
def greetd-pid []: nothing -> string {
    let r = (do { ^pgrep -x greetd } | complete)
    if $r.exit_code != 0 { return "" }
    $r.stdout | lines | each { |l| $l | str trim } | where { |l| $l != "" } | get -o 0 | default ""
}

# Every pid running the greeter, as a sorted list. Matched on the installed
# path rather than the process name: `auroval-greet` is run by cage, which is
# run by a shell, and the binary name alone also matches the shell's argv.
def greeter-pids [] {
    let r = (do { ^pgrep -f "auroval-greet" } | complete)
    if $r.exit_code != 0 { return [] }
    let me = ($nu.pid | into string)
    $r.stdout | lines
    | each { |l| $l | str trim }
    | where { |l| $l != "" and $l != $me }
    | sort
}

# The PAM helper's log, as lines, or an empty list. Readable as you because
# root writes it with the default umask; see pam-extra-prompt.sh.
def pam-log-lines []: nothing -> list<string> {
    if not ($PAM_LOG | path exists) { return [] }
    # `try`, because "the file exists but root made it unreadable" is a real
    # outcome and not one worth aborting a sitting over.
    try {
        open --raw $PAM_LOG | lines | each { |l| $l | str trim } | where { |l| $l != "" }
    } catch {
        []
    }
}

# Ask the operator something. `input` needs a terminal, and on a real console
# it has one — but piped into a non-tty, which is how this script gets smoke-
# tested before a sitting, reedline raises rather than returning EOF. Every
# prompt goes through here so that a missing terminal costs an unanswered
# question and not an aborted run with the receipt half written.
def ask [prompt: string]: nothing -> string {
    try { input $prompt } catch { "" }
}

# The repository root, for the `cd` in the printed instructions. The script
# lives in spike/p1ab/, so two levels up.
def pwd-of-repo [] {
    $env.FILE_PWD | path dirname | path dirname
}
