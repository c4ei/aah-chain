#!/usr/bin/env bash
set -euo pipefail

service_name="${IEUM_SERVICE_NAME:-ieum-chain.service}"
binary_path="${IEUM_BINARY_PATH:-/opt/ieum-chain/ieum-chain}"
manifest_url="${IEUM_UPDATE_MANIFEST_URL:?IEUM_UPDATE_MANIFEST_URL is required}"
release_public_key="${IEUM_RELEASE_PUBLIC_KEY:?IEUM_RELEASE_PUBLIC_KEY is required}"
rpc_url="${IEUM_RPC_URL:-http://127.0.0.1:8989}"

systemctl stop "$service_name"

if ! "$binary_path" update \
    --manifest-url "$manifest_url" \
    --release-public-key "$release_public_key"; then
    systemctl start "$service_name"
    exit 1
fi

systemctl start "$service_name"

healthy=0
for _ in $(seq 1 30); do
    if curl --fail --silent --show-error \
        -H 'content-type: application/json' \
        --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
        "$rpc_url" | grep -q '"result"'; then
        healthy=1
        break
    fi
    sleep 2
done

if [[ "$healthy" -eq 1 ]]; then
    exit 0
fi

systemctl stop "$service_name"
if [[ ! -f "${binary_path}.previous" ]]; then
    echo "업데이트 후 상태 확인에 실패했고 이전 바이너리도 없습니다." >&2
    exit 1
fi
cp --preserve=mode,ownership,timestamps "${binary_path}.previous" "$binary_path"
systemctl start "$service_name"
echo "상태 확인 실패로 이전 버전을 복구했습니다." >&2
exit 1
