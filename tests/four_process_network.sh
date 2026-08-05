#!/usr/bin/env bash
set -euo pipefail

binary="${1:-target/release/ieum-chain}"
binary="$(realpath "$binary")"
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
    --git_action_test
    --validator-index "$index"
    --port "$p2p_port"
    --rpc-port "$rpc_port"
    --rpc-data-dir "$test_root/node-$index/ledger"
    --node-key "$test_root/node-$index/keys/p2p_identity.key"
    --validator-key "$test_root/node-$index/keys/consensus_signing.key"
    --validators-config "$test_root/node-$index/validators.json"
  )

  # 각 노드를 빈 임시 작업 디렉터리에서 실행한다. 저장소의 운영용
  # config/network.json·config/update.json을 읽지 않으므로 CI 테스트가
  # 운영 DNS, 공개 광고 주소 또는 자동 업데이트에 접근하지 않는다.
  mkdir -p "$test_root/node-$index"

  if [[ -n "$peer" ]]; then
    args+=(--peer "$peer")
  fi

  (
    cd "$test_root/node-$index"
    exec "$binary" "${args[@]}"
  ) >"$test_root/node-$index.log" 2>&1 &
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

recipient="0x3252b7b65e50B54508974dB8d634134B0bd6be90"
transfer_value="0x16345785d8a0000" # 0.1 IEUM

for index in 1 2 3 4; do
  port="$((9200 + index))"
  balance_response="$(rpc "$port" eth_getBalance "[\"$faucet\",\"latest\"]")"
  python3 - "$index" "$faucet" "$balance_response" <<'PY'
import json
import sys

index, faucet, raw = sys.argv[1:]
response = json.loads(raw)
if "error" in response:
    raise SystemExit(f"노드 {index} faucet 잔액 조회 실패: {response['error']}")
balance = int(response.get("result", "0x0"), 16)
required = 10**18  # 1 IEUM
if balance < required:
    raise SystemExit(
        f"노드 {index} faucet 잔액 부족: address={faucet}, "
        f"balance={balance}, required={required}"
    )
print(f"노드 {index} faucet 확인: {faucet}, balance={balance} wei")
PY
done

send_response="$(rpc 9202 eth_sendTransaction \
  "[{
    \"from\":\"$faucet\",
    \"to\":\"$recipient\",
    \"value\":\"$transfer_value\",
    \"gas\":\"0x5208\",
    \"gasPrice\":\"0x1\"
  }]")"

transaction_hash="$(python3 - "$send_response" <<'PY'
import json
import sys

response = json.loads(sys.argv[1])
if "error" in response:
    raise SystemExit(f"0.1 IEUM 송금 제출 실패: {response['error']}")
result = response.get("result")
if not isinstance(result, str) or not result.startswith("0x"):
    raise SystemExit(f"송금 해시가 없는 응답: {response}")
print(result)
PY
)"
echo "0.1 IEUM 송금 제출 완료: $transaction_hash"

for _ in $(seq 1 60); do
  heights=()
  roots=()
  recipient_balances=()
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

    if ! balance_response="$(rpc "$port" eth_getBalance "[\"$recipient\",\"latest\"]" 2>/dev/null)"; then
      status_read_failed=true
      break
    fi
    if ! recipient_balance="$(python3 - "$balance_response" <<'PY'
import json
import sys
response = json.loads(sys.argv[1])
if "error" in response:
    raise SystemExit(1)
print(int(response.get("result", "0x0"), 16))
PY
)"; then
      status_read_failed=true
      break
    fi
    recipient_balances+=("$recipient_balance")
  done

  if [[ "$status_read_failed" == false ]] &&
     [[ "${#heights[@]}" -eq 4 ]] &&
     [[ "${heights[0]}" -ge 1 ]] &&
     [[ "${heights[0]}" == "${heights[1]}" ]] &&
     [[ "${heights[1]}" == "${heights[2]}" ]] &&
     [[ "${heights[2]}" == "${heights[3]}" ]] &&
     [[ "${roots[0]}" == "${roots[1]}" ]] &&
     [[ "${roots[1]}" == "${roots[2]}" ]] &&
     [[ "${roots[2]}" == "${roots[3]}" ]] &&
     [[ "${recipient_balances[*]}" == "100000000000000000 100000000000000000 100000000000000000 100000000000000000" ]]; then
    echo "4-process BFT passed: heights=${heights[*]}, stateRoot=${roots[0]}, recipientBalance=${recipient_balances[0]}"
    exit 0
  fi

  sleep 0.5
done

echo "4프로세스 BFT 합의가 제한 시간 안에 완료되지 않았습니다."
dump_logs
exit 1
