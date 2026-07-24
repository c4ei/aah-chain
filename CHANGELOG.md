# 변경 기록

## 0.0.3 - 2026-07-24

### 추가

- geth/web3 호출용 HTTP JSON-RPC 서버를 추가했습니다.
- `web3_clientVersion`, `net_version`, `eth_chainId`, `eth_syncing`,
  `eth_blockNumber`, `eth_accounts`, `eth_coinbase`,
  `personal_newAccount`, `personal_unlockAccount`, `eth_getBalance`,
  `eth_getTransactionCount`, `eth_sendTransaction`, 보조적인 network/gas
  조회 메서드를 구현했습니다.
- 주소 생성·잔액 조회·송금 자동 테스트와 사용 문서를 추가했습니다.

### 수정

- Rust 1.97의 `clippy -D warnings`에서 실패하던 중첩 `if`를 정리했습니다.
- 큰 `identify::Event`를 `Box`로 감싸 `large_enum_variant` 오류를 수정했습니다.
- Kademlia 이벤트 값을 명시적으로 소비해 미사용 variant field 경고를 수정했습니다.

### 제한

- `eth_sendRawTransaction`은 명확한 미지원 오류를 반환합니다.
- RPC의 20바이트 주소는 기존 Ed25519 원장 주소에 대한 호환 별칭입니다.
- 개발용 계정은 아직 메모리 기반이며 암호화 keystore는 다음 단계입니다.

## 0.0.2 - 2026-07-24

### 수정

- `src/lib.rs`의 위조 서명 테스트에서 한글을 ASCII 전용 byte string
  (`b"..."`)에 넣어 컴파일되지 않던 문제를 수정했습니다.
- `"위조 서명".as_bytes()`를 사용해 UTF-8 바이트를 서명 함수에 전달합니다.

### 문서

- 모바일·웹 지갑이 필요할 때만 접속하고 작업 후 연결을 끊는 라이트
  클라이언트 구조를 `docs/CLIENT.md`에 추가했습니다.
- README, 아키텍처와 로드맵에 간헐 접속 클라이언트 계획을 반영했습니다.

## 0.0.1

- 서명된 BFT 투표와 검증을 추가했습니다.
- 합의 WAL 저장·복구와 로컬 이중서명 방지를 추가했습니다.
- 주요 소스에 한국어 학습 주석을 보강했습니다.
