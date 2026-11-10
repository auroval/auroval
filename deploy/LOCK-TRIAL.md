# Putting auroval-lock on the machine, and getting back out

This exists **before** anything calls the locker, which is P3e's own order:
an install step and a written rollback first, a deliberate keybind second,
the idle timeout last. A lock you did not ask for is the one you are least
ready for, and a recovery you work out while locked out is not a recovery.

Nothing in this document is theoretical. Every command in section 4 has been
run, and section 3 says which of them have not been run *while actually
locked out* — because that is a different thing and it is honest to say so.

## 1. What the install does, and what it deliberately does not

`deploy/install.nu` builds `auroval-lock`, stages it, digests it, and the
root script copies it to `/usr/local/lib/auroval/auroval-lock`.

**That is all it does, and the list of what it does not do is the point:**

| | |
|---|---|
| `~/.config/hypr/hypridle.conf` | untouched. The idle timeout and lock-before-suspend still call `hyprlock` |
| `SUPER + L` | untouched. Still `hyprlock` |
| `hyprlock` | still installed, still the locker of record |
| a keybind for `auroval-lock` | **none is written.** It is used by choice or not at all |
| `/etc/pam.d/auroval-lock` | not written here. `pc-setup`'s playbook writes it |

**And nothing calls it.** A bare `auroval-lock` prints its usage and exits 2;
there is no invocation without an argument that locks anything. Since
2026-11-10 there *is* a production mode — `auroval-lock lock` — and it is a
named mode for exactly that reason. It also refuses to start while
`AUROVAL_LOCK_SECONDS`, `AUROVAL_LOCK_BREAK_GL` or `AUROVAL_FRAME_DUMP` are
set, because those are test hooks and the environment here is a file you
write yourself.

So the binary can now be pointed at. **Nothing points at it**: that is the
next step of P3e and it is a line in your own config, not something the
install does.

So installing it is reversible by deleting one file, and until a later change
it cannot lock anything you did not deliberately ask it to.

## 2. Looking at it without risking anything

`probe` binds `ext_session_lock_manager_v1`, reports the version, counts the
outputs, and exits **without ever calling `lock`**. It is safe on the desktop
you are sitting in:

```nu
auroval-lock-probe
```

There is deliberately no one-word wrapper for `rehearse`. Somebody typing a
short name they half-remember is exactly the person who would be caught by
it.

## 3. The one thing that can go wrong, and why no supervisor can fix it

`ext-session-lock-v1` **fails closed on purpose**. If the locking process
dies while the session is locked, the session stays locked — sctk's `Drop`
sends `destroy` rather than `unlock_and_destroy`, with a comment reading
*"choosing not to unlock here results in us failing secure."* `auroval-lock`
installs no signal handler, so `SIGTERM`, `SIGINT` and `SIGKILL` all leave
the screen held. That is R14, and it is a feature.

**A second instance cannot rescue the first.** This was measured rather than
assumed on 2026-11-09: a second `auroval-lock` started against a compositor
that is still holding a dead client's lock is sent `finished` immediately and
locks nothing. So there is no supervisor to write, and this document is the
recovery story instead.

**Hyprland already has the recovery, and it is good.** Killing a locker that
holds a lock does not leave a black screen: Hyprland draws *"it looks like
you locked your screen but the lockscreen app died"* and names the way out
itself. From another VT — `Ctrl+Alt+F3`, log in as yourself:

```nu
hyprctl --instance 0 eval 'hl.clear_crashed_lockscreen()'
```

That has been run and it works, and it is written plainly on purpose. This
line used to carry escaped parentheses — `\(\)` — on the reasoning that
`(...)` inside a nushell string is a subexpression. **That is true of an
interpolated string and false of this one.** Single quotes in nushell are
literal, so the backslashes were passed through to Lua, which answered

```
error: 3 [string "hl.clear_crashed_lockscreen\(\)"]:1: syntax error near '\'
```

Found by the maintainer on 2026-11-10, by running the recovery command in the
one situation where it has to work on the first try. Which is the argument for
why it lives in this file at all, and the argument for why a correct-looking
command in a document is not the same thing as a command somebody has run.

`spike/p3/failure-round.nu` prints the same line and prints it correctly: there
it *is* inside `$"..."`, where the escape is required, and the receipt in
`failure-round-2026-11-10.txt` shows `()` coming out the other end.

**What has not been exercised:** the sequence above while genuinely locked out
of this machine's own session, with the real PAM stack. It has been done in a
nested compositor. Until it has been done for real, keep a second VT logged
in before using the locker in earnest — which is also what P3b and L11 ask
for.

## 4. Going back

Delete the binary. Nothing references it, so nothing breaks:

```nu
sudo rm -f /usr/local/lib/auroval/auroval-lock
```

And the probe wrapper, which is yours rather than root's:

```nu
rm -f ~/.local/bin/auroval-lock-probe
```

If a keybind was added by hand in a later step, remove that line from
`~/.config/hypr/` and reload:

```nu
hyprctl reload
```

Nothing under `/etc` was written for the locker by `install.nu`, so there is
nothing to undo there. `/etc/pam.d/auroval-lock` comes from `pc-setup`; it
does nothing while no locker is installed, and removing it would stop
`auroval-lock` authenticating at all — which is the safe direction, not a
dangerous one.

## 5. What is still missing before this is a locker you rely on

- ~~A production mode.~~ **Done 2026-11-10**: `auroval-lock lock`.
- ~~**L7**, context loss, and the attended run that shows these screens
  actually unlock.~~ **Both done 2026-11-10.** L7 is tested six-of-six in the
  failure round, and two attended rehearsals unlocked — the second one blind,
  with the GL context destroyed.
- **P3d**, the 1440p rehearsal for L14.
- `hyprlock` comes off only after `greet.md`'s own metric is met: *used
  daily for two weeks and no lockout incident*.
