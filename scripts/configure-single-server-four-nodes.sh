#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
    echo "sudo $0 LAN_IP 로 실행하세요." >&2
    exit 1
fi

lan_ip="${1:-}"
if [[ ! "$lan_ip" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]]; then
    echo "사용법: sudo $0 192.168.1.148" >&2
    exit 1
fi

run_user="${IEUM_RUN_USER:-${SUDO_USER:-}}"
if [[ -z "$run_user" || "$run_user" == "root" ]]; then
    echo "IEUM_RUN_USER=dev 처럼 노드 실행 계정을 지정하세요." >&2
    exit 1
fi
id "$run_user" >/dev/null 2>&1 || {
    echo "실행 계정이 없습니다: $run_user" >&2
    exit 1
}
run_group="$(id -gn "$run_user")"

dirs=(/opt/ieum-chain /opt/ieum-node1 /opt/ieum-node2 /opt/ieum-node3)
services=(ieum-chain ieum-node1 ieum-node2 ieum-node3)
p2p_ports=(7001 7002 7003 7004)
rpc_ports=(8989 8990 8991 8992)
node_keys=(data/server.node.key data/server.node.key data/server.node.key data/server.node.key)
declare -a peer_ids

source_binary="${IEUM_SOURCE_BINARY:-/opt/ieum-chain/ieum-chain}"
if [[ ! -x "$source_binary" ]]; then
    echo "기준 실행파일이 없습니다: $source_binary" >&2
    exit 1
fi

for index in 0 1 2 3; do
    dir="${dirs[$index]}"
    for required in \
        config/validator.key \
        config/validators.json \
        config/events.json \
        "${node_keys[$index]}"
    do
        if [[ ! -f "$dir/$required" ]]; then
            echo "필수 운영 파일이 없습니다: $dir/$required" >&2
            exit 1
        fi
    done
done

validator_hash="$(sha256sum "${dirs[0]}/config/validators.json" | awk '{print $1}')"
for index in 1 2 3; do
    current_hash="$(sha256sum "${dirs[$index]}/config/validators.json" | awk '{print $1}')"
    if [[ "$current_hash" != "$validator_hash" ]]; then
        echo "네 노드의 validators.json이 동일하지 않습니다." >&2
        echo "불일치: ${dirs[$index]}/config/validators.json" >&2
        exit 1
    fi
done

for service in "${services[@]}"; do
    systemctl stop "$service" 2>/dev/null || true
done

for index in 0 1 2 3; do
    dir="${dirs[$index]}"
    install -d -m 750 -o "$run_user" -g "$run_group" \
        "$dir" "$dir/config" "$dir/data"

    if [[ "$dir/ieum-chain" != "$source_binary" ]]; then
        if [[ -x "$dir/ieum-chain" ]]; then
            cp --preserve=mode,ownership,timestamps \
                "$dir/ieum-chain" "$dir/ieum-chain.previous"
        fi
        install -m 755 -o "$run_user" -g "$run_group" \
            "$source_binary" "$dir/ieum-chain"
    fi

    peer_ids[$index]="$(
        cd "$dir"
        runuser -u "$run_user" -- \
            "$dir/ieum-chain" node peer-id --key "${node_keys[$index]}"
    )"
    if [[ ! "${peer_ids[$index]}" =~ ^12D3KooW[1-9A-HJ-NP-Za-km-z]+$ ]]; then
        echo "PeerId 계산 결과가 올바르지 않습니다: $dir/${node_keys[$index]}" >&2
        exit 1
    fi
done

for index in 0 1 2 3; do
    dir="${dirs[$index]}"
    peer_args=()
    for peer_index in 0 1 2 3; do
        if [[ "$peer_index" -ne "$index" ]]; then
            peer_args+=(
                --bootstrap
                "/ip4/$lan_ip/udp/${p2p_ports[$peer_index]}/quic-v1/p2p/${peer_ids[$peer_index]}"
            )
        fi
    done
    (
        cd "$dir"
        runuser -u "$run_user" -- \
            "$dir/ieum-chain" network set \
            "${peer_args[@]}" \
            --advertise-address \
            "/dns4/node.ieum.aah.name/udp/${p2p_ports[$index]}/quic-v1/p2p/${peer_ids[$index]}"
    )
done

timestamp="$(date +%Y%m%d_%H%M%S)"
for index in 0 1 2 3; do
    dir="${dirs[$index]}"
    service="${services[$index]}"
    unit_path="/etc/systemd/system/$service.service"
    if [[ -f "$unit_path" ]]; then
        cp --preserve=mode,ownership,timestamps \
            "$unit_path" "$unit_path.bak_$timestamp"
    fi
    sed \
        -e "s|@DESCRIPTION@|IEUM Chain local validator $((index + 1))|g" \
        -e "s|@RUN_USER@|$run_user|g" \
        -e "s|@RUN_GROUP@|$run_group|g" \
        -e "s|@INSTALL_DIR@|$dir|g" \
        -e "s|@P2P_PORT@|${p2p_ports[$index]}|g" \
        -e "s|@RPC_PORT@|${rpc_ports[$index]}|g" \
        -e "s|@NODE_KEY@|${node_keys[$index]}|g" \
        -e "s|@VALIDATOR_INDEX@|$((index + 1))|g" \
        "$(dirname "${BASH_SOURCE[0]}")/ieum-chain-four-node.service.in" \
        >"$unit_path"
    chmod 644 "$unit_path"
done

# 추가 노드도 각 작업 폴더의 실행파일을 사용하므로 과거 공용 복사본은 제거한다.
rm -f /usr/local/bin/ieum-chain

systemctl daemon-reload
for service in "${services[@]}"; do
    systemctl enable "$service"
    systemctl start "$service"
done

echo "한 서버 4노드 구성을 완료했습니다."
for index in 0 1 2 3; do
    echo "${services[$index]}: UDP ${p2p_ports[$index]}, RPC ${rpc_ports[$index]}, PeerId ${peer_ids[$index]}"
done
