#!/usr/bin/env nu
#
# P1/E — R15's criterion, rehearsed: the greeter renders its configured
# background while the maintainer's home is mode 700.
#
# R15's fit criterion, and the last unticked box on issue #34:
#
#     With /home/<user> at mode 700, the configured background still renders.
#
# It has been deferred twice as "needs a real login". It does not. The claim
# has two halves and only one of them is about logging in:
#
#   * can the greeter, AS THE `greeter` USER, read what it was configured
#     with while /home/dirk is closed to it — and does the background then
#     arrive on screen
#   * does a session start
#
# The first half is the whole of R15 and it needs no compositor, no greetd and
# no DRM master, because `snapshot` renders one frame through surfaceless EGL.
# What it does need is to be the `greeter` user, and that is not a detail to
# work around: it IS the question. Run as you, the answer is always yes,
# because you own the home the test is about.
#
# So this script needs root for exactly one thing — `runuser`, to become
# someone else — and it refuses to pretend otherwise. It never runs sudo
# itself; this machine trips pam_faillock on a failed attempt and has locked
# its maintainer out before. Run as anyone but root it prints what it would
# do, prints the one command to run, and exits.
#
#     sudo nu spike/p1e/greeter-render.nu
#
# WHY `runuser -u greeter -- env -i` AND NOT `sudo -u greeter`.
#
# Because a rehearsal that inherits your environment is rehearsing the wrong
# thing. greetd clears the environment and hands its session only the PAM env
# list plus its own XDG variables; `sudo -u greeter env` would carry your
# PATH, your XDG_RUNTIME_DIR, your GST_* overrides and your HOME, several of
# which would make the greeter work for reasons the real greeter will not
# have. `env -i` plus an explicit list is the same method spike/p0 used and
# the same one greeter-probe.nu beside this file uses, and the explicit list
# is deployment's own: deploy/auroval-greet.sh's four variables, nothing else.
#
# WHAT IT MEASURES. Two snapshots, both as `greeter`, both into
# /var/cache/auroval because that is the one directory the greeter owns:
#
#   * a REFERENCE with no background configured, which is the solid colour by
#     definition. Without it "it fell back" would be a hard-coded hex that
#     starts lying the day the palette changes.
#   * the REAL one, through /etc/auroval/greet.lua exactly as deployed.
#
# A frame of video has six figures of distinct colours; the fallback has a
# couple of thousand, all of them the login field and its text. That count,
# against the reference's, is what "the background arrived" rests on.
#
# And it records the mode of /home/dirk at the time, and has the greeter try
# to read it, so the receipt carries its own precondition instead of asserting
# it. A pass against a home that had quietly gone back to 755 is worth
# nothing.
#
# It installs nothing and changes no system configuration. It writes two PNGs
# and one text file.

const BIN    = "/usr/local/lib/auroval/auroval-greet"
const CONFIG = "/etc/auroval/greet.lua"
const CACHE  = "/var/cache/auroval"
const STATE  = "/var/lib/auroval"
const TOTAL  = 7

def main [
    --bin: string = ""      # override the binary under test
    --home: string = ""     # the home that must be unreadable; default: the invoking user's
    --timeout: int = 60     # seconds to allow one snapshot before giving up
    --keep                  # leave the PNGs in /var/cache/auroval afterwards
] {
    let bin = if ($bin | is-empty) { $BIN } else { $bin }
    # Whose home R15 is about. Under sudo the invoking user is the maintainer,
    # which is exactly the home that is mode 700 and exactly the one the
    # greeter must not need.
    let who = ($env | get -o SUDO_USER | default "dirk")
    let home = if ($home | is-empty) {
        (^getent passwd $who | str trim | split row ":" | get -o 5 | default $"/home/($who)")
    } else { $home }

    # ── the refusal, and the whole of what it would do ───────────────────
    if (id -u | into int) != 0 {
        print "This needs root, and this script will not acquire it for you."
        print ""
        print "It uses root for one thing only: `runuser`, to become the"
        print "`greeter` user. That is not incidental — run as you, the answer"
        print "is always yes, because you own the home the test is about."
        print ""
        print "What it would do, as root:"
        print $"  1. record the mode of ($home), and have `greeter` try to read it"
        print $"  2. check ($CONFIG), the file it points at, ($CACHE), and that"
        print $"     the INSTALLED binary is new enough to have a `snapshot` mode"
        print $"  3. runuser -u greeter -- env -i <the four deployment vars> \\"
        print $"       ($bin) snapshot ($CACHE)/r15-reference.png"
        print "     with no background configured — the solid colour, for comparison"
        print $"  4. the same again through ($CONFIG) as deployed"
        print "  5. compare the two frames' colour counts, and report what the"
        print "     greeter said to stderr either way"
        print ""
        print "It changes no system configuration. Run it yourself:"
        print ""
        print "    sudo nu spike/p1e/greeter-render.nu"
        print ""
        exit 2
    }

    let out = ($"($home)/auroval-p1e-greeter-render.txt")
    mut step = 0
    let started = (date now | format date "%Y-%m-%dT%H:%M:%S%:z")
    mut lines = [
        "auroval P1/E — R15: the configured background with /home at mode 700"
        $started
        ""
        $"binary    ($bin)"
        $"config    ($CONFIG)"
        $"home      ($home)   \(the one R15 says must not be needed)"
        $"host      (sys host | get hostname)  kernel (sys host | get kernel_version)"
        $"method    runuser -u greeter -- env -i ...   \(greetd clears the environment; so does this)"
        ""
    ]
    # Written before the first run and again after every step. The real run
    # can sit for the whole timeout while GStreamer builds a cold registry,
    # and a receipt that only exists after the last step is a receipt you lose
    # the first time you lose patience.
    $lines | str join "\n" | save -f $out

    # ── 1: the greeter user ──────────────────────────────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  the greeter user"
    if (^id -u greeter | complete).exit_code != 0 {
        print "         there is no `greeter` user — is greetd installed?"
        exit 2
    }
    let greeter_home = (^getent passwd greeter | str trim | split row ":" | get 5)
    print $"         uid (^id -u greeter | str trim), home ($greeter_home)"
    $lines = ($lines | append [
        $"greeter   uid (^id -u greeter | str trim), home ($greeter_home), groups (^id -nG greeter | str trim)"
    ])
    $lines | str join "\n" | save -f $out

    # ── 2: the precondition, measured rather than assumed ────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  the precondition: ($home)"
    let mode = (^stat -c '%a %U:%G' $home | str trim)
    let mode_only = ($mode | split row " " | first)
    print $"         ($mode)"
    # Asking the greeter itself is stronger than reading the mode bits: it
    # settles group membership, ACLs and anything else at once.
    let peek = (do { ^runuser -u greeter -- env -i /usr/bin/ls $home } | complete)
    let shut_out = ($peek.exit_code != 0)
    print $"         greeter can list it: (not $shut_out)"
    if not $shut_out {
        print $"         ($home) is NOT closed to the greeter, so a pass here would"
        print "         prove nothing about R15. Set it and run this again:"
        print $"             sudo chmod 700 ($home)"
        $lines = ($lines | append [
            $"PRECONDITION FAILED — ($home) is ($mode) and the greeter can read it."
            "Nothing below would be about R15."
        ])
        $lines | str join "\n" | save -f $out
        exit 1
    }
    $lines = ($lines | append [
        $"home      ($home) is mode ($mode_only), owner ($mode | split row ' ' | get 1)"
        $"          greeter's own attempt to list it: ($peek.stderr | str trim)"
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 3: the things deployment must have put there ─────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  binary, config, background, cache"
    if not ($bin | path exists) {
        print $"         ($bin) is missing — run: nu deploy/install.nu"
        exit 2
    }
    # The deployed binary, not the one in the repo, and deliberately so: the
    # repo's target/release sits under the very home R15 closes, so the
    # greeter cannot reach it. That makes `nu deploy/install.nu` a precondition
    # of this rehearsal rather than a convenience — and on 2026-11-08 the
    # installed binary was old enough to predate `snapshot` entirely, which
    # this would otherwise have reported as the greeter refusing to draw.
    let usage = (do { ^$bin } | complete)
    if not (($usage.stdout + $usage.stderr) | str contains "snapshot") {
        print $"         ($bin) has no `snapshot` mode — it predates this test."
        print "         Rebuild and reinstall first:"
        print "             cargo build --release -p greet"
        print "             nu deploy/install.nu"
        print "         then the root command it prints, then run this again."
        exit 2
    }
    if not ($CONFIG | path exists) {
        print $"         ($CONFIG) is missing — run: nu deploy/install.nu"
        exit 2
    }
    let configured = (
        open $CONFIG
        | parse -r '(?s)background\s*=\s*\{?\s*"([^"]+)"'
        | get -o capture0.0
        | default ""
    )
    if ($configured | is-empty) {
        print $"         ($CONFIG) configures no background — there would be"
        print "         nothing for R15 to render. The file says:"
        print (open $CONFIG)
        exit 2
    }
    print $"         configured background: ($configured)"
    # Readable BY THE GREETER, not by root. Root can read anything, which is
    # why the audit this rehearsal replaces could not answer the question.
    let bg_ok = (do { ^runuser -u greeter -- env -i /usr/bin/test -r $configured } | complete)
    print $"         greeter can read it: ($bg_ok.exit_code == 0)"
    let own = (^stat -c '%U:%G %a' $CACHE | str trim)
    print $"         ($CACHE)  ($own)"
    if not ($own | str starts-with "greeter:greeter") {
        print $"         ($CACHE) is not the greeter's — install /etc/tmpfiles.d/auroval.conf"
        exit 2
    }
    $lines = ($lines | append [
        $"configured ($configured)"
        $"           mode (^stat -c '%U:%G %a' $configured | str trim); greeter can read it: ($bg_ok.exit_code == 0)"
        $"cache      ($CACHE)  ($own)"
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 4: a config with no background, for the reference frame ──────────
    $step += 1
    print $"  ($step)/($TOTAL)  writing the reference config"
    let nobg = $"($CACHE)/r15-nobg.lua"
    "return { }\n" | save -f $nobg
    ^chmod 644 $nobg
    print $"         ($nobg)"

    # ── 5: the reference snapshot, as greeter ────────────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  reference snapshot as `greeter`: no background"
    let ref_png = $"($CACHE)/r15-reference.png"
    let reference = (snap $bin $nobg $ref_png $timeout)
    if not $reference.drew {
        print "         the greeter drew nothing even with no background at all,"
        print "         so nothing below would be about R15. It said:"
        print $reference.err
        $lines = ($lines | append [
            "reference FAILED — the greeter as `greeter` cannot draw at all:"
            $reference.err
        ])
        $lines | str join "\n" | save -f $out
        exit 1
    }
    print $"         ($reference.colours) colours, corner ($reference.corner), exit ($reference.exit) in ($reference.secs)"
    $lines = ($lines | append [
        "reference — as greeter, no background configured: the solid colour"
        $"  colours ($reference.colours)   \(the login field and its text, nothing else)"
        $"  corner  ($reference.corner)"
        $"  exited  ($reference.exit) in ($reference.secs)"
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 6: the real one, through the deployed config ─────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  the real snapshot as `greeter`: ($CONFIG)"
    print "         (a cold GStreamer registry costs about a second here)"
    let real_png = $"($CACHE)/r15-background.png"
    let real = (snap $bin $CONFIG $real_png $timeout)
    print $"         drew ($real.drew), ($real.colours) colours, corner ($real.corner)"
    print $"         exit ($real.exit) in ($real.secs)"
    for l in ($real.err | lines | first 6) { print $"         | ($l)" }
    $lines = ($lines | append [
        $"real — as greeter, through ($CONFIG)"
        $"  drew    ($real.drew)"
        $"  colours ($real.colours)"
        $"  corner  ($real.corner)"
        $"  exited  ($real.exit) in ($real.secs)"
        $"  png     ($real_png)"
        $"  stderr  ($real.err | lines | length) lines:"
        ($real.err | lines | each { |l| $"          ($l)" } | first 20 | str join "\n")
        ""
    ])
    $lines | str join "\n" | save -f $out

    # ── 7: the verdict ───────────────────────────────────────────────────
    $step += 1
    print $"  ($step)/($TOTAL)  verdict"
    # "The background arrived" is the colour count, not the absence of a
    # complaint: a frame of 1440p video carries six figures of distinct
    # colours and the fallback carries the login field's couple of thousand.
    # The factor of ten is the margin, and the reference is the baseline, so
    # this keeps working when the palette or the clip changes.
    let arrived = ($real.drew and ($real.colours > ($reference.colours * 10)))
    let fellback = ($real.err | str contains "falling back to the solid colour")
    let verdict = if ($arrived and (not $fellback)) { "PASS" } else { "FAIL" }
    let said = (
        $real.err | lines
        | where { |l| ($l | str contains "[background]") }
        | first
        | default "(the greeter said nothing about the background)"
    )
    mut tail = [
        "────────────────────────────────────────────────────────────────"
        $"VERDICT  R15 'with /home at mode 700, the configured background still renders': ($verdict)"
        ""
        $"  home      ($home) was mode ($mode_only) and the greeter's own `ls` was refused:"
        $"              ($peek.stderr | str trim)"
        $"  rendered  ($real.colours) distinct colours against the solid colour's ($reference.colours)"
        $"            — a factor of ((($real.colours) / ([$reference.colours 1] | math max)) | math round), so there is a picture behind the login field"
        $"  said      ($said)"
        $"  fellback  ($fellback)   \(a fallback line anywhere in stderr would mean the opposite of a pass)"
        ""
        "  This covers R15 entirely: R15 is about whether the greeter can"
        "  reach its configured background as itself, and it can. It does NOT"
        "  cover logging in — snapshot never speaks to greetd or PAM. That is"
        "  spike/p1ab/greetd-round.nu's sitting."
        ""
        $"frames   ($ref_png)"
        $"         ($real_png)"
        $"receipt  ($out)"
    ]
    for l in $tail { print $"         ($l)" }
    $lines = ($lines | append $tail)
    $lines | str join "\n" | save -f $out
    # The receipt is written into the maintainer's home by root, so it would
    # otherwise be a root-owned file in a directory he owns: readable, but not
    # his to edit or delete without sudo again.
    ^chown $"($who):($who)" $out
    rm -f $nobg
    if not $keep {
        rm -f $ref_png $real_png
        print $"         \(frames removed; pass --keep to look at them\)"
    }
}

# One snapshot, as `greeter`, with greetd's own environment and nothing else.
#
# The four variables are deploy/auroval-greet.sh's, verbatim, because that is
# the file greetd actually runs. HOME is `/`, which is the greeter's real home
# and the reason C15's cache variables exist at all. XDG_RUNTIME_DIR is
# deliberately absent: pam_systemd gives the real session one, and a
# surfaceless snapshot has no socket to find in it, so adding it here would be
# inventing an advantage the test does not need.
def snap [bin: string, cfg: string, png: string, secs: int] {
    let envs = [
        "HOME=/"
        "USER=greeter"
        "LOGNAME=greeter"
        "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin"
        $"XDG_CACHE_HOME=($CACHE)"
        $"XDG_STATE_HOME=($STATE)"
        $"GST_REGISTRY=($CACHE)/gst-registry.bin"
        $"AUROVAL_CONFIG=($cfg)"
    ]
    rm -f $png
    let began = (date now)
    let r = (do { ^timeout $secs runuser -u greeter -- env -i ...$envs $bin snapshot $png } | complete)
    let elapsed = ((date now) - $began)
    let drew = ($png | path exists)
    {
        exit: $r.exit_code
        secs: ($elapsed | format duration sec)
        drew: $drew
        out: $r.stdout
        err: $r.stderr
        corner: (if $drew { corner_of $png } else { "n/a" })
        colours: (if $drew { colours_of $png } else { 0 })
    }
}

# The top-left pixel as "r,g,b" in 0-255 — recorded rather than tested
# against, because the first frame of a dark clip can sit very close to the
# fallback colour and the colour count is the honest discriminator.
def corner_of [png: string] {
    (^magick $png -format "%[fx:int(255*p{0,0}.r)],%[fx:int(255*p{0,0}.g)],%[fx:int(255*p{0,0}.b)]" info:
        | str trim)
}

# How many distinct colours the frame holds. This is the measurement the
# verdict rests on.
def colours_of [png: string] {
    (^identify -format "%k" $png | str trim | into int)
}
