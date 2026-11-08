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

Then, if you want R1 in this sitting rather than the next one, read `PAM.md`
and run its §3 before you start. Doing it afterwards costs a second sitting.

## Order, and why

1. **`keyboard-round.nu` first.** It needs no root and no greetd, it is capped
   at 45 s, and it is the one that tells you the VT plumbing works at all. If
   cage cannot take DRM master, you want to know that before greetd is
   involved.
2. **`greetd-round.nu` second.** It is an observer: it cannot start greetd, so
   it prints the one root command and watches from another VT. Read its
   step 4 before leaving the console you are on — there is no scrollback on
   the next one.

Both refuse to run inside a compositor, and say how to reach a free console.

## What this sitting cannot settle

- **R1 as its fit criterion words it.** `pam_exec` cannot ask a question: its
  `stdout` option relays output to the application as `PAM_TEXT_INFO`, and
  `expose_authtok` only reads the cached token back. What the sitting
  exercises is the `wants_response() == false` branch — a message drawn and
  answered with `response: None`, which a fixed two-field greeter still has
  nowhere to put. A real extra *prompt* needs `pam_oath` or similar. `PAM.md`
  §6 has the detail.
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
