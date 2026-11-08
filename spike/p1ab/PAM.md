# P1/A — putting an extra step in greetd's PAM stack

What the operator installs for the R1 sitting, and exactly how to take it out
again. Read the whole thing before running anything in it. Nothing here is run
by a script in this repository: every command below is a root command and this
project never runs root commands on your behalf.

Companion files: `spike/p1ab/pam-extra-prompt.sh` (the helper PAM runs) and
`spike/p1ab/greetd-round.nu` (the harness that watches the sitting).

---

## 1. What is being added, and why

**What.** One `auth` line in `/etc/pam.d/greetd`, and one script copied to
`/usr/local/lib/auroval/pam-extra-prompt.sh`. That is the whole change.

**Why.** R1 is the requirement that the greeter drives greetd's conversation
and assumes nothing about it, and its fit criterion in `docs/prd/greet.md` is:

> A PAM stack carrying an extra `pam_exec` prompt completes login with no code
> change. A fixed two-field implementation fails this.

`crates/greet/src/auth.rs` is built for that — an `auth_message` that is a
statement rather than a question is rendered and answered with
`PostAuthMessageResponse { response: None }` — and it is **the only module in
this project with neither a test nor a real run behind it**. The extra line
below is what makes a real PAM stack send that message, so the branch runs for
once against something other than an assertion.

**Be honest about what it produces.** It produces an extra **`info` message**,
not an extra **prompt**. `pam_exec` cannot ask a question; §6 explains why, and
what would be needed for a real one. The `info` is still the half of R1 that
discriminates: a message arrives where a fixed two-field greeter has nowhere to
put it, and it has to be drawn and answered with no response.

**Why this is a safe thing to agree to — the one paragraph that matters.**
`/etc/pam.d/greetd` is **greetd's own PAM service file and nothing else reads
it**. SDDM authenticates through `/etc/pam.d/sddm`, `/etc/pam.d/sddm-greeter`
and `/etc/pam.d/sddm-autologin`; `login(1)` on a console uses
`/etc/pam.d/login`; `sudo` uses `/etc/pam.d/sudo`. None of them includes
`/etc/pam.d/greetd`. So **a broken `/etc/pam.d/greetd` cannot lock you out of
SDDM, out of a TTY login, or out of `sudo`** — the worst it can do is break the
thing that is not yet how you log in. greetd is `disabled` on this machine and
SDDM is `enabled`; this change does not alter either, and must not.

The line is `optional`, which means even a helper that fails, hangs briefly or
is missing entirely cannot refuse a login through greetd.

---

## 2. The exact line

```
auth       optional     pam_exec.so type=auth stdout quiet /usr/local/lib/auroval/pam-extra-prompt.sh
```

Each piece, because an unexplained PAM line is an unauditable one:

| token | why |
| --- | --- |
| `auth` | the stage the conversation happens in; `account` and `session` have no greeter listening |
| `optional` | its result is ignored. Nothing it does can refuse a login. This is the safety property |
| `type=auth` | pam_exec runs the command only for this module type — a stray copy of the line elsewhere does nothing |
| `stdout` | **the load-bearing flag.** Without it the helper's output goes to `/dev/null` and the greeter sees nothing at all. With it, the output is relayed to the calling application, which is greetd, which forwards it to the greeter |
| `quiet` | suppresses pam_exec's own "command failed with status N" message, so the only thing the greeter draws is the helper's text |

It goes **after `pam_nologin.so` and before `auth include
system-local-login`**, on purpose: the `info` then arrives *before* the
password prompt, which is the hardest ordering for a fixed implementation and
the clearest one to watch. The resulting file is in §3.

---

## 3. The exact command to add it

Three commands, as root. Run them in a root shell (see §5), not from the VT you
are about to put greetd on.

```sh
# 1. Install the helper where only root can write it, since root executes it.
install -d -m 0755 /usr/local/lib/auroval
install -m 0755 -o root -g root \
    /home/dirk/Documents/projects/auroval/spike/p1ab/pam-extra-prompt.sh \
    /usr/local/lib/auroval/pam-extra-prompt.sh

# 2. Back up the PAM file before touching it. This backup is the undo in §4.
cp -a /etc/pam.d/greetd /etc/pam.d/greetd.auroval-backup

# 3. Write the file with the extra line in it. Spelled out in full rather than
#    patched in place: a sed expression that half-matches leaves a PAM stack
#    nobody can read, and this file is six lines long. It is the original file
#    with exactly one line added — in particular `auth include
#    system-local-login`, the line that actually checks your password, is
#    still there. A copy of this block that is missing it authenticates nobody.
cat > /etc/pam.d/greetd <<'EOF'
#%PAM-1.0

auth       required     pam_securetty.so
auth       requisite    pam_nologin.so
auth       optional     pam_exec.so type=auth stdout quiet /usr/local/lib/auroval/pam-extra-prompt.sh
auth       include      system-local-login
account    include      system-local-login
session    include      system-local-login
EOF
```

Then check it reads the way you expect, before you rely on it:

```sh
diff /etc/pam.d/greetd.auroval-backup /etc/pam.d/greetd
test -x /usr/local/lib/auroval/pam-extra-prompt.sh && echo helper ok
```

The `diff` must show exactly one added line and nothing else.

---

## 3b. The variant that actually tests R1: `pam_oath`

§6 explains why `pam_exec` produces a message and not a question. This section
is the other half, and **running both together is what makes the sitting worth
sitting**: `pam_exec` exercises `auth.rs`'s `wants_response() == false` branch
and `pam_oath` exercises an extra *prompt*, so the one login covers both.

What R1 actually discriminates on is the **count**: a greeter with a fixed
username field and a fixed password field has nowhere to put a second
question. `pam_oath` asks one, with echo off, which greetd reports as a
`secret` message — the same shape as the password prompt and therefore not a
new drawing problem, which is the point. The greeter should ask twice and
assume nothing.

**Nothing in this section has been run on this machine.** `oath-toolkit` is
not installed. It is in `cachyos-extra-v4` as 2.6.15-1.1 and ships
`/usr/lib/security/pam_oath.so` and `/usr/bin/oathtool`; both of those were
checked with `pacman -Si` and `pacman -Fl`, and nothing else here was. Treat
the commands as read-then-run, not as a transcript of something that worked.

Its licence is GPL-3.0-or-later. That is not the licence problem this project
avoided with `greetd_ipc`: nothing links against it. PAM loads the module in
greetd's process and `oathtool` is a separate program you run by hand.

### The secret is yours, and this file will not generate it

Generate it in your own root shell. It must not pass through a chat
transcript, a scrollback or a repository, and that is the only reason this is
four commands instead of one.

```sh
# 1. The package.
pacman -S oath-toolkit

# 2. A 20-byte secret, as hex. Keep this output visible; step 4 needs it.
head -c 20 /dev/urandom | xxd -p -c 40

# 3. The usersfile. Replace SECRET with what step 2 printed. The `-` is the
#    PIN field, and means no PIN. HOTP/T30/6 is TOTP, 30-second steps, six
#    digits — the thing any authenticator app would show you.
#    pam_oath records the last-used step in this file, so it must stay
#    writable by root; 0600 root:root is what it wants and what it checks.
install -m 0600 -o root -g root /dev/null /etc/users.oath
printf 'HOTP/T30/6 dirk - SECRET\n' >> /etc/users.oath

# 4. Check it before PAM depends on it: this must print a six-digit number,
#    and it is the number you will type on the VT.
oathtool --totp SECRET
```

### The combined stack

This is §3's file with one more line. Everything §3 says about writing it out
in full rather than patching it applies here too, and `auth include
system-local-login` — the line that checks your password — is still in it.

```sh
cp -a /etc/pam.d/greetd /etc/pam.d/greetd.auroval-backup

cat > /etc/pam.d/greetd <<'EOF'
#%PAM-1.0

auth       required     pam_securetty.so
auth       requisite    pam_nologin.so
auth       optional     pam_exec.so type=auth stdout quiet /usr/local/lib/auroval/pam-extra-prompt.sh
auth       required     pam_oath.so usersfile=/etc/users.oath window=30 digits=6
auth       include      system-local-login
account    include      system-local-login
session    include      system-local-login
EOF

diff /etc/pam.d/greetd.auroval-backup /etc/pam.d/greetd
```

`required`, not `optional`: an `optional` one-time password that nobody checks
would let the sitting pass while proving nothing. The cost of `required` is
that a wrong or missing code means greetd will not log you in — **and that is
survivable precisely because of §1: SDDM, console login and sudo do not read
this file.** You cannot lock yourself out of the machine with it. You can lock
yourself out of greetd, which is the thing you are testing.

### On the VT

Have a second console with `oathtool --totp SECRET` ready, because the code
changes every thirty seconds and you cannot run it on the console greetd owns.
The order you will be asked in follows the stack: the `pam_exec` message, then
the OATH code, then your password.

### Removing it

§4 removes `pam_exec`. For this variant, also:

```sh
rm -f /etc/users.oath
pacman -Rs oath-toolkit        # only if you installed it for this
grep -cE 'pam_exec|pam_oath' /etc/pam.d/greetd   # must print 0
```

---

## 4. The exact command to remove it

**Do this at the end of the sitting.** The helper holds every greetd login for
four seconds (see `HOLD` in the script), writes to a log, and exists only to
prove one requirement. It is not something to leave installed.

```sh
# Restore the file as it was, from the backup §3 made.
install -m 0644 -o root -g root /etc/pam.d/greetd.auroval-backup /etc/pam.d/greetd
rm -f /etc/pam.d/greetd.auroval-backup

# The helper and its log.
rm -f /usr/local/lib/auroval/pam-extra-prompt.sh
rm -f /var/log/auroval-pam-extra.log
```

If the backup is gone, this is the original file verbatim, as shipped by Arch's
`greetd` package on this machine on 2026-11-08:

```sh
cat > /etc/pam.d/greetd <<'EOF'
#%PAM-1.0

auth       required     pam_securetty.so
auth       requisite    pam_nologin.so
auth       include      system-local-login
account    include      system-local-login
session    include      system-local-login
EOF
```

Verify afterwards:

```sh
grep -c pam_exec /etc/pam.d/greetd   # must print 0
```

`/usr/local/lib/auroval/auroval-greet`, `auroval-greet.sh`,
`/etc/greetd/auroval.toml`, `/etc/tmpfiles.d/auroval.conf` and the
`/var/lib/auroval` and `/var/cache/auroval` directories belong to
`deploy/install.nu` and are **not** part of this undo. Leave them.

---

## 5. Keep a root shell open on another VT

**Before you edit `/etc/pam.d/greetd`, open a root shell on a VT you are not
using, and leave it open until §4 is done.** For example on VT 2: `Ctrl+Alt+F2`,
log in as yourself, `sudo -i`, and then do not touch that VT again.

Why, when §1 says this cannot lock you out: because "cannot" is an argument and
a live root shell is a fact. The argument is sound — SDDM, `login` and `sudo`
read their own PAM files and none of them includes greetd's — but PAM failures
are the one class of mistake where being wrong means being unable to fix being
wrong. A shell that is already authenticated does not care what
`/etc/pam.d/anything` says.

Three further things worth knowing while the line is in place:

* **greetd may run this helper twice per sitting, with different users.** Some
  greetd builds authenticate the greeter session itself through the same
  `greetd` PAM service before handing the conversation to you. If
  `/var/log/auroval-pam-extra.log` shows a `user=greeter` line as well as a
  `user=dirk` line, that is this, and it is harmless. The harness reports the
  users it sees rather than assuming one.
* **`pam_securetty.so` is already the first line of this file** and it refuses
  root logins from a TTY not listed in `/etc/securetty`. It is pre-existing, not
  something this change introduces, but if a greetd login fails in a way that
  has nothing to do with the password, suspect it before suspecting the new line.
* **Do not put this line in `/etc/pam.d/system-auth`, `system-login`,
  `system-local-login`, `login` or `sddm`.** Those are shared, SDDM and `sudo`
  do read them, and a four-second hold or a failing helper there is exactly the
  lockout this document keeps claiming is impossible.

---

## 6. Honest note: `pam_exec` cannot produce a prompt

*(§3b is the variant that can. Read this section first — it is why §3b exists.)*

The requirement says "an extra `pam_exec` prompt". `pam_exec` cannot produce
one, and a plausible-looking recipe that silently produces nothing is worse
than knowing this.

**What `pam_exec` does.** From `pam_exec(8)` on this machine:

> `stdout` — Per default the output of the executed command is written to
> `/dev/null`. With this option, the stdout output of the executed command is
> redirected to the calling application.

In Linux-PAM's implementation that redirection is a call to `pam_info(3)`,
i.e. a conversation message of type **`PAM_TEXT_INFO`**, which greetd maps to
`auth_message_type: "info"`. `pam_exec` never calls the conversation function
to *read* a response, so it can never emit `PAM_PROMPT_ECHO_ON` or
`PAM_PROMPT_ECHO_OFF`. There is no option that changes this.

**Why `expose_authtok` is not the loophole.** It looks like it should be: it
makes the module obtain the authentication token, and obtaining a token can
mean asking for one. But it obtains it with `pam_get_authtok(3)`, whose manual
page says it "returns the cached authentication token, or prompts the user if
no token is currently cached". Placed *after* `system-local-login` the token is
already cached, so nothing is asked. Placed *before* it, `pam_exec` does the
asking with the default `Password: ` prompt and caches the result, and Arch's
`pam_unix.so try_first_pass` then reuses it — so there is still exactly one
password prompt, just asked by a different module. Either way: no extra
question, and it would pass your password to the helper on stdin, which is a
reason not to do it at all.

**What this means for the sitting.** R1's branch in `auth.rs` is
`AuthMessageType::wants_response() == false` → render `Ui::Message` → reply
`PostAuthMessageResponse { response: None }`. That is precisely the branch an
`info` exercises, it is the branch that has never run, and it is the branch a
fixed two-field greeter does not have. The sitting is worth doing. What it does
*not* prove is that an extra *secret* or *visible* prompt renders and accepts
input — `auth.rs` shares one code path for those with the password prompt, which
does get exercised, so the untested gap is narrower than it looks, but it is not
zero.

**What a real extra question needs.** A module that calls the conversation with
`PAM_PROMPT_ECHO_ON`/`OFF` for something other than the password. Nothing in
base `pam` does that arbitrarily — the full module list on this machine is in
`/usr/lib/security/` and `pam_echo.so`, the obvious candidate, also only emits
`PAM_TEXT_INFO`. The realistic options, in order of effort:

1. **`pam_oath.so`** from `oath-toolkit` (in `extra`, not installed here). It
   prompts `One-time password (OATH) : ` with echo off, which greetd forwards
   as `auth_message_type: "secret"` — a genuine second question, and also
   exactly the real-world scenario the PRD's "a deployment adds a second
   authentication factor" note is about. As `auth optional pam_oath.so ...` it
   still asks, and still cannot refuse a login. This is the right follow-up if
   the `info` sitting is judged insufficient for R1.
2. **`pam-u2f`** (also in `extra`). Closer to the real thing, needs hardware.
3. **A ten-line C PAM module** that calls `pam_prompt(pamh,
   PAM_PROMPT_ECHO_ON, ...)` once. Most faithful to "an extra prompt", and the
   most code to audit for the smallest requirement.

None of those are installed and installing them is a `pacman` command, i.e.
root, i.e. yours — this document does not print one, because adding a package
to the machine is a bigger decision than adding a line to a file you can
`diff`.

---

## 7. What to do if a greetd login stops working

In this order, from the root shell of §5:

```sh
# 1. Is it the new line? Take it out and retry.
install -m 0644 /etc/pam.d/greetd.auroval-backup /etc/pam.d/greetd

# 2. What did PAM actually say?
journalctl -b -t greetd -n 50 --no-pager
journalctl -b _COMM=greetd -n 50 --no-pager

# 3. Did the helper run at all?
cat /var/log/auroval-pam-extra.log
```

SDDM is still `enabled` and still the way into this machine. Rebooting gets you
a working login screen regardless of what `/etc/pam.d/greetd` says.
