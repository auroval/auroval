#!/bin/sh
# Runs as greetd's `greeter` user, started by greetd. Execs cage, which execs
# the spike. Nothing else belongs here.
#
# Deliberately /bin/sh and not nushell, which is this project's convention
# elsewhere: this file is on the greeter's startup path, and C15 measured that
# path to the millisecond. A shell with a 30 ms start is a shell that spends
# R16's budget before the greeter has drawn anything.

# C15: the greeter's home is `/` and is not writable, so without this GStreamer
# rescans every plugin on every start — about 0.9 s, measured.
XDG_CACHE_HOME=/var/lib/auroval-spike/cache
GST_REGISTRY=/var/lib/auroval-spike/cache/gst-registry.bin
export XDG_CACHE_HOME GST_REGISTRY

exec cage -- /var/lib/auroval-spike/spike-p0 greet \
    /var/lib/auroval-spike/media/loop1440p60.h264.mp4 \
    --user "${AUROVAL_LOGIN_USER:-dirk}" \
    --session /var/lib/auroval-spike/session-proof.sh
