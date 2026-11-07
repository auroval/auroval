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
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libgstreamer1.0-dev \
        libgstreamer-plugins-base1.0-dev \
        libwayland-dev \
        libxkbcommon-dev \
        libudev-dev \
        libseat-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
