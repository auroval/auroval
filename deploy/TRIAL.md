# Running auroval as the display manager, on trial

**SDDM stays installed throughout.** This swaps which unit boots the login
screen and nothing else, and it is one command to swap back. The PRD puts
*"SDDM comes off the machine"* at the **end** of P1; this is the careful step
before that, not that step.

Read the whole file before running any of it. Every command here is a root
command and this project never runs one for you.

---

## 1. What changes, exactly

Three things, and none of them touches SDDM's own configuration:

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
- **Anything that makes you reach for the power button.** That is the finding,
  whatever it turns out to be.

The look is deliberately unfinished: there is no `/etc/auroval/greet.lua` on
this machine, so the greeter runs on `Palette::default()` — hyprlock's own
colours on a solid background, with no image behind it. The background is #34
and the theme is #5. Using it plainly for a week is a good way to find out
what the theme actually needs.
