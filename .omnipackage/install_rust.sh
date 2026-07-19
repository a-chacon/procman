#!/usr/bin/env bash
# Ensures a Rust toolchain new enough for edition 2024 (rustc >= 1.85).
# Keeps the distro toolchain when it qualifies, else downloads a current one.
# Runs inside the build container; safe to re-run.

set -xEeuo pipefail

MIN_MINOR=85
RUST_VERSION=1.95.0

if command -v cargo >/dev/null 2>&1; then
    ver=$(cargo --version | awk '{print $2}')   # e.g. 1.86.0
    major=${ver%%.*}
    minor=${ver#*.}; minor=${minor%%.*}
    if [ "${major:-0}" -gt 1 ] || { [ "${major:-0}" -eq 1 ] && [ "${minor:-0}" -ge "$MIN_MINOR" ]; }; then
        echo "Using system cargo $ver"
        exit 0
    fi
    echo "System cargo $ver is too old for edition 2024; downloading $RUST_VERSION"
fi

ARCH=$(uname -m)
curl -O https://static.rust-lang.org/dist/rust-${RUST_VERSION}-${ARCH}-unknown-linux-gnu.tar.gz
tar -xzf rust-*.tar.gz
cd rust-*/
./install.sh --prefix=/usr
cd ..
rm -rf rust-*
