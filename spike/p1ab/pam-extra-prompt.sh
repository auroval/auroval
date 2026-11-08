#!/bin/sh
# P1/A — the extra step in greetd's PAM stack, so R1 has something to prove.
#
# THIS FILE IS /bin/sh AND NOT NUSHELL, AND THAT IS DELIBERATE. Everything
# else in this repository is nushell by standing rule; this one cannot be.
# PAM's pam_exec module execve()s this path directly from inside the auth
# conversation: there is no login shell, no user session, PATH is whatever
# pam_getenvlist(3) happened to carry (usually nothing), and it runs as root
# because greetd runs as root. /bin/sh is the only interpreter certain to
# exist in that context. See spike/p1ab/PAM.md for the rest of the reasoning.
#
# WHAT IT IS FOR
#
# R1 says the greeter must make no assumption about the number, order or type
# of messages greetd sends, and its fit criterion is "a PAM stack carrying an
# extra pam_exec prompt completes login with no code change". crates/greet/
# src/auth.rs has a branch for exactly that — an auth_message that is a
# statement rather than a question gets rendered and answered with
# `PostAuthMessageResponse { response: None }` — and that branch has never
# been run against a real PAM stack. This script is what runs it.
#
# WHAT IT ACTUALLY PRODUCES — READ THIS BEFORE BELIEVING THE FILENAME
#
# An `info` message, not a prompt. pam_exec with the `stdout` option relays
# this script's stdout to the calling application through pam_info(3), which
# is PAM_TEXT_INFO, which greetd forwards as
# `auth_message_type: "info"`. pam_exec never calls the conversation to read
# a response, so it cannot produce PAM_PROMPT_ECHO_ON/OFF — it cannot ask a
# question at all. The name `pam-extra-prompt.sh` is the name of the thing in
# R1's fit criterion, not a claim about this file. PAM.md §6 says what a real
# extra *question* needs instead.
#
# An `info` is still the half of R1 that matters, because it is the half that
# breaks a fixed two-field greeter: a message arrives where no message was
# designed to arrive, and it must be drawn and answered with no response.
#
# SAFETY
#
#   * It is `optional` in the stack (see PAM.md), so nothing it does can
#     refuse a login.
#   * It exits 0 on every path, including the paths it does not understand.
#   * It writes one line to a log and prints two lines. It changes nothing.
#   * It is run as root, so it must live somewhere only root can write.
#     PAM.md installs it to /usr/local/lib/auroval/, mode 0755, root-owned.

# pam_exec's child inherits the PAM environment, which on this stack is close
# to empty. Never rely on an inherited PATH in something root executes.
PATH=/usr/bin:/bin
export PATH

# Only the auth stage. The module line in PAM.md also says `type=auth`, so
# this is belt and braces — but a stray `account` or `session` copy of the
# line would otherwise fire this script at times the greeter is not even
# looking at a conversation, and the log would stop meaning anything.
[ "${PAM_TYPE:-}" = "auth" ] || exit 0

# How long to hold the conversation open AFTER the lines above have been
# relayed. Without it the sitting cannot see anything: auth.rs answers an
# `info` the instant it arrives, PAM moves straight on to the password
# prompt, and the message is overwritten within milliseconds. Those few
# seconds are the difference between "R1 verified" and "something flashed".
#
# It is also the measurement that tells you HOW pam_exec relays: if the
# greeter sits for this many seconds with the lines ON SCREEN, pam_exec
# streams line by line; if it sits blank and then flashes them, pam_exec
# buffers until the child exits. Either way R1 is exercised; only the first
# is watchable. The operator's script in greetd-round.nu asks about this.
#
# Lower it to 0 for a stack you intend to leave in place. Do not leave this
# line in place.
HOLD=4

LOG=/var/log/auroval-pam-extra.log

# The receipt, written FIRST, before anything that could be slow or could
# fail. This log is the only part of the sitting a non-root harness can
# observe: it is created by root with the default umask, so mode 0644, so
# greetd-round.nu can read it as you and prove from it that PAM really ran
# this script, how many times, and when. That timeline is what turns R3's
# "the process did not exit" from an eye-witness claim into a measurement.
#
# Appended, never truncated: two attempts in one sitting is the entire point.
{
    printf '%s service=%s type=%s user=%s tty=%s hold=%s\n' \
        "$(date -Is 2>/dev/null || echo unknown-time)" \
        "${PAM_SERVICE:-?}" \
        "${PAM_TYPE:-?}" \
        "${PAM_USER:-?}" \
        "${PAM_TTY:-?}" \
        "$HOLD"
} >> "$LOG" 2>/dev/null

# The two lines the greeter has to draw and was never coded for. Deliberately
# not shaped like a question: it is not one, and telling the operator to type
# an answer to something that cannot take one would make the sitting unreadable.
echo "auroval P1/A: an extra PAM step is in this stack (R1)."
echo "Nothing to answer. This line is the test; the greeter drew it unprompted."

# Hold the conversation. `sleep` is in coreutils, which is in /usr/bin, which
# is on the PATH set above; if it is somehow missing the test still works,
# just too fast to watch, so the failure is ignored on purpose.
[ "$HOLD" -gt 0 ] 2>/dev/null && sleep "$HOLD" 2>/dev/null

# Always. A greeter that cannot be logged into because a spike script
# returned non-zero is a worse outcome than a spike that proves nothing.
exit 0
