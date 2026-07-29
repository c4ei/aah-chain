#!/usr/bin/env bash
set -euo pipefail

version="${1:?usage: scripts/build-ubuntu-release.sh VERSION}"
root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
archive="$root_dir/download/ieum-chain_ubuntu_x86_64_v${version}.tar.xz"
binary="$root_dir/download/ieum-chain_ubuntu_x86_64_v${version}"

cd "$root_dir"
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked

install -m 755 target/release/ieum-chain "$binary"
tar -C target/release -cJf "$archive" ieum-chain
sha256sum "$binary" "$archive" | tee "$root_dir/download/SHA256SUMS-v${version}.txt"
tar -tJf "$archive"
"$binary" --version

echo
echo "Git 추가 대상:"
echo "  download/$(basename "$binary")"
echo "  download/$(basename "$archive")"
echo "  download/SHA256SUMS-v${version}.txt"
