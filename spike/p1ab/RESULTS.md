# P1/A+B results — the console sitting

Receipts for #36. Run on `games-werkkamer`, kernel 7.3.1-1-cachyos, nvidia,
2026-11-08. SDDM remained the way in throughout; greetd was started by hand
with a deadline and stopped itself.

## Status

| requirement | state |
|---|---|
| R6 — key repeat and caps lock | **four of five checks pass.** The fifth fails for a real reason, below |
| R3 — a wrong password is not fatal | **holds, measured** |
| R1 — an extra PAM step needs no code change | **holds, both branches** |
| R2 — the greeter starts what it names | **holds for the exec half** |
| the greeter's own `LC_TIME` | **not measured.** Missed in the sitting; needs greetd running and one root command |

## R3 — a wrong password returns to the prompt

This is the one the sitting existed for, and `auth.rs` had neither a test nor
a real run behind it until now.

```
10:37:41  greetd starts, greeter session for `greeter` opens
10:37:57  pam_exec fires        attempt 1, user=dirk
10:38:27  pam_unix(greetd:auth): authentication failure ... user=dirk
10:38:29  pam_exec fires        attempt 2, user=dirk
10:39:27  pam_unix(greetd:session): session opened for user dirk
10:39:29  systemd: Reached target Hyprland session
```

The greeter's pid was **844301 throughout**. The harness sampled the process
set once a second and recorded two changes in 173 s: the greeter appearing at
t=0, and the greeter gone at t=102, which is the successful login. Nothing
between. So the authentication failure at 10:38:27 did not restart the
greeter, and the *same process* accepted a second attempt a minute later.

That is stronger than the criterion asks. "The process does not exit" would be
satisfied by a greeter that survived in an unusable state; a second
authentication completing through the same pid shows the prompt came back and
worked.

Receipt: `~/auroval-p1ab-greetd.txt` and `/var/log/auroval-greetd.log`
(`error: authentication error: pam_authenticate: AUTH_ERR`, exactly one).

## R1 — both branches, and which module proved which

The PAM stack carried two extra steps (`PAM.md` §3 and §3b):

- `pam_exec` → `PAM_TEXT_INFO` → greetd `info` → `auth.rs`'s
  `wants_response() == false` branch: a message drawn and answered with
  `response: None`.
- `pam_oath` → `PAM_PROMPT_ECHO_OFF` → greetd `secret` → a **second prompt**
  in one conversation.

**The message branch ran twice**, once per attempt, logged by the helper at
10:37:57 and 10:38:29.

**The prompt branch ran and was answered correctly.** The evidence is
negative and it is the good kind: the single failure in the journal is
`pam_unix(greetd:auth)`, not `pam_oath`, and `pam_oath` sits *before*
`system-local-login` in the stack. Reaching `pam_unix` at all means `pam_oath`
returned success, which means it asked for a code and got a valid one —
through a greeter that was never written for it. `pam_oath` logged nothing in
the whole window.

So R1 holds in the sense its fit criterion means: the conversation asked for
things in a count and order the greeter does not assume, and the greeter drew
and answered them. **The criterion's wording still needs fixing** — it names
`pam_exec`, and `pam_exec` cannot produce a prompt (`PAM.md` §6).

## R2 — the greeter starts what it names

`Reached target Hyprland session` at 10:39:29, and `uwsm_hyprland.desktop`
started at 10:39:46. The argv the greeter carried into
`Request::StartSession` reached an actual session. The desk half — that
`Session.command` is carried verbatim from `split_exec` through to
`auth.rs` — was already readable in the code; this is the half that needed a
console.

## R6 — four of five

Second sitting, 10:23, receipt `keyboard-run-2026-11-08-1024.txt`.

| check | result |
|---|---|
| repeat announced | `Repeat { rate: 25, delay: 600 }` |
| repeat events | 49 events, median interval 50.0 ms |
| rate agrees | **FAIL** — 40.0 ms asked, 50.0 ms measured, 25% off |
| caps lock both ways | on at 10.554, off at 13.956 |
| field count rose, fell | rose to 6, emptied by a held backspace |

The failure is the greeter's and the mechanism is in the loop. SCTK
reschedules repeat with `TimeoutAction::ToDuration(gap)`, which counts the gap
from when its callback **returned**, and `shell::run` calls `draw_all()`
unconditionally on every turn. So the period is 40 ms plus a full 2560×1440
redraw, which is the 10 ms that turns 25/s into 20/s.

The first repeat is the evidence for that reading rather than a quantised
timer: pressed at 5.164, repeated at 5.766 — 602 ms against the announced 600.
The delay is scheduled before any drawing happens; only the repeats after it
carry the cost.

**Underneath it is a defect worth more than the symptom.** This login screen
redraws itself about sixty times a second while nothing moves. The fix is to
draw when something changed, which also fixes the repeat rate.

## What the sitting cost, and what it taught

Four things went wrong that had nothing to do with the requirements, and all
four were in the harnesses rather than the greeter.

1. **The first R6 run failed three of five checks and the greeter was right
   every time.** `log_key` printed `ui.typed`, but at the username stage
   `push` fills `username`. And local key repeat never reached `log_key` at
   all: SCTK 0.20 delivers it through the closure given to
   `get_keyboard_with_repeat`, not through `KeyboardHandler::repeat_key`,
   which only fires for a server-sent `KeyState::Repeated`.
2. **`greetd-session.nu` died on `open $config`.** Nushell parses a `.toml` by
   extension, so the regex got a record. It had never reached that line
   before.
3. **The operator's briefing scrolled off the top of the console**, which is
   the one place where that cannot be recovered from. A briefing that only
   works on a tall enough terminal is not a briefing.
4. **tty3 still had a login session** from the R6 round an hour earlier, so
   greetd could not take it. The harness should check the VT it is about to
   tell someone to use.

The pattern is one thing: *instrumentation is code, and it is the code nobody
runs before they depend on it.* Every one of these cost a VT round trip, and a
VT round trip costs more than the same mistake anywhere else, because the
person reading the error has no scrollback and nothing to compare against.

## Not measured

**The greeter's own `LC_TIME`.** `/proc/<pid>/environ` is 0400 and owned by
`greeter`, so the harness could not read it, and greetd had stopped by the
time anyone noticed. `/etc/locale.conf` sets `LC_TIME=nl_NL.UTF-8` and
systemd's manager environment carries it, so the greeter probably sees it —
but "probably" is what `clock.rs` was written to stop happening. Next time
greetd runs, as root:

```
tr '\0' '\n' < /proc/<greeter-pid>/environ | grep -E '^LANG|^LC_'
```

`sudo -u greeter env` is not this answer: greetd clears the environment and
passes only the PAM env list plus its own XDG variables.

**What was on screen.** Whether the `info` line was visible for its four
seconds or flashed at the end, whether the OATH prompt replaced the first
prompt's text rather than being appended to it, and what the error message
said. The operator ran this sitting without the briefing (see 3 above), so
these were not observed. They need one more login, not a whole sitting.
