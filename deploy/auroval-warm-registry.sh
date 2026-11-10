#!/bin/sh
# Re-warm the greeter's GStreamer plugin registry after the plugins change.
#
# WHY THIS EXISTS. C15: greetd's greeter user has HOME=/, which is not
# writable, so GStreamer cannot persist its plugin registry and rescans every
# plugin on every start. deploy/auroval.tmpfiles.conf gives it
# /var/cache/auroval instead, and deploy/auroval-greet.sh points GST_REGISTRY
# there. That registry survives reboots, so the scan is paid once rather than
# per login -- and deploy/install.nu pays it at install time.
#
# Issue #77 is what install.nu does NOT cover. The registry is invalidated
# whenever the plugins underneath it change, which on a rolling distribution
# is every gstreamer update, roughly monthly. An install that happened weeks
# ago cannot warm a registry that was invalidated this morning, and nothing
# else did: measured on a real VT, that first login costs between 1.3 s and
# 1.9 s to first frame, against R16's 500 ms budget. Receipts in
# spike/p1e/cage-run-2026-11-09-1019.txt and -2026-11-10-0802.txt.
#
# So this runs from a pacman hook on the plugin directory itself
# (deploy/auroval-warm-registry.hook) and pays the scan during the update,
# where a second is invisible, instead of at the next login, where it is a
# black screen. It is also what install.nu's root script calls, so the install
# and the hook cannot drift apart.
#
# EXIT CODES, because a hook's exit code is a message to the person updating:
#   0  the registry is warm, or auroval is not installed on this machine
#   1  auroval IS installed here and the registry could not be warmed
# A failure is worth a line from pacman -- it means the next login is slow --
# and is never worth more than that. Nothing here can break a boot: the
# registry is a cache in the FHS sense, and a greeter with none rebuilds it.

set -u

CACHE=/var/cache/auroval
REGISTRY="$CACHE/gst-registry.bin"
PLUGINS=/usr/lib/gstreamer-1.0

# Not applicable rather than broken. /var/cache/auroval is created by
# auroval's tmpfiles snippet, so its absence means auroval's greeter was
# never installed here -- or was removed -- and there is no registry to warm.
if [ ! -d "$CACHE" ]; then
    exit 0
fi

fail() {
    echo "auroval: $1" >&2
    echo "auroval: the greeter's registry is stale, so its next start pays the" >&2
    echo "auroval: plugin scan once -- about a second of black screen. Fix it" >&2
    echo "auroval: with: sudo /usr/local/lib/auroval/auroval-warm-registry.sh" >&2
    exit 1
}

[ "$(id -u)" = 0 ] || fail "this has to run as root; it writes as the greeter user"
command -v gst-inspect-1.0 >/dev/null 2>&1 || fail "gst-inspect-1.0 is not installed"
command -v runuser >/dev/null 2>&1 || fail "runuser is not available"
id greeter >/dev/null 2>&1 || fail "there is no greeter user"

# gst-inspect decides for itself what is stale: it compares each plugin's
# mtime and size against what the registry records and rewrites only what
# changed. So the stale registry is deliberately left in place rather than
# deleted -- revalidating it measured 444 ms on this machine against the
# 0.9-1.0 s a rebuild from nothing costs, and the result is the same registry.
runuser -u greeter -- env \
    XDG_CACHE_HOME="$CACHE" \
    GST_REGISTRY="$REGISTRY" \
    gst-inspect-1.0 >/dev/null 2>&1 \
    || fail "gst-inspect-1.0 failed as the greeter user"

# The property we actually want, asked of the filesystem rather than inferred
# from an exit code: the registry postdates every plugin it claims to index.
# gst-inspect can return 0 having written nothing -- an unwritable registry
# path is the obvious way -- and then the login pays the scan while this
# script reports success.
[ -s "$REGISTRY" ] || fail "$REGISTRY is missing or empty after a successful scan"

newest=""
for so in "$PLUGINS"/*.so; do
    [ -e "$so" ] || continue
    if [ -z "$newest" ] || [ "$so" -nt "$newest" ]; then
        newest="$so"
    fi
done
if [ -n "$newest" ] && [ ! "$REGISTRY" -nt "$newest" ]; then
    fail "$REGISTRY is not newer than $newest, so it was not rewritten"
fi

exit 0
