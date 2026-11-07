# Deploying the greeter

Everything here exists because of one fact: the greeter runs as `greeter`,
whose home is `/` and is not writable. A greeter that works beautifully from
your own account can be a second slower and have no memory at all once it is
actually deployed, and neither failure looks like a bug in the code.

**SDDM stays the way into this machine.** Nothing in this directory enables
`greetd`, nothing touches `/etc/greetd/config.toml`, and nothing stops,
disables or reconfigures SDDM. Switching display managers is the last box of
P1 and it is a deliberate, separate act.

## The files

| file | installed as | why |
|---|---|---|
| `auroval-greet.sh` | `/usr/local/lib/auroval/auroval-greet.sh` | greetd's command. Sets C15's cache variables, execs `cage`. `/bin/sh`, because it is on the startup path |
| `auroval.tmpfiles.conf` | `/etc/tmpfiles.d/auroval.conf` | creates `/var/lib/auroval` and `/var/cache/auroval`, owned by `greeter` |
| `greetd-auroval.toml` | `/etc/greetd/auroval.toml` | greetd's config for auroval — **not** `config.toml`, and never read by `greetd.service` |
| `install.nu` | — | builds, stages, writes the one root script, writes the `auroval-vt` wrapper. Runs no `sudo` |

The binary itself is installed as `/usr/local/lib/auroval/auroval-greet`, out
of any home directory: `greeter` cannot read `/home/dirk` when it is mode 700,
which it is.

## Installing

```
nu deploy/install.nu
```

It builds the greeter, stages what root has to copy, and prints one command.
It never runs `sudo` itself — a failed `sudo` trips `pam_faillock` and can
lock you out of this machine, so every privileged step is in a script you read
and run yourself:

```
sudo sh target/auroval-install-root.sh
```

That script installs four files and runs `systemd-tmpfiles --create`. It does
nothing else.

## Checking it worked

`auroval-greet probe` run as *you* proves almost nothing: you have a home, a
cache and a session, and the greeter has none of them. Run it as the greeter:

```
sudo nu spike/p1e/greeter-probe.nu
```

Before the tmpfiles snippet is installed this reports

```
state file   /var/lib/auroval/last-login
  writable     no — Permission denied (os error 13)
```

and after it, `writable yes`. That line is R4's memory working or not working,
and the difference between the two is entirely deployment.

## The two numbers behind all of this

**C15 — the writable cache is worth ~0.9 s of every login.** Measured as the
`greeter` user, process start to GStreamer ready:

| | |
|---|---|
| no writable cache (the default) | 1018 ms |
| writable cache, registry absent | 1011 ms |
| writable cache, registry warm | 93 ms |

That is why `auroval-greet.sh` exists at all rather than greetd calling the
binary directly, and why `/var/cache/auroval` is created by tmpfiles rather
than hoped for. The font half of the same problem — fontconfig reporting *"No
writable cache directories"* — is solved inside the greeter, which loads fonts
by path and never asks fontconfig.

**R16 — first frame within 500 ms, provisionally.** The budget came from one
P0 measurement of 243 ms, at 1255×674, through a *nested* `cage`, inside a
running Hyprland session. Every part of that is the wrong context. Re-measure
it properly:

```
auroval-vt
```

from a TTY login. See [`../spike/p1e/RESULTS.md`](../spike/p1e/RESULTS.md) for
what that harness does and what it has reported so far.

## Running greetd without becoming greetd

```
sudo nu spike/p1e/greetd-session.nu --vt 3 --seconds 60
```

greetd is a daemon, not a one-shot: when a session ends it starts a *fresh*
greeter and switches the console back to its own VT. Run as a service before
you mean it, it will keep pulling you off whatever you were doing — this
happened during P0. So that script starts greetd detached with a deadline,
refuses an occupied VT, and stops greetd itself when the time is up.
Ctrl+Alt+F1 gets you back to your desktop at any point regardless.
