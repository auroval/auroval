#!/bin/sh
# greetd starts this as the authenticated user once `start_session` succeeds.
# Its only job is to leave evidence that the login really completed, as the
# right user, in a real session — then exit so the VT is released.
out="$HOME/auroval-p0-login-proof.txt"
{
    echo "greetd login succeeded via the auroval P0 spike"
    date -Is
    echo
    echo "whoami : $(id -un)"
    echo "id     : $(id)"
    echo "HOME   : $HOME"
    echo "shell  : $SHELL"
    echo
    echo "-- session, as greetd/logind set it up --"
    echo "XDG_SESSION_TYPE  : ${XDG_SESSION_TYPE-<unset>}"
    echo "XDG_SESSION_ID    : ${XDG_SESSION_ID-<unset>}"
    echo "XDG_RUNTIME_DIR   : ${XDG_RUNTIME_DIR-<unset>}"
    echo "XDG_VTNR          : ${XDG_VTNR-<unset>}"
    echo "XDG_SESSION_CLASS : ${XDG_SESSION_CLASS-<unset>}"
} > "$out" 2>&1
chmod 0644 "$out" 2>/dev/null
# Stay alive. greetd restarts the greeter the moment a session ends, and
# re-activates its own VT when it does — so a short-lived session makes the
# console bounce back every few seconds. The orchestrator stops greetd once it
# sees the proof file above, which is what ends the test.
sleep 600
