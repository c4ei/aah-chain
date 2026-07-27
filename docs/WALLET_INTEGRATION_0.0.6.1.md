# 지갑 연동 보강 v0.0.6.1

## 이번 버전에서 수정

- 제네시스 잔액뿐 아니라 거래의 `amount`, `fee`도 `u128`로 통일
- EIP-155 legacy 거래의 `value`를 16바이트까지 해석
- `gasPrice × gasLimit`을 `u128`로 안전하게 계산
- 노드 관리 계정의 `eth_sendTransaction`도 gas limit을 포함해 수수료 계산

이 변경으로 기존 `u64` 최대값 때문에 약 18.44 IEUM를 넘는 거래가 거부되던 제한이 제거된다.

## 호환성 주의

`Transaction` JSON의 숫자는 JSON number가 아니라 Rust 직렬화 규격을 그대로 사용 중이다.
기존 `v0.0.5.2` 블록 파일의 작은 정수는 `u128`로 역직렬화할 수 있지만, 운영 데이터로
전환하기 전에 복사본에서 체크포인트 복원 시험을 해야 한다.

## 아직 남은 중요 작업

- RPC 제출 경로와 4노드 BFT 확정 경로의 단일화
- pending nonce 계산과 같은 계정의 연속 거래 예약
- 동일 raw transaction 재제출의 멱등 응답
- locked/valid value, 체크포인트 정족수 서명
- 분할망과 장시간 부하 시험

## 테스트

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

지갑에서는 제네시스 잔액이 충분한 테스트 계정으로 20 IEUM 이상을 전송하고,
`eth_getTransactionReceipt`와 송수신 잔액을 함께 확인한다.
