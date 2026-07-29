#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -lt 1 ]]; then
    echo "사용법: $0 user@host [user@host ...]" >&2
    exit 2
fi

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${IEUM_LOCAL_BINARY:-$root_dir/target/release/ieum-chain}"
remote_binary="${IEUM_REMOTE_BINARY:-/opt/ieum-chain/ieum-chain}"
service="${IEUM_SERVICE_NAME:-ieum-chain.service}"
rpc_url="${IEUM_RPC_URL:-http://127.0.0.1:8989}"
ssh_options=(-o BatchMode=yes -o ConnectTimeout=10)

[[ -x "$binary" ]] || {
    echo "배포할 실행파일이 없습니다: $binary" >&2
    exit 1
}

version="$("$binary" --version)"
sha256="$(sha256sum "$binary" | awk '{print $1}')"
echo "순차 배포 시작: $version ($sha256)"

for host in "$@"; do
    remote_tmp="/tmp/ieum-chain.deploy.$$"
    echo
    echo "[$host] 업로드"
    scp "${ssh_options[@]}" "$binary" "$host:$remote_tmp"

    ssh "${ssh_options[@]}" "$host" \
        "IEUM_DEPLOY_TMP='$remote_tmp' IEUM_EXPECTED_SHA256='$sha256' IEUM_REMOTE_BINARY='$remote_binary' IEUM_SERVICE_NAME='$service' IEUM_RPC_URL='$rpc_url' bash -s" <<'REMOTE'
set -euo pipefail

cleanup() {
    rm -f "$IEUM_DEPLOY_TMP"
}
trap cleanup EXIT

actual_sha256="$(sha256sum "$IEUM_DEPLOY_TMP" | awk '{print $1}')"
[[ "$actual_sha256" == "$IEUM_EXPECTED_SHA256" ]] || {
    echo "SHA-256 검증 실패" >&2
    exit 1
}
chmod 755 "$IEUM_DEPLOY_TMP"
"$IEUM_DEPLOY_TMP" --version

sudo systemctl stop "$IEUM_SERVICE_NAME"
sudo cp --preserve=mode,ownership,timestamps \
    "$IEUM_REMOTE_BINARY" "${IEUM_REMOTE_BINARY}.previous"
sudo install -m 755 "$IEUM_DEPLOY_TMP" "$IEUM_REMOTE_BINARY"
sudo systemctl start "$IEUM_SERVICE_NAME"

healthy=0
for _ in $(seq 1 30); do
    if curl --fail --silent \
        -H 'content-type: application/json' \
        --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
        "$IEUM_RPC_URL" | grep -q '"result"'; then
        healthy=1
        break
    fi
    sleep 2
done

if [[ "$healthy" -eq 1 ]]; then
    "$IEUM_REMOTE_BINARY" --version
    exit 0
fi

echo "상태 확인 실패: 이전 버전으로 복구합니다." >&2
sudo systemctl stop "$IEUM_SERVICE_NAME"
sudo cp --preserve=mode,ownership,timestamps \
    "${IEUM_REMOTE_BINARY}.previous" "$IEUM_REMOTE_BINARY"
sudo systemctl start "$IEUM_SERVICE_NAME"
exit 1
REMOTE
    echo "[$host] 정상 배포 완료"
done

echo
echo "모든 서버의 순차 배포가 완료됐습니다."
