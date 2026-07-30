#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -lt 2 ]]; then
    echo "사용법: $0 배포압축.tar.xz user@host [user@host ...]" >&2
    exit 2
fi

archive="$(realpath "$1")"
shift
[[ -f "$archive" ]] || {
    echo "배포 압축이 없습니다: $archive" >&2
    exit 1
}

archive_name="$(basename "$archive")"
sha256="$(sha256sum "$archive" | awk '{print $1}')"
ssh_options=(-o BatchMode=yes -o ConnectTimeout=10)

for host in "$@"; do
    remote_archive="/tmp/$archive_name"
    echo "[$host] 배포본 업로드"
    scp "${ssh_options[@]}" "$archive" "$host:$remote_archive"
    ssh -t "${ssh_options[@]}" "$host" \
        "set -e; echo '$sha256  $remote_archive' | sha256sum -c -; \
tmp_dir=\$(mktemp -d); \
trap 'rm -rf -- \"\$tmp_dir\" \"$remote_archive\"' EXIT; \
tar -xJf \"$remote_archive\" -C \"\$tmp_dir\"; \
package_dir=\$(find \"\$tmp_dir\" -mindepth 1 -maxdepth 1 -type d -name 'ieum-chain-node-v*' -print -quit); \
test -n \"\$package_dir\"; \
cd \"\$package_dir\"; \
sudo IEUM_RUN_USER='${host%%@*}' ./install.sh"
    echo "[$host] 설치 및 실행 완료"
done
