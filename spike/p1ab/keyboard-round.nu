#!/usr/bin/env nu
#
# P1/A+B — R6's receipt: keyboard input through wl_seat, held down by a person.
#
# R6 asks for keyboard input "including key repeat and caps lock". Neither half
# of that is reachable from a machine nobody is sitting at. A unit test has no
# seat, so there is nothing for the compositor to repeat; `snapshot` mode has no
# keyboard at all, so there is nothing to press. The only instrument that
# produces this evidence is a person on a real VT holding a key down, and the
# only thing that turns "it felt right" into a number is a log of what the
# compositor actually delivered.
#
# So this is a harness for exactly one sitting. It runs the greeter under cage
# with AUROVAL_LOG_KEYS set, keeps the stderr that comes back, and then reads
# that log and says, check by check, which part of R6 it covers and on which
# number.
#
# The sitting is twenty seconds of pressing keys inside a forty-five second
# ceiling, and the ceiling is deliberate. P1/E's first attempt asked its
# operator to sit still on a VT for five and a half minutes; they did not, they
# switched away halfway, and the abandonment looked like a bug in the greeter.
# A run that is over before you start wondering whether it has hung is worth
# more than any amount of error reporting around a long one.
#
# NO SUDO. cage takes DRM master through logind from the VT you are already
# logged in on, which is the whole of what this needs. It writes one file in
# your home directory and touches no system configuration.
#
# Run it like this:
#
#   1. Ctrl+Alt+F3, log in as yourself
#   2. cd ~/Documents/projects/auroval
#      nu spike/p1ab/keyboard-round.nu
#   3. Read the instructions it prints, and only then press Enter. Once cage has
#      the screen the console is gone and nothing printed after that moment can
#      be read, which is why the briefing comes first and waits.
#   4. Ctrl+Alt+F1 to come back to your desktop
#
# Escape route: the run is wrapped in `timeout`, and that is the only way out —
# `preview` draws forever and its Escape key clears the field rather than
# quitting. Worst case is forty-five seconds of greeter and then your console
# back, with the receipt already on disk.

const OUT = "~/auroval-p1ab-keyboard.txt"
const FALLBACK = "/usr/local/lib/auroval/auroval-greet"

# How long the held Backspace is meant to last, in seconds. The briefing and
# the repeat-count sanity check have to agree about this, so it is one number.
const HOLD_S = 2

def main [
    --bin: string = ""          # override the binary under test
    --timeout: int = 45         # seconds before the run is killed; the only exit
    --log: string = ""          # skip cage and parse this log file instead
] {
    let repo = ($env.FILE_PWD | path dirname | path dirname)
    let out = ($OUT | path expand)
    let steps = 6
    mut step = 0

    # ── the refusal that keeps the result honest ─────────────────────────
    # Nested in a desktop session, cage gets its keyboard from Hyprland, which
    # does its own repeat and its own caps-lock handling. Every number below
    # would then be a measurement of the desktop compositor, not of the greeter
    # on the seat it will actually run on.
    if ($env | get -o WAYLAND_DISPLAY | is-not-empty) and ($log | is-empty) {
        print "WAYLAND_DISPLAY is set, so this would be a nested session: cage"
        print "would take its seat from your desktop compositor, and the repeat"
        print "rate and caps-lock state logged below would be Hyprland's, not"
        print "the ones the greeter will meet on a real seat. R6 cannot be"
        print "settled that way."
        print ""
        print "This needs a real TTY. Ctrl+Alt+F3, log in as yourself, and run"
        print "it there. Ctrl+Alt+F1 brings you back afterwards."
        exit 2
    }
    if ($env | get -o DISPLAY | is-not-empty) and ($log | is-empty) {
        print "DISPLAY is set — an X session is in the way. Use a free VT"
        print "(Ctrl+Alt+F3), not a terminal inside a running session."
        exit 2
    }

    # ── 1: the binary ────────────────────────────────────────────────────
    $step = $step + 1
    print $"  ($step)/($steps)  binary"
    let built = $"($repo)/target/release/auroval-greet"
    let bin = if ($bin | is-not-empty) {
        $bin
    } else if ($built | path exists) {
        $built
    } else {
        $FALLBACK
    }
    if ($log | is-empty) and not ($bin | path exists) {
        print $"         ($built) is missing,"
        print $"         and so is ($FALLBACK)."
        print "         Nothing here builds it for you: a greeter built by the"
        print "         harness that measures it is a greeter nobody chose to"
        print "         ship. Build it yourself and run this again:"
        print ""
        print "           cargo build --release -p greet"
        exit 2
    }
    let which_bin = if ($bin == $built) { "the one built in this tree" } else if ($bin == $FALLBACK) { "the deployed one" } else { "overridden with --bin" }
    print $"         ($bin)  \(($which_bin))"

    # ── 2: cage ──────────────────────────────────────────────────────────
    $step = $step + 1
    print $"  ($step)/($steps)  cage"
    if ($log | is-empty) and (which cage | is-empty) {
        print "         cage is not installed — pacman -S cage"
        exit 2
    }
    print "         present"

    # ── 3: the receipt ───────────────────────────────────────────────────
    # Opened before the run, not written after it. A sitting you can abort and
    # still have evidence from is a sitting the operator will agree to repeat,
    # and P1/E lost a whole run to output that was flushed only on exit.
    $step = $step + 1
    print $"  ($step)/($steps)  receipt"
    let started = (date now | format date "%Y-%m-%dT%H:%M:%S%:z")
    mut lines = [
        "auroval P1/A+B — keyboard through wl_seat, with a person holding a key (R6)"
        $started
        ""
        $"binary    ($bin)  \(($which_bin))"
        $"timeout   ($timeout) s  — the only way the run ends"
        $"host      (sys host | get hostname)  kernel (sys host | get kernel_version)"
        $"tty       (do { ^tty } | complete | get stdout | str trim)"
        ""
    ]
    $lines | str join "\n" | save -f $out
    print $"         ($out)"

    # ── 4: the briefing, and the wait ────────────────────────────────────
    # Everything the operator needs has to be on screen now. In a moment cage
    # owns the display, and from then until the timeout fires there is no
    # console to read, no prompt to answer and no way to ask what to do next.
    $step = $step + 1
    print $"  ($step)/($steps)  briefing"
    let captured = if ($log | is-not-empty) {
        print $"         reading ($log) instead of running cage"
        open --raw $log
    } else {
        brief $timeout
        input "Press Enter when you have read that and are ready. "
        print ""
        print $"  ($step)/($steps)  the run — the screen goes to the greeter now"
        print $"         it ends by itself in at most ($timeout) s"
        run-once $bin $timeout $out
    }

    # ── 5: what came back ────────────────────────────────────────────────
    $step = $step + 1
    print ""
    print $"  ($step)/($steps)  log"
    let keys = parse-keys $captured
    let caps = parse-caps $captured
    let info = parse-info $captured
    $lines = ($lines | append ["── captured log ──" $captured ""])
    $lines | str join "\n" | save -f $out

    if ($keys | is-empty) and ($caps | is-empty) and ($info | is-empty) {
        # The two causes are worth telling apart out loud, because the fix is
        # completely different and a bare exit code sends the operator to the
        # wrong one. R6 is not failed by this; it is simply unmeasured.
        print "         nothing. The log has no `keys` lines at all."
        print ""
        print "  Either the greeter never saw a seat — cage announced no"
        print "  wl_keyboard, so there was nothing to press and nothing to"
        print "  repeat, which is a greeter-side or seat-side fault — or no key"
        print "  was pressed during the run, which is just a sitting to take"
        print "  again. The captured output above tells you which: a greeter"
        print "  that started and drew says so, a greeter that never got a seat"
        print "  usually says that too."
        print ""
        print $"  R6 is NOT measured by this run. Receipt: ($out)"
        $lines = ($lines | append [
            "── verdict ──"
            "R6 NOT MEASURED — the log contains no `keys` lines."
            "Cause is one of: no wl_keyboard was announced (no seat), or nothing was pressed."
        ])
        $lines | append "" | str join "\n" | save -f $out
        exit 1
    }
    print $"         ($keys | length) key events, ($caps | length) caps-lock changes, ($info | length) repeat-info"

    # ── 6: the verdict ───────────────────────────────────────────────────
    $step = $step + 1
    print ""
    print $"  ($step)/($steps)  verdict"
    print ""
    let verdict = (judge $keys $caps $info)
    $lines = ($lines | append ["── verdict ──"] | append ($verdict | each { |c| $"[(if $c.ok { 'PASS' } else { 'FAIL' })] ($c.name) — ($c.detail)" }))
    let failed = ($verdict | where not ok | length)
    print ""
    if $failed == 0 and $log != "" {
        # A receipt that cannot tell a real sitting from a replayed file is
        # worse than no receipt: --log exists to test this parser, and the
        # numbers it just printed came from a file somebody typed.
        print "  Every check passed, but this was --log: the numbers came from"
        print $"  ($log), not from a seat. R6 is NOT covered by this run."
        $lines = ($lines | append $"REPLAY ONLY — parsed ($log); proves the parser, not the greeter.")
    } else if $failed == 0 {
        print "  Every check passed: R6's keyboard path is exercised on a real"
        print "  seat, with repeat and caps lock, on the numbers above."
        $lines = ($lines | append "R6 COVERED — all checks passed on this sitting.")
    } else {
        print $"  ($failed) of ($verdict | length) checks failed. A failed check here is"
        print "  usually one of two things, and the detail line says which: the"
        print "  greeter did not get what the compositor sent, or the operator"
        print "  did not do that part of the script. Re-read the four steps and"
        print "  take the sitting again before touching any code."
        $lines = ($lines | append $"R6 PARTIAL — ($failed) of ($verdict | length) checks failed on this sitting.")
    }
    $lines | append "" | str join "\n" | save -f $out
    print ""
    print $"written to ($out)"
    if ($log | is-empty) {
        print "Ctrl+Alt+F1 to go back to your desktop."
    }
}

# The briefing. Short, numbered, and in the order the parser expects to find
# the events, because an operator improvising the order is an operator whose
# backspace burst overlaps the caps-lock change.
def brief [timeout: int] {
    print ""
    print "  ───────────────────────────────────────────────────────────────"
    print "  In a moment the greeter takes the whole screen. Do these four"
    print "  things, in this order, taking about twenty seconds over them:"
    print ""
    print "    1. Type a few characters — five or six, anything at all."
    print $"    2. Hold BACKSPACE down for about ($HOLD_S) seconds, until the field"
    print "       is empty and stays empty. This is the one that matters:"
    print "       holding it is what makes the compositor repeat."
    print "    3. Press CAPS LOCK on. Wait a second. Press it off again."
    print "    4. Press TAB once."
    print ""
    print "  Then stop and wait. The run ends by itself — you do not have to"
    print $"  quit anything, and you cannot: it is killed after ($timeout) s and"
    print "  that is the only exit. Escape clears the field, it does not quit."
    print "  Do NOT press Enter during the run; that submits a login."
    print ""
    print "  You will not be stranded. When the timeout fires you get this"
    print "  console back with the verdict on it, and Ctrl+Alt+F1 returns you"
    print "  to your desktop, which has been sitting untouched on its own VT"
    print "  the whole time."
    print "  ───────────────────────────────────────────────────────────────"
    print ""
}

# One run under cage, with the keyboard log on. Returns the captured output and
# writes it to the receipt before returning it, so a run killed between here
# and the verdict still leaves the evidence behind.
def run-once [bin: string, timeout: int, out: string]: nothing -> string {
    let r = (do {
        with-env { AUROVAL_LOG_KEYS: "1" } {
            # -s: let the operator leave. cage blocks VT switching by
            # default, so without it the timeout below is the ONLY way out
            # of this run, which is a cruel thing to do to someone who is
            # only here to hold a key down.
            ^timeout $"($timeout)s" cage -s -- $bin preview
        }
    } | complete)
    let text = ($"($r.stderr)\n($r.stdout)" | str trim)
    # 124 is timeout doing its job, which is the normal end of this run and not
    # a failure worth reporting as one.
    let how = (if $r.exit_code == 124 { $"timeout after ($timeout) s, as designed" } else { $"exited ($r.exit_code)" })
    [$"run       ($how)" "" $text] | str join "\n" | save -a $out
    $text
}

# `keys   1.234  press   keysym "BackSpace"  field 3 chars`
# (`typed` is accepted too: that was the word before the first sitting
#  showed it was reading the wrong buffer at the username stage.)
#
# The keysym is Debug-formatted in the greeter, so it arrives quoted; the quotes
# are optional here so a hand-written log parses too. The character itself is
# never in the log, deliberately — see `log_key` in shell.rs.
def parse-keys [text: string] {
    $text
    | parse -r '(?m)^keys\s+(?<t>[\d.]+)\s+(?<how>press|repeat)[\w-]*\s+keysym\s+"?(?<sym>[^"\s]+)"?\s+(?:typed|field)\s+(?<n>\d+)\s+chars'
    | each { |r| { t: ($r.t | into float), how: $r.how, sym: $r.sym, n: ($r.n | into int) } }
}

# `keys   2.100  caps-lock on`
def parse-caps [text: string] {
    $text
    | parse -r '(?m)^keys\s+(?<t>[\d.]+)\s+caps-lock\s+(?<state>on|off)'
    | each { |r| { t: ($r.t | into float), state: $r.state } }
}

# `keys   0.050  repeat-info Repeat { rate: 25, delay: 600 }`
def parse-info [text: string] {
    $text
    | parse -r '(?m)^keys\s+(?<t>[\d.]+)\s+repeat-info\s+(?<info>.+)$'
    | each { |r| { t: ($r.t | into float), info: ($r.info | str trim) } }
}

# One named line per check, each with the number it rests on. A check that
# prints PASS without a number is a check nobody can argue with later.
def check [name: string, ok: bool, detail: string] {
    let tag = (if $ok { "PASS" } else { "FAIL" })
    print $"    [($tag)]  ($name | fill -a left -w 22)  ($detail)"
    { name: $name, ok: $ok, detail: $detail }
}

def judge [keys: list, caps: list, info: list] {
    mut out = []

    # ── the announcement ─────────────────────────────────────────────────
    # SCTK does the repeating, from whatever the compositor announced. Without
    # this line there is no rate to hold the measured intervals against, and
    # `Disable` means the compositor asked for no repeat at all — in which case
    # an absence of repeat events below is correct behaviour, not a fault.
    let announced = ($info | get -o 0.info | default "")
    let disabled = ($announced | str starts-with "Disable")
    let rate = ($announced | parse -r 'rate:\s*(?<rate>\d+)' | get -o rate.0)
    let delay = ($info | get -o 0.info | default "" | parse -r 'delay:\s*(?<delay>\d+)' | get -o delay.0)
    $out = ($out | append (check "repeat-info announced" ($info | is-not-empty) (
        if ($info | is-empty) {
            "no repeat-info line — the compositor announced no rate, so there is nothing for the intervals below to agree with"
        } else {
            $"($info | get 0.info)  \(at ($info | get 0.t) s)"
        }
    )))

    # ── the repeats themselves ───────────────────────────────────────────
    let reps = ($keys | where how == "repeat")
    # Only gaps between consecutive repeats of the SAME keysym count. A gap
    # that straddles two bursts is the operator's thinking time, not a repeat
    # interval, and it would drag the median towards nonsense.
    let gaps = (
        if ($reps | length) < 2 { [] } else {
            1..(($reps | length) - 1)
            | each { |i|
                let a = ($reps | get ($i - 1))
                let b = ($reps | get $i)
                if $a.sym == $b.sym { (($b.t - $a.t) * 1000 | math round -p 1) } else { null }
            }
            | where { |g| $g != null }
        }
    )
    let median = (if ($gaps | is-empty) { null } else { $gaps | math median | math round -p 1 })
    # When the compositor announced Disable there is nothing to measure and
    # nothing to fail: no repeat events is then the correct outcome, and saying
    # otherwise would send the operator looking for a fault that is a setting.
    $out = ($out | append (check "repeat events" (($reps | is-not-empty) or $disabled) (
        if ($reps | is-empty) {
            if $disabled {
                "0 repeat events, and the compositor announced Disable — correct, but R6's repeat is then untested on this seat"
            } else {
                "0 repeat events — nothing was held down long enough to make the compositor repeat"
            }
        } else if ($median == null) {
            $"($reps | length) repeat events, but no two consecutive on one key — no interval to take a median of"
        } else {
            $"($reps | length) events over ($gaps | length) intervals, median ($median) ms"
        }
    )))

    # ── do the two numbers agree ─────────────────────────────────────────
    # They are measured at different ends of the same mechanism: one is what
    # the compositor said it would do, the other is when the events actually
    # landed in the greeter's event loop. A handful of milliseconds between
    # them is scheduling, not disagreement, so this check passes on 10% and
    # prints both numbers either way for anyone who wants to argue.
    $out = ($out | append (
        if $disabled {
            check "rate agrees" true "nothing to compare — repeat was announced as Disable, so there is no rate"
        } else if ($rate == null) or ($median == null) {
            check "rate agrees" false $"cannot compare — announced ($rate | default '—')/s against measured ($median | default '—') ms"
        } else {
            let expected = (1000.0 / ($rate | into float) | math round -p 1)
            let off = (($median - $expected) | math abs | math round -p 1)
            let pct = ($off / $expected * 100 | math round -p 1)
            check "rate agrees" ($pct <= 10) $"announced ($rate)/s = ($expected) ms, measured ($median) ms, off by ($off) ms \(($pct)%)"
        }
    ))

    # ── caps lock ────────────────────────────────────────────────────────
    # R6 wants the warning to appear and to go away again, so both edges have
    # to be in the log. Only `on` would leave a greeter that latches.
    let on = ($caps | where state == "on" | length)
    let off = ($caps | where state == "off" | length)
    $out = ($out | append (check "caps lock both ways" (($on > 0) and ($off > 0)) (
        $"($on) on, ($off) off" + (if $on == 0 { " — never saw it come on" } else if $off == 0 { " — came on and never went off; the warning would latch" } else { "" })
    )))

    # ── did the field agree with the keyboard ────────────────────────────
    # The count is logged BEFORE the event is applied, so the last line of a
    # held backspace reads `field 1 chars` with the field about to be empty.
    # That is why the floor is 1 and not 0: it is the shape of the log, not
    # slack in the requirement.
    let counts = ($keys | get n)
    let peak = ($counts | math max)
    let peak_at = ($counts | enumerate | where item == $peak | get 0.index)
    let after = ($counts | skip $peak_at)
    let trough = ($after | math min)
    $out = ($out | append (check "field count rose, fell" (($peak > 0) and ($trough <= 1)) (
        $"rose to ($peak), fell to ($trough)" + (if $peak == 0 { " — no character ever reached the field" } else if $trough > 1 { " — the held backspace did not empty it" } else { "" })
    )))

    $out
}
