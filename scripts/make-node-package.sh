#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="${1:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root_dir/Cargo.toml" | head -n 1)}"
binary="${IEUM_LOCAL_BINARY:-$root_dir/target/release/ieum-chain}"
output_dir="${IEUM_OUTPUT_DIR:-$root_dir/download}"
archive="$output_dir/ieum-chain_node_ubuntu_x86_64_v${version}.tar.xz"
checksum="$output_dir/ieum-chain_node_ubuntu_x86_64_v${version}.sha256"

[[ -n "$version" ]] || {
    echo "Cargo.toml에서 버전을 확인할 수 없습니다." >&2
    exit 1
}
if [[ "${IEUM_SKIP_BUILD:-0}" != "1" ]]; then
    cd "$root_dir"
    cargo fmt --all --check
    cargo clippy --all-targets --all-features --locked -- -D warnings
    cargo test --all-targets --all-features --locked
    cargo build --release --locked
fi

[[ -x "$binary" ]] || {
    echo "실행파일이 없습니다: $binary" >&2
    echo "IEUM_SKIP_BUILD=1 사용 시 실행파일을 먼저 빌드해야 합니다." >&2
    exit 1
}

reported_version="$("$binary" --version)"
if [[ "$reported_version" != *"$version"* ]]; then
    echo "실행파일 버전이 요청 버전과 다릅니다: $reported_version / $version" >&2
    exit 1
fi

stage_dir="$(mktemp -d)"
cleanup() {
    rm -rf -- "$stage_dir"
}
trap cleanup EXIT

package_root="$stage_dir/ieum-chain-node-v${version}"
install -d "$package_root/config"
install -m 755 "$binary" "$package_root/ieum-chain"
install -m 755 "$root_dir/scripts/install-node-package.sh" "$package_root/install.sh"
install -m 644 "$root_dir/scripts/ieum-chain.service.in" \
    "$package_root/ieum-chain.service.in"

for config_name in genesis.json events.json upgrades.json bootstrap.json; do
    if [[ -f "$root_dir/config/$config_name" ]]; then
        install -m 644 "$root_dir/config/$config_name" \
            "$package_root/config/$config_name"
    fi
done
if [[ -f "$root_dir/config/update.example.json" ]]; then
    install -m 644 "$root_dir/config/update.example.json" \
        "$package_root/config/update.example.json"
fi

install -d "$output_dir"
tar -C "$stage_dir" -cJf "$archive" "ieum-chain-node-v${version}"
(
    cd "$output_dir"
    sha256sum "$(basename "$archive")" >"$(basename "$checksum")"
)

echo "노드 배포본 생성 완료:"
echo "  $archive"
echo "  $checksum"
echo
echo "대상 서버:"
echo "  tar -xJf $(basename "$archive")"
echo "  cd ieum-chain-node-v${version}"
echo "  sudo ./install.sh"
