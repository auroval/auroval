# Debian, not Arch, on purpose.
#
# The container exists to build, test and lint the crates, and to match what CI
# runs so a green pull request means something. It is deliberately NOT an
# attempt to reproduce the Arch host: the P0 spike needs /dev/dri, DRM master on
# a real TTY and greetd's own system user, none of which survive a container's
# user and device namespaces. Hardware measurements happen on the host.
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Build dependencies for the two things this workspace links against: GStreamer
# (video decode) and Wayland (the client surfaces).
#
# The second group is what `cargo test` needs rather than what the build needs.
# The render tests in crates/greet/src/view.rs draw through a surfaceless EGL
# context and read the pixels back, so they want Mesa's software rasteriser
# (libegl-mesa0 is the driver, libgl1-mesa-dri carries llvmpipe) and a font on
# disk, because the atlas loads fonts by path and never through fontconfig.
# Without them `cargo test` here fails three tests that pass on the host and in
# CI, which is precisely the divergence this container exists to avoid.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libgstreamer1.0-dev \
        libgstreamer-plugins-base1.0-dev \
        libwayland-dev \
        libxkbcommon-dev \
        libudev-dev \
        libseat-dev \
        libegl1 \
        libegl-mesa0 \
        libgbm1 \
        libgl1-mesa-dri \
        fonts-dejavu-core \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# The same pin CI's test job uses: these tests assert pixel values, so the
# rasteriser has to be the same one every run.
ENV EGL_PLATFORM=surfaceless \
    GALLIUM_DRIVER=llvmpipe \
    LIBGL_ALWAYS_SOFTWARE=1
