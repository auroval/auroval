# Running auroval as the display manager, on trial

**SDDM stays installed throughout.** This swaps which unit boots the login
screen and nothing else, and it is one command to swap back. The PRD puts
*"SDDM comes off the machine"* at the **end** of P1; this is the careful step
before that, not that step.

Read the whole file before running any of it. Every command here is a root
command and this project never runs one for you.

---

## 1. What changes, exactly

Three things the install and systemd do, and none of them touches SDDM's own
configuration:

- `greetd.service` is enabled and `sddm.service` disabled. Both carry
  `Alias=display-manager.service`, so exactly one of them may be enabled at a
  time — that alias is also why the order below matters.
- A systemd drop-in points greetd at **auroval's** config instead of its own.
  `/etc/greetd/config.toml` is greetd's file, still the stock one, and stays
  untouched. That is deliberate: the day you want plain `agreety` back, it is
  there and correct.
- The greeter runs on **VT 1**, which is what `greetd.service` already expects
  — it carries `Conflicts=getty@tty1.service`. That is now `install.nu`'s
  default, and it was not on 2026-11-09: an install without `--vt 1` wrote
  `vt = 3` over a file that said 1, while the greetd started before it went on
  serving tty1. Nothing compared them, and the only place it would have shown
  is the next reboot. `install.nu` now says so when the two differ.

And two more that **you** put in place by hand, because this project will not
write either of them for you — see below.

### The keyring, and why this edit is yours to make

`/etc/pam.d/sddm` hands the password you just typed to gnome-keyring.
`/etc/pam.d/greetd` is six lines and carries nothing of the sort, so after the
switch your login keyring stays locked and the first application that wants a
stored secret raises a dialog asking for the same password again.

Two lines fix it. Add them to `/etc/pam.d/greetd`, in these positions — the
order is the one `/etc/pam.d/sddm` already proves on an Arch machine:

```
#%PAM-1.0

auth       required     pam_securetty.so
auth       requisite    pam_nologin.so
auth       include      system-local-login
-auth      optional     pam_gnome_keyring.so

account    include      system-local-login

session    include      system-local-login
-session   optional     pam_gnome_keyring.so auto_start
```

The leading `-` means a missing module is not an error, and `optional` means a
failing one does not fail the login: if gnome-keyring is not installed, or
breaks, you still get in. The `auth` line captures the password; the `session`
line passes it to the daemon. Keep `auth include system-local-login` where it
is — that is the line that checks your password at all.

**`install.nu` does not do this, on purpose.** `/etc/pam.d/greetd` is greetd's
file, shipped by greetd's package, in the same way `/etc/greetd/config.toml`
is — and §2's way out is two commands and no graphical session. An edit this
project wrote into a PAM stack would survive that rollback and sit on a
machine that is back on SDDM. It is also a per-machine authentication
decision: the unlock works only while your keyring password *is* your login
password, which nothing here can check. See issue #111.

**What this does not fix.** If your keyring password differs from your login
password — because you set one explicitly, or carried the keyring over from
another machine — PAM hands over the wrong secret and you get the dialog
anyway. That is correct behaviour and there is no edit to this file that
changes it; the way out is to change the keyring's password to match, in
Seahorse or whatever you use, or to live with the prompt.

To check it afterwards, from inside a session you logged into normally:

```
busctl --user call org.freedesktop.secrets /org/freedesktop/secrets/collection/login \
    org.freedesktop.DBus.Properties Get ss org.freedesktop.Secret.Collection Locked
```

`v b false` means unlocked. If you had to answer a dialog to get there it
proves nothing — see §5.

### The theme, which is the only thing that makes it look like anything

`/etc/auroval/greet.lua` is what the greeter and the lock screen both read, and
without it they fall back on a solid colour and hyprlock's borrowed accent.
`deploy/greet.lua` in this repository is the one this machine should run: one
scene from `/usr/local/share/auroval/`, its R9 fallback chain, and the palette
that scene derives to.

```sh
install -Dm644 /home/dirk/Documents/projects/auroval/deploy/greet.lua \
    /etc/auroval/greet.lua
```

No reload is needed and no service restarts: greetd execs a fresh greeter when
a session ends, so the next **log out** picks the file up. A restart of
`greetd` would not be the same measurement — on this machine the desktop
session is on tty1 and was opened by that same greetd, so restarting it puts a
greeter on a VT that already has a compositor.

`install.nu` does not write this file either, and for the same reason as the
PAM lines above: which scenes a machine shows is a per-machine choice, and §4's
rollback should not leave this project's configuration behind on a machine that
is back on SDDM. If the file is missing or unparseable the greeter says so in
the journal and draws on defaults — R13 — so a mistake here costs the look and
never the login.

## 2. The way out, before the way in

**Learn this before you reboot.** It is short and it is the whole reason this
is a trial rather than a switch.

The greeter runs under `cage -s`, so **Ctrl+Alt+F2** works from inside it, and
systemd starts a getty on any VT you switch to. From that getty, logged in as
yourself:

```
sudo systemctl disable --now greetd.service
sudo systemctl enable --now sddm.service
```

That is the rollback. It is two commands, it needs no graphical session, and
it works from any console.

If the greeter crashes instead of drawing, greetd restarts it — five times in
thirty seconds and then it gives up, leaving you at a console. You are not
locked out either way. The one thing that would lock you out is a greeter that
draws but will not accept input, and **that** is what `-s` is for: switch away
and roll back.

## 3. Switching over

```sh
# The greeter itself, on VT 1 — now the default, so no flag is needed.
#   nu deploy/install.nu
sh /home/dirk/Documents/projects/auroval/target/auroval-install-root.sh

# Point greetd at auroval's config. greetd's own config.toml is not touched.
mkdir -p /etc/systemd/system/greetd.service.d
cat > /etc/systemd/system/greetd.service.d/auroval.conf <<'EOF'
# auroval, on trial. Remove this file and `systemctl daemon-reload` to put
# greetd back on its own /etc/greetd/config.toml.
[Service]
ExecStart=
ExecStart=/usr/bin/greetd --config /etc/greetd/auroval.toml
EOF
systemctl daemon-reload

# One display manager at a time: off before on.
systemctl disable sddm.service
systemctl enable greetd.service
```

Then check what you are about to boot into, **before** you boot into it:

```sh
systemctl is-enabled greetd sddm          # enabled / disabled
readlink -f /etc/systemd/system/display-manager.service   # .../greetd.service
systemctl cat greetd.service | grep ExecStart             # the auroval config
```

Reboot. The login screen on VT 1 is auroval's.

## 4. Going back

```sh
systemctl disable --now greetd.service
systemctl enable --now sddm.service
rm -f /etc/systemd/system/greetd.service.d/auroval.conf
rmdir --ignore-fail-on-non-empty /etc/systemd/system/greetd.service.d
systemctl daemon-reload
```

Nothing auroval installed is removed by this — the greeter, its config and the
tmpfiles entry stay where they are and do nothing while greetd is disabled.

The one installed file that keeps doing something is the pacman hook, which
re-warms the greeter's plugin registry on every gstreamer update (issue #77).
It costs about a second of an update you were already waiting for, and it
exits silently if the greeter is gone, so there is no need to remove it. If
you want it off anyway:

```sh
rm -f /etc/pacman.d/hooks/auroval-warm-registry.hook
```

## 5. What to watch for during the trial

This is the point of a trial rather than a switch, so the things to notice are
the ones no harness has seen:

- **A wrong password.** It should say so. That clause of R3 was broken until
  2026-11-08 and the fix has unit tests but has not been seen on a console —
  the first typo you make is its first real test.
- **The remembered user and session** (R4) across reboots, and whether the
  session it preselects is the one you want.
- **How long it takes to draw** on a cold boot rather than a warm start. R16
  is 8 ms warm; a cold boot is a different measurement and nobody has taken it.
- **A password dialog you did not expect**, in the first minutes after login.
  That is the keyring (§1) and it means the PAM lines are missing, in the wrong
  place, or defeated by a keyring password that is not your login password.
  Measured, not remembered: a `SystemPrompter` unit in
  `systemctl --user list-units --all` says a dialog was raised at some point
  this boot, which is the half of the check you cannot see once you have
  clicked the thing away.
- **Anything that makes you reach for the power button.** That is the finding,
  whatever it turns out to be.

The look is no longer bare, and was never quite as bare as an earlier version
of this paragraph said. `/etc/auroval/greet.lua` has been on this machine since
2026-11-08 and has always configured a background; what it lacked until
2026-11-22 was a `palette`, so the greeter drew nightshade's scene under
hyprlock's Catppuccin accent. `deploy/greet.lua` is the file as it should be —
one scene, and the palette that scene derives to — and §1 says how to install
it.

One thing is deliberately held back there: the pool is **one** candidate, not
the two that are installed. scene-01 and scene-05 extract to hues 175° apart
(cyan against red-orange) while `palette` is a single table, so a two-deep pool
would show one scene's picture under the other's accent on half its boots. The
second candidate is in the file, commented, with the numbers. That question is
Q11's, the background is #34, and the theme is #5. Using it plainly for a week
is still the best way to find out what the theme actually needs.
