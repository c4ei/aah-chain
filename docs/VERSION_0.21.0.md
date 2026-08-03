# IEUM Chain v0.21.0 운영 합류·월렛 RPC

## 요청 사항

- 신규 노드가 운영망에 안전하게 합류할 수 있어야 한다.
- 월렛이 운영망 신원과 동기화·확정 상태를 확인해야 한다.
- 거래 단위 복구와 체크포인트 롤백의 승인 강도를 분리한다.

## 처리 내용

- 기존 snapshot sync의 manifest, chunk hash, state root, tip quorum 검증을 유지한다.
- `ieum_networkIdentity`, `ieum_protocolVersion`, `ieum_syncStatus`,
  `ieum_finalizedBlock`, `ieum_recoveryStatus`,
  `ieum_getRecoveryByTransaction` RPC를 추가했다.
- 거래 단위 복구는 검증자 수 또는 투표권 3/4 승인을 허용한다.
- 체크포인트 롤백은 두 기준 모두 3/4를 요구한다.
- 원본 거래는 삭제하지 않고 `data/ledger/recovery/records.json` 감사 기록과 연결한다.

## 복구 기록 예시

```json
[{"incidentId":"INCIDENT-2026-001","transactionHash":"0x64자리_해시","recoveryTransactionHash":"0x64자리_해시","status":"applied","approvalBasis":"validator_count","approvedAt":1785720000}]
```

RPC는 이 파일을 읽기만 한다. 다중서명 복구 적용기가 검증 후 원자적으로 기록해야 한다.

## 운영 검사

```bash
cargo fmt --all --check &&
cargo clippy --all-targets --all-features --locked -- -D warnings &&
cargo test --all-targets --all-features --locked &&
cargo build --release --locked
```
