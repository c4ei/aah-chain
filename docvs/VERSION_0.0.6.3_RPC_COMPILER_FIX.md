# IEUM Chain v0.0.6.3 RPC 컴파일 수정

기준 소스: `ieum_chain_v0_0_6_3_test`

## 수정 이유

RPC 모듈은 `u128` 금액과 체크포인트·블록 조회 기능을 요구하지만 기존 원장 핵심 자료형은
여전히 `u64`이고 필요한 `Blockchain` API가 없어 컴파일 오류가 발생했습니다.

## 변경 내용

- `Transaction.amount`, `Transaction.fee`, 잔액을 `u128`로 통일
- `1 IEUM = 10^18` 최소 단위와 80,000 IEUM 제네시스 배분 지원
- `Blockchain.chain_id`, `genesis_commitment` 추가
- 제네시스·체크포인트 생성/복원 구현
- tip, 블록, 거래, 잔액, nonce 조회 API 구현
- Ed25519 내부 거래, secp256k1 계정 거래, EIP-155 raw 거래 검증 경로 분리
- 잔액·수수료 덧셈 overflow 검사
- RPC 블록 저장 시 빌림 충돌 방지를 위해 확정 후 체인 상태를 별도 복제

## 적용 및 확인

이 압축의 파일을 `ieum-chain` 프로젝트 루트에 덮어쓴 후 실행합니다.

```bash
cargo clean
cargo test
cargo run -- --port 7001 --rpc-port 8545
```

RPC 확인:

```bash
curl -s http://127.0.0.1:8545 \
  -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
```

예상 `result`는 Chain ID 21004의 16진수인 `0x520c`입니다.

## 주의

이 변경분은 첨부된 `ieum_chain_v0_0_6_3_test`에 덮어쓰는 용도입니다.
이전 버전에 바로 적용하면 RPC 의존 파일이 빠질 수 있습니다.
