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
    --rpc-data-dir "$test_root/node-$index"
    --node-key "$test_root/node-$index.key"
    --validator-key "$test_root/validator-$index.key"
    --validators-config "$test_root/validators.json"
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

start_node 1
for _ in $(seq 1 50); do
  peer_id="$(sed -n 's/^IEUM 서버 노드 시작: //p' "$test_root/node-1.log" | head -1)"
  [[ -n "$peer_id" ]] && break
  sleep 0.2
done
[[ -n "${peer_id:-}" ]] || { cat "$test_root/node-1.log"; exit 1; }
bootstrap="/ip4/127.0.0.1/udp/7201/quic-v1/p2p/$peer_id"

start_node 2 "$bootstrap"
start_node 3 "$bootstrap"
start_node 4 "$bootstrap"
sleep 4

rpc() {
  local port="$1"
  local method="$2"
  local params="$3"
  curl --fail --silent --show-error \
    -H 'content-type: application/json' \
    --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$method\",\"params\":$params}" \
    "http://127.0.0.1:$port"
}

faucet="$(rpc 9202 eth_coinbase '[]' | python3 -c 'import json,sys; print(json.load(sys.stdin)["result"])')"
rpc 9202 eth_sendTransaction \
  "[{\"from\":\"$faucet\",\"to\":\"0x1111111111111111111111111111111111111111\",\"value\":\"0x1\",\"gas\":\"0x1\",\"gasPrice\":\"0x1\"}]" >/dev/null

for _ in $(seq 1 60); do
  heights=()
  roots=()
  for port in 9201 9202 9203 9204; do
    status="$(rpc "$port" ieum_nodeStatus '[]')"
    heights+=("$(python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["height"])' <<<"$status")")
    roots+=("$(python3 -c 'import json,sys; print(json.load(sys.stdin)["result"]["stateRoot"])' <<<"$status")")
  done
  if [[ "${heights[*]}" == "1 1 1 1" && "${roots[0]}" == "${roots[1]}" &&
        "${roots[1]}" == "${roots[2]}" && "${roots[2]}" == "${roots[3]}" ]]; then
    echo "4-process BFT passed: heights=${heights[*]}, stateRoot=${roots[0]}"
    exit 0
  fi
  sleep 0.5
done

for index in 1 2 3 4; do
  echo "===== node $index ====="
  tail -100 "$test_root/node-$index.log"
done
exit 1
