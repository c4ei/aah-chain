# IEUM Chain v0.0.6.4 RPC 8989 및 BFT 테스트 수정

작성일: 2026-07-27

## 요청 사항

- 운영 서버에서 지갑/geth 호환 JSON-RPC를 8989 포트로 서비스
- `tests/four_node_bft.rs` 컴파일 오류 수정
- 변경된 파일만 배포할 수 있는 압축본 제공

## 처리 내용

1. `src/lib.rs`에서 `consensus_runtime` 모듈과 `ConsensusRuntime`을 공개했습니다.
2. `Wallet`은 개인키를 보유하므로 `Clone`을 구현하지 않았습니다.
3. 4노드 테스트는 각 노드 지갑을 고정된 테스트 seed에서 별도로 재생성합니다.
4. JSON-RPC 기본 주소를 `127.0.0.1:8989`로 변경했습니다.
5. Cargo 패키지 버전을 `0.6.4`로 변경하고 `Cargo.lock`을 갱신했습니다.

## 운영 실행

기본 RPC 포트가 8989이므로 다음처럼 실행합니다.

```bash
cargo run --release -- server --port 7001
```

명시적으로 적어도 동일합니다.

```bash
cargo run --release -- server --port 7001 --rpc-host 127.0.0.1 --rpc-port 8989
```

정상 확인:

```bash
curl -sS http://127.0.0.1:8989 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'
```

운영 서버에서는 8989 포트를 인터넷에 직접 개방하지 않습니다. Caddy에서
`127.0.0.1:8989`로 reverse proxy하고 TLS, 접근 제한, rate limit을 적용합니다.

한 서버에서 두 번째 노드를 실행할 때는 P2P와 RPC 포트가 모두 달라야 합니다.

```bash
cargo run --release -- client \
  --port 7002 \
  --rpc-port 8990 \
  --peer /dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/서버PeerId
```

## 검증 명령

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## 변경 파일

- `Cargo.toml`
- `Cargo.lock`
- `CHANGELOG.md`
- `README.md`
- `src/lib.rs`
- `src/main.rs`
- `src/consensus.rs`
- `src/consensus_runtime.rs`
- `tests/four_node_bft.rs`
- `docs/VERSION_0.0.6.4_RPC_8989_BFT_TEST_FIX.md`

# 추가 컴파일 수정 (2026-07-27)

직전 변경분 적용 후 `consensus_runtime.rs`에서 사용하던 합의 API가
`consensus.rs`에 없어 발생한 `E0432`, `E0599` 오류를 수정했다.

- `SignedProposal`과 제안자 Ed25519 서명 검증 추가
- `BftConsensus::round()` 추가
- `BftConsensus::handle_proposal()` 추가
- `BftConsensus::on_timeout()` 추가
- 정족수 달성 직후 늦게 도착한 동일 라운드 투표 허용
- 이미 확정된 블록의 중복 precommit을 안전하게 무시
