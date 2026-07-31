#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="${1:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root_dir/Cargo.toml" | head -n 1)}"
binary="${IEUM_LOCAL_BINARY:-$root_dir/target/release/ieum-chain}"
output_dir="${IEUM_OUTPUT_DIR:-$root_dir/download}"
archive="$output_dir/ieum-chain_node_ubuntu_x86_64_v${version}.tar.xz"
checksum="$output_dir/ieum-chain_node_ubuntu_x86_64_v${version}.sha256"
update_binary="$output_dir/ieum-chain_linux_x86_64_v${version}"
manifest="$output_dir/update-manifest.json"
update_config="$root_dir/config/update.json"
private_key="${IEUM_RELEASE_PRIVATE_KEY:-$root_dir/backups/release-private.pem}"
artifact_base_url="${IEUM_ARTIFACT_BASE_URL:-https://raw.githubusercontent.com/c4ei/ieum-chain/main/download}"
manifest_url="${IEUM_MANIFEST_URL:-$artifact_base_url/update-manifest.json}"
protocol_version="${IEUM_PROTOCOL_VERSION:-$(sed -n 's/^const SUPPORTED_PROTOCOL_VERSION: u32 = \([0-9][0-9]*\);/\1/p' "$root_dir/src/main.rs" | head -n 1)}"

[[ -n "$version" ]] || {
    echo "Cargo.toml에서 버전을 확인할 수 없습니다." >&2
    exit 1
}
[[ -n "$protocol_version" ]] || {
    echo "src/main.rs에서 프로토콜 버전을 확인할 수 없습니다." >&2
    exit 1
}
for command_name in openssl python3 sha256sum; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "필수 명령이 없습니다: $command_name" >&2
        exit 1
    }
done
[[ -f "$private_key" ]] || {
    echo "릴리스 개인키가 없습니다: $private_key" >&2
    echo "IEUM_RELEASE_PRIVATE_KEY로 다른 경로를 지정할 수 있습니다." >&2
    exit 1
}
key_mode="$(stat -c '%a' "$private_key")"
if (( (8#$key_mode & 8#077) != 0 )); then
    echo "릴리스 개인키 권한이 안전하지 않습니다($key_mode)." >&2
    echo "chmod 600 \"$private_key\" 후 다시 실행하세요." >&2
    exit 1
fi
if ! openssl pkey -in "$private_key" -text_pub -noout 2>/dev/null |
    grep -q 'ED25519 Public-Key'; then
    echo "릴리스 개인키는 Ed25519 PEM이어야 합니다: $private_key" >&2
    exit 1
fi

if [[ "${IEUM_SKIP_BUILD:-0}" != "1" ]]; then
    cd "$root_dir"
    # 새 libp2p 기능이 추가된 변경 파일 패키지를 적용한 직후에도 Cargo.lock에
    # 필요한 전이 의존성을 먼저 추가한 뒤 --locked 검증을 계속할 수 있게 합니다.
    cargo fetch
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

install -d "$output_dir"
install -m 755 "$binary" "$update_binary"

release_public_key="$(
    openssl pkey -in "$private_key" -pubout -outform DER 2>/dev/null |
        tail -c 32 |
        python3 -c 'import sys; print(sys.stdin.buffer.read().hex())'
)"
[[ "${#release_public_key}" -eq 64 ]] || {
    echo "릴리스 공개키 추출에 실패했습니다." >&2
    exit 1
}
update_sha256="$(sha256sum "$update_binary" | awk '{print $1}')"
published_at="$(date +%s)"
unsigned_manifest="$stage_dir/update-manifest.unsigned.json"
signature_file="$stage_dir/update-manifest.signature"

VERSION="$version" \
PROTOCOL_VERSION="$protocol_version" \
PUBLISHED_AT="$published_at" \
ARTIFACT_URL="$artifact_base_url/$(basename "$update_binary")" \
ARTIFACT_SHA256="$update_sha256" \
UNSIGNED_MANIFEST="$unsigned_manifest" \
python3 - <<'PY'
import json
import os

manifest = {
    "version": os.environ["VERSION"],
    "protocol_version": int(os.environ["PROTOCOL_VERSION"]),
    "mandatory": False,
    "published_at": int(os.environ["PUBLISHED_AT"]),
    "notes": f"IEUM Chain v{os.environ['VERSION']} signed release",
    "artifacts": {
        "linux-x86_64": {
            "url": os.environ["ARTIFACT_URL"],
            "sha256": os.environ["ARTIFACT_SHA256"],
        }
    },
}
with open(os.environ["UNSIGNED_MANIFEST"], "w", encoding="utf-8") as file:
    json.dump(manifest, file, ensure_ascii=False, separators=(",", ":"))
PY

openssl pkeyutl -sign -rawin -inkey "$private_key" \
    -in "$unsigned_manifest" -out "$signature_file"
signature_hex="$(python3 -c 'import pathlib, sys; print(pathlib.Path(sys.argv[1]).read_bytes().hex())' "$signature_file")"
[[ "${#signature_hex}" -eq 128 ]] || {
    echo "manifest Ed25519 서명 생성에 실패했습니다." >&2
    exit 1
}

UNSIGNED_MANIFEST="$unsigned_manifest" \
SIGNED_MANIFEST="$manifest" \
SIGNATURE_HEX="$signature_hex" \
UPDATE_CONFIG="$update_config" \
MANIFEST_URL="$manifest_url" \
RELEASE_PUBLIC_KEY="$release_public_key" \
python3 - <<'PY'
import json
import os

with open(os.environ["UNSIGNED_MANIFEST"], encoding="utf-8") as file:
    manifest = json.load(file)
manifest["signature"] = os.environ["SIGNATURE_HEX"]
with open(os.environ["SIGNED_MANIFEST"], "w", encoding="utf-8") as file:
    json.dump(manifest, file, ensure_ascii=False, indent=2)
    file.write("\n")

config = {
    "enabled": True,
    "manifest_url": os.environ["MANIFEST_URL"],
    "release_public_key": os.environ["RELEASE_PUBLIC_KEY"],
    "check_interval_secs": 300,
}
with open(os.environ["UPDATE_CONFIG"], "w", encoding="utf-8") as file:
    json.dump(config, file, ensure_ascii=False, indent=2)
    file.write("\n")
PY

# 생성한 서명을 실제 개인키의 공개키로 다시 검증한다.
openssl pkey -in "$private_key" -pubout \
    -out "$stage_dir/release-public.pem" 2>/dev/null
openssl pkeyutl -verify -rawin -pubin \
    -inkey "$stage_dir/release-public.pem" \
    -in "$unsigned_manifest" -sigfile "$signature_file" >/dev/null

package_root="$stage_dir/ieum-chain-node-v${version}"
install -d "$package_root/config"
install -m 755 "$binary" "$package_root/ieum-chain"
install -m 755 "$root_dir/scripts/install-node-package.sh" "$package_root/install.sh"
install -m 644 "$root_dir/scripts/ieum-chain.service.in" \
    "$package_root/ieum-chain.service.in"
manual="$root_dir/docs/USER_MANUAL_${version}.md"
if [[ ! -f "$manual" ]]; then
    manual="$root_dir/docs/USER_MANUAL_0.19.9.md"
fi
install -m 644 "$manual" "$package_root/USER_MANUAL.md"
if [[ -f "$root_dir/docs/VERSION_${version}.md" ]]; then
    install -m 644 "$root_dir/docs/VERSION_${version}.md" \
        "$package_root/RELEASE_NOTES.md"
fi

for config_name in genesis.json events.json upgrades.json bootstrap.json update.json; do
    if [[ -f "$root_dir/config/$config_name" ]]; then
        install -m 644 "$root_dir/config/$config_name" \
            "$package_root/config/$config_name"
    fi
done

tar -C "$stage_dir" -cJf "$archive" "ieum-chain-node-v${version}"
(
    cd "$output_dir"
    sha256sum "$(basename "$archive")" >"$(basename "$checksum")"
)

echo "서명된 노드 릴리스 생성 완료:"
echo "  $archive"
echo "  $checksum"
echo "  $update_binary"
echo "  $manifest"
echo "  $update_config"
echo "릴리스 공개키(hex): $release_public_key"
echo "개인키는 배포본과 Git에 포함되지 않았습니다: $private_key"
echo
echo "검토 후 게시:"
echo "  git add Cargo.toml Cargo.lock CHANGELOG.md README.md config/update.json download scripts docs src"
echo "  git commit -m \"IEUM v${version} signed release\""
echo "  git push"
echo
echo "v0.19.7 노드는 최초 한 번 v${version} 패키지를 설치해야 합니다:"
echo "  tar -xJf $(basename "$archive")"
echo "  cd ieum-chain-node-v${version}"
echo "  sudo ./install.sh"
