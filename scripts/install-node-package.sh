#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
    echo "sudo ./install.sh 로 실행하세요." >&2
    exit 1
fi

package_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
install_dir="${IEUM_INSTALL_DIR:-/opt/ieum-chain}"
service_name="${IEUM_SERVICE_NAME:-ieum-chain.service}"
run_user="${IEUM_RUN_USER:-${SUDO_USER:-}}"

if [[ -z "$run_user" || "$run_user" == "root" ]]; then
    echo "실행 계정을 찾을 수 없습니다. IEUM_RUN_USER=사용자 를 지정하세요." >&2
    exit 1
fi
id "$run_user" >/dev/null 2>&1 || {
    echo "실행 계정이 없습니다: $run_user" >&2
    exit 1
}

run_group="$(id -gn "$run_user")"
binary_src="$package_dir/ieum-chain"
[[ -x "$binary_src" ]] || {
    echo "배포본에 ieum-chain 실행파일이 없습니다." >&2
    exit 1
}

install -d -m 750 -o "$run_user" -g "$run_group" \
    "$install_dir" "$install_dir/config" "$install_dir/data"

if [[ -x "$install_dir/ieum-chain" ]]; then
    cp --preserve=mode,ownership,timestamps \
        "$install_dir/ieum-chain" "$install_dir/ieum-chain.previous"
fi
install -m 755 -o "$run_user" -g "$run_group" \
    "$binary_src" "$install_dir/ieum-chain"

# 공개 네트워크 설정만 최초 설치 때 복사한다. 서버 고유 키와 원장은 건드리지 않는다.
for config_name in genesis.json events.json upgrades.json bootstrap.json; do
    config_src="$package_dir/config/$config_name"
    config_dst="$install_dir/config/$config_name"
    if [[ -f "$config_src" && ! -e "$config_dst" ]]; then
        install -m 640 -o "$run_user" -g "$run_group" "$config_src" "$config_dst"
    fi
done
if [[ -f "$package_dir/config/update.json" ]]; then
    install -m 640 -o "$run_user" -g "$run_group" \
        "$package_dir/config/update.json" \
        "$install_dir/config/update.json"
fi

unit_path="/etc/systemd/system/$service_name"
sed \
    -e "s|@RUN_USER@|$run_user|g" \
    -e "s|@RUN_GROUP@|$run_group|g" \
    -e "s|@INSTALL_DIR@|$install_dir|g" \
    "$package_dir/ieum-chain.service.in" >"$unit_path"
chmod 644 "$unit_path"

systemctl daemon-reload
systemctl enable "$service_name"
systemctl restart "$service_name"

healthy=0
for _ in $(seq 1 30); do
    if curl --fail --silent \
        -H 'content-type: application/json' \
        --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
        http://127.0.0.1:8989 | grep -q '"result"'; then
        healthy=1
        break
    fi
    sleep 2
done

if [[ "$healthy" -ne 1 ]]; then
    echo "서비스는 설치됐지만 60초 안에 RPC 상태 확인에 실패했습니다." >&2
    echo "확인: sudo journalctl -u $service_name -n 100 --no-pager" >&2
    exit 1
fi

echo "IEUM 노드 설치 및 자동 실행 완료"
echo "설치 경로: $install_dir"
echo "서비스: $service_name"
echo "로그 확인: sudo journalctl -u $service_name -f"
