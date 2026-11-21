#!/bin/sh
# Install the pinned Zola into $1 (default /usr/local/bin).
#
# This is a script rather than two copies of the same shell, because unlike the
# apt list in system-deps.txt the work here is long enough that two hand-copied
# versions would drift: download, verify, extract, check. Both readers run this
# exact file.
#
#   .devcontainer/Dockerfile           COPYs and runs it at image build time
#   .github/actions/zola/action.yml    runs it from the checkout on the runner
#
# POSIX sh, not bash: the Dockerfile's base image is Debian and this runs
# before anything is installed, so it assumes nothing beyond sh, curl and tar.
set -eu

bindir="${1:-/usr/local/bin}"
here="$(cd "$(dirname "$0")" && pwd)"
pin="$here/zola-version.txt"

test -f "$pin" || { echo "missing $pin" >&2; exit 1; }

# One parser for the whole file: strip comments, then read `key [arch] value`.
field() {
    sed -e 's/#.*//' "$pin" | awk -v k="$1" -v a="${2:-}" '
        $1 != k { next }
        a == "" { print $2; exit }
        $2 == a { print $3; exit }
    '
}

arch="$(uname -m)"
version="$(field version)"
sha="$(field sha256 "$arch")"
target="$(field target "$arch")"

# Every one of these being empty is a different mistake, and a single "failed"
# would send the reader to the wrong one. An unknown architecture in particular
# must stop here: falling through to an unverified download is the failure this
# file exists to prevent.
test -n "$version" || { echo "no version in $pin" >&2; exit 1; }
test -n "$sha"     || { echo "no sha256 for $arch in $pin -- add one, do not skip verification" >&2; exit 1; }
test -n "$target"  || { echo "no target triple for $arch in $pin" >&2; exit 1; }

tarball="zola-v${version}-${target}.tar.gz"
url="https://github.com/getzola/zola/releases/download/v${version}/${tarball}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "fetching $url"
curl -fsSL --retry 3 -o "$tmp/$tarball" "$url"

echo "$sha  $tmp/$tarball" | sha256sum -c - >/dev/null || {
    echo "checksum mismatch for $tarball" >&2
    echo "  expected $sha" >&2
    echo "  actual   $(sha256sum "$tmp/$tarball" | cut -d' ' -f1)" >&2
    exit 1
}

# The archive is a bare `zola` binary at the root; -C into the destination
# rather than extracting here and moving, so a partial extract cannot leave a
# half-written binary on PATH under its final name.
tar -xzf "$tmp/$tarball" -C "$tmp"
install -m 0755 "$tmp/zola" "$bindir/zola"

# Prove it runs, in the environment it was installed into. A musl binary on a
# glibc image and a corrupt extract both look fine until the first build.
"$bindir/zola" --version
