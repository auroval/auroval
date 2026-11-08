# Debian, not Arch, on purpose.
#
# The container exists to build, test and lint the crates, and to match what CI
# runs so a green pull request means something. It is deliberately NOT an
# attempt to reproduce the Arch host: the P0 spike needs /dev/dri, DRM master on
# a real TTY and greetd's own system user, none of which survive a container's
# user and device namespaces. Hardware measurements happen on the host.
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# The package list is not written here. It lives in system-deps.txt, next to
# this file, and the composite action CI uses installs that same file -- so the
# container and the runner cannot drift apart, which is the one guarantee this
# container is for. The reasons each package is in the list are in that file;
# read it before changing anything here.
#
# This works because `.devcontainer` is the build context, both for the
# `devcontainer builds` CI job and for devcontainer.json, so system-deps.txt is
# a sibling `COPY` can see.
#
# The sed strips comments and blank lines; what remains is package names, which
# apt wants as separate arguments, so $packages is deliberately unquoted. An
# empty list would make `apt-get install` succeed having installed nothing, and
# that failure would surface much later as three failing render tests, so the
# `test -n` fails here instead.
COPY system-deps.txt /tmp/system-deps.txt
RUN set -eu; \
    export DEBIAN_FRONTEND=noninteractive; \
    packages="$(sed -e 's/#.*//' -e '/^[[:space:]]*$/d' /tmp/system-deps.txt)"; \
    test -n "$packages" || { echo 'system-deps.txt parsed to no packages' >&2; exit 1; }; \
    echo "installing: $packages"; \
    apt-get update; \
    apt-get install -y --no-install-recommends $packages; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/* /tmp/system-deps.txt

# The same pin CI's test job uses: these tests assert pixel values, so the
# rasteriser has to be the same one every run. This pair cannot be shared the
# way the package list is -- here it is an ENV baked into the image, there it is
# an `env:` block on one step -- so system-deps.txt ends with a note saying both
# exist, for whoever changes one of them.
ENV EGL_PLATFORM=surfaceless \
    GALLIUM_DRIVER=llvmpipe \
    LIBGL_ALWAYS_SOFTWARE=1
