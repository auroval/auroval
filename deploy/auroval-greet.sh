#!/bin/sh
# greetd's command. Runs as the `greeter` user, execs cage, which execs the
# greeter. Nothing else belongs here.
#
# Deliberately /bin/sh and not nushell, which is this project's convention
# everywhere else. This file is on the greeter's startup path and C15 measured
# that path to the millisecond: a shell that takes 30 ms to start has spent 6%
# of R16's budget before the greeter has drawn a pixel.
#
# C15, the whole reason this file exists rather than greetd calling the binary
# directly: the greeter's home is `/` and is not writable, so GStreamer cannot
# persist its plugin registry and rescans every plugin on every start.
# Measured as the `greeter` user, process start to GStreamer ready:
#
#     no writable cache (the default)  1018 ms
#     writable cache, registry absent  1011 ms
#     writable cache, registry warm      93 ms
#
# So these four lines are worth more than nine tenths of a second on every
# single boot — most of R16's budget — and they are deployment, not code.
# /var/cache/auroval must exist and be owned by `greeter` before this runs;
# deploy/auroval.tmpfiles.conf is what makes that true across reboots.
#
# The font half of C15 is handled in the greeter itself, which loads fonts by
# path rather than through fontconfig, so there is nothing to set here for it.

XDG_CACHE_HOME=/var/cache/auroval
GST_REGISTRY=/var/cache/auroval/gst-registry.bin
export XDG_CACHE_HOME GST_REGISTRY

# R4's memory lives here and the greeter owns it. Not exported for the
# greeter's benefit — it has the path compiled in — but so that anything run
# under this script, including a probe, agrees about where state is.
XDG_STATE_HOME=/var/lib/auroval
export XDG_STATE_HOME

# `greet` is named here rather than left to a default, and it is the whole
# contract between greetd and this binary: greetd's config passes no
# arguments, so without a mode the greeter printed its usage and exited 2 on
# every single start. It was invisible while the binary had no shell to run,
# because every run exited 2 for that reason instead.
# -s allows VT switching, and it is OFF in cage by default. Without it the
# greeter's console is a room with no door: Ctrl+Alt+F1 does nothing, and a
# greeter that hangs takes the whole machine with it until someone reaches it
# over the network or holds the power button. Found on 2026-11-08, by an
# operator who could not leave.
#
# It costs nothing that matters. The VT a person switches to is a getty, which
# asks for a password like anything else, so this is not a way past
# authentication. And it is the greeter only: C5's fail-closed argument is
# about the ext-session-lock locker, which is a different surface and does not
# run under cage.
exec cage -s -- /usr/local/lib/auroval/auroval-greet greet "$@"
