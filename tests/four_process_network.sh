#!/usr/bin/env bash
set -euo pipefail

binary="${1:-target/release/ieum-chain}"
test_root="$(mktemp -d)"
pids=()

cleanup() {
  for pid in "${pids[@]:-}"; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$test_root"
}
trap cleanup EXIT

dump_logs() {
  for index in 1 2 3 4; do
    echo "===== node $index ====="

    if [[ -f "$test_root/node-$index.log" ]]; then
      tail -100 "$test_root/node-$index.log"
    else
      echo "로그 파일 없음"
    fi
  done
}

start_node() {
  local index="$1"
  local peer="${2:-}"
  local p2p_port="$((7200 + index))"
  local rpc_port="$((9200 + index))"
  local args=(
    server
    --validator-index "$index"
    --allow-insecure-test-keys
    --port "$p2p_port"
    --rpc-port "$rpc_port"
    --rpc-data-dir "$test_root/node-$index/ledger"
    --node-key "$test_root/node-$index/server.node.key"
    --validator-key "$test_root/node-$index/validator.key"
    --validators-config "$test_root/node-$index/validators.json"
    --no-default-bootstrap
    --propose-timeout-ms 1500
    --prevote-timeout-ms 1500
    --precommit-timeout-ms 1500
  )

  if [[ -n "$peer" ]]; then
    args+=(--peer "$peer")
  fi

  "$binary" "${args[@]}" >"$test_root/node-$index.log" 2>&1 &
  pids+=("$!")
}

rpc() {
  local port="$1"
  local method="$2"
  local params="$3"

  curl --fail --silent --show-error \
    --connect-timeout 2 \
    --max-time 5 \
    -H 'content-type: application/json' \
    --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\",\"params\":$params}" \
    "http://127.0.0.1:$port"
}

wait_for_rpc() {
  local index="$1"
  local port="$((9200 + index))"
  local pid="${pids[$((index - 1))]}"

  for _ in $(seq 1 120); do
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "노드 $index 프로세스가 RPC 준비 전에 종료됐습니다."
      dump_logs
      return 1
    fi

    if rpc "$port" ieum_nodeStatus '[]' >/dev/null 2>&1; then
      echo "노드 $index RPC 준비 완료: 127.0.0.1:$port"
      return 0
    fi

    sleep 0.5
  done

  echo "노드 $index RPC가 60초 안에 준비되지 않았습니다: 127.0.0.1:$port"
  dump_logs
  return 1
}

start_node 1

peer_id=""
for _ in $(seq 1 50); do
  if ! kill -0 "${pids[0]}" 2>/dev/null; then
    echo "노드 1이 PeerId 생성 전에 종료됐습니다."
    dump_logs
    exit 1
  fi

  peer_id="$(
    sed -n 's/^IEUM 서버 노드 시작: //p' \
      "$test_root/node-1.log" |
      head -1
  )"

  if [[ -n "$peer_id" ]]; then
    break
  fi

  sleep 0.2
done

if [[ -z "$peer_id" ]]; then
  echo "노드 1의 PeerId를 확인하지 못했습니다."
  dump_logs
  exit 1
fi

bootstrap="/ip4/127.0.0.1/udp/7201/quic-v1/p2p/$peer_id"

start_node 2 "$bootstrap"
start_node 3 "$bootstrap"
start_node 4 "$bootstrap"

for index in 1 2 3 4; do
  wait_for_rpc "$index"
done

faucet_response="$(rpc 9202 eth_coinbase '[]')"

faucet="$(
  python3 -c '
import json
import sys

response = json.load(sys.stdin)
result = response.get("result")

if not isinstance(result, str) or not result:
    raise SystemExit("eth_coinbase 응답에 유효한 result가 없습니다.")

print(result)
' <<<"$faucet_response"
)"

rpc 9202 eth_sendTransaction \
  "[{
    \"from\":\"$faucet\",
    \"to\":\"0x1111111111111111111111111111111111111111\",
    \"value\":\"0x1\",
    \"gas\":\"0x1\",
    \"gasPrice\":\"0x1\"
  }]" >/dev/null

for _ in $(seq 1 60); do
  heights=()
  roots=()
  status_read_failed=false

  for index in 1 2 3 4; do
    port="$((9200 + index))"
    pid="${pids[$((index - 1))]}"

    if ! kill -0 "$pid" 2>/dev/null; then
      echo "합의 대기 중 노드 $index 프로세스가 종료됐습니다."
      dump_logs
      exit 1
    fi

    if ! status="$(rpc "$port" ieum_nodeStatus '[]' 2>/dev/null)"; then
      status_read_failed=true
      break
    fi

    if ! parsed="$(
      python3 -c '
import json
import sys

response = json.load(sys.stdin)
result = response["result"]
print(result["height"])
print(result["stateRoot"])
' <<<"$status" 2>/dev/null
    )"; then
      status_read_failed=true
      break
    fi

    height="$(sed -n '1p' <<<"$parsed")"
    state_root="$(sed -n '2p' <<<"$parsed")"

    heights+=("$height")
    roots+=("$state_root")
  done

  if [[ "$status_read_failed" == false ]] &&
     [[ "${#heights[@]}" -eq 4 ]] &&
     [[ "${heights[*]}" == "1 1 1 1" ]] &&
     [[ "${roots[0]}" == "${roots[1]}" ]] &&
     [[ "${roots[1]}" == "${roots[2]}" ]] &&
     [[ "${roots[2]}" == "${roots[3]}" ]]; then
    echo "4-process BFT passed: heights=${heights[*]}, stateRoot=${roots[0]}"
    exit 0
  fi

  sleep 0.5
done

echo "4프로세스 BFT 합의가 제한 시간 안에 완료되지 않았습니다."
dump_logs
exit 1
