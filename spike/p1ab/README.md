# P1/A+B — the console sitting

Four things need a real console, and three of them are about `auth.rs`, still
the only module in this project with neither a test nor a real run behind it.
This directory is what makes them one sitting instead of four.

| | what | harness | needs root |
|---|---|---|---|
| R6 | key repeat and caps lock | `keyboard-round.nu` | no |
| R3 | wrong password returns to the prompt | `greetd-round.nu` | one command, yours to run |
| R1 | an extra PAM message is drawn and answered | same, plus `PAM.md` | one command, yours to run |
| R2 | the chosen session actually starts | same sitting, by eye | — |

## Before you sit down

**Re-install, or R6 produces nothing.** The deployed binary at
`/usr/local/lib/auroval/auroval-greet` predates `AUROVAL_LOG_KEYS`; `strings`
on it finds neither that nor `AUROVAL_EXIT_AFTER_FIRST_FRAME`. The harness
prefers `target/release/auroval-greet` and will say which one it used, so a
local build is enough for R6 — but greetd runs the deployed one, so R3 and R1
need the install either way.

```
nu deploy/install.nu
sudo sh target/auroval-install-root.sh
```

Then read `PAM.md` and run **§3 and §3b** before you start. Doing either
afterwards costs a second sitting, and the two together are what make R1 worth
testing at all: §3's `pam_exec` gives a message with no response, §3b's
`pam_oath` gives a second prompt. One login covers both branches of
`auth.rs`.

§3b installs `oath-toolkit` and writes a secret you generate yourself. It is
the only part of this sitting that adds a package to the machine, and `PAM.md`
removes it as prominently as it adds it.

## Order, and why

On the VT, type the one-word wrapper rather than the path — `deploy/install.nu`
writes one for each harness, because the clipboard does not survive a VT switch
and a long path is a path you will mistype on a console with no scrollback:
`auroval-keys`, `auroval-greetd`, `auroval-vt`.

1. **`keyboard-round.nu` first**, as `auroval-keys`. It needs no root and no greetd, it is capped
   at 45 s, and it is the one that tells you the VT plumbing works at all. If
   cage cannot take DRM master, you want to know that before greetd is
   involved.
2. **`greetd-round.nu` second**, as `auroval-greetd`. It is an observer: it cannot start greetd, so
   it prints the one root command and watches from another VT. Read its
   step 4 before leaving the console you are on — there is no scrollback on
   the next one.

Both refuse to run inside a compositor, and say how to reach a free console.

## What this sitting cannot settle

- **R1 with `pam_exec` alone.** It cannot ask a question: its `stdout` option
  relays output to the application as `PAM_TEXT_INFO`, and `expose_authtok`
  only reads the cached token back. With §3 alone the sitting exercises the
  `wants_response() == false` branch and nothing about message *count*, which
  is what a fixed two-field greeter actually fails on. §3b is the fix, and the
  fit criterion in `docs/prd/greet.md` should probably say "an extra prompt"
  rather than naming `pam_exec`, since the module it names cannot produce one.
- **R10**, which needs a second monitor. P4.
- **C15 in the greeter's own context.** `spike/p1e/RESULTS.md` already says
  why: nothing on the still path starts GStreamer, so cold and warm both
  measure the same. Unmeasurable until the P2 media path lands. Do not
  re-attempt it here.

## The mode-700 rehearsal is not part of this

It reads as console work and is not. R15's criterion is *"with `/home/<user>`
at mode 700, the configured background still renders"* — a render, not a
login, and `snapshot` goes through surfaceless EGL with no compositor, no
greetd and no DRM master. It needs root only to become `greeter`, so it groups
with the three commands at the end of `spike/p1e/RESULTS.md`. It is blocked on
something else first: there is no `/etc/auroval/` on this machine and so no
configured background to render.
