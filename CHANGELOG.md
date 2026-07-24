# 변경 기록

## 0.0.6-1 - 2026-07-24

- 거래 금액과 수수료를 `u128`로 확장해 18.44 AAH 초과 송금 지원
- legacy raw transaction의 value를 최대 16바이트로 해석
- `eth_sendTransaction`의 gasPrice × gas를 실제 수수료로 계산
- 20 AAH 이상 raw transaction 회귀 테스트 추가
- 지갑 연동 RPC·nonce·수수료·합의 보강 항목 문서화

## 0.0.5-1 - 2026-07-24

- 영구 libp2p node key와 재시작 후 고정 PeerId
- `config/bootstrap.json` bootstrap 목록
- 서명된 proposal 검증, timeout/round change, 확정 후 저장 BFT 실행 코어
- 신규 노드 확정 블록 배치 동기화
- 4검증자 합의·미확정 미저장·timeout·신규 노드 sync 통합 테스트
- 제네시스 AAH 총량과 개인키 보유 계좌 이전 문서

## 0.0.4-1 - 2026-07-24

### 수정

- `src/storage.rs`의 `load()` 함수를 테스트 모듈 앞으로 이동해 Rust 1.97의
  `clippy::items-after-test-module` 오류를 제거했습니다.

## 0.0.4 - 2026-07-24

### 추가

- `GenesisConfig` 검증과 결정론적 제네시스 설정 해시를 추가했습니다.
- BFT 참여 목표, 월별 보상 곡선, 검증자 가중투표와 2/3 초과 확정 로직을
  추가했습니다.
- EIP-155 legacy RLP 송금용 `eth_sendRawTransaction`을 추가했습니다.
- 1MiB 개별 블록 제한과 최대 100MB JSONL 블록 세그먼트 회전을 추가했습니다.

### 제한

- raw transaction은 legacy EIP-155 단순 송금만 지원합니다. EIP-1559,
  컨트랙트 생성, calldata/EVM 실행은 아직 지원하지 않습니다.
- 월별 정책 모듈의 실제 보상 발행, 참여 증명, 영구 저장과 거버넌스 RPC는
  후속 작업입니다.

## 0.0.3 - 2026-07-24

### 추가

- geth/web3 호출용 HTTP JSON-RPC 서버를 추가했습니다.
- `web3_clientVersion`, `net_version`, `eth_chainId`, `eth_syncing`,
  `eth_blockNumber`, `eth_accounts`, `eth_coinbase`,
  `personal_newAccount`, `personal_unlockAccount`, `eth_getBalance`,
  `eth_getTransactionCount`, `eth_sendTransaction`, 보조적인 network/gas
  조회 메서드를 구현했습니다.
- 주소 생성·잔액 조회·송금 자동 테스트와 사용 문서를 추가했습니다.
- 사용자 계정을 secp256k1로 전환하고 Ethereum 표준 20바이트 주소 계산을
  적용했습니다. 알려진 geth 주소 벡터 테스트를 추가했습니다.
- BIP-39 seed와 `m/44'/60'/0'/0/n` BIP-44 파생을 추가했습니다.
- `personal_importRawKey`, `aah_newMnemonic`, `aah_importMnemonic`을 추가했습니다.
- 합의 검증자/P2P Ed25519 키와 사용자 자산 계정 키의 역할을 분리했습니다.

### 수정

- Rust 1.97의 `clippy -D warnings`에서 실패하던 중첩 `if`를 정리했습니다.
- 큰 `identify::Event`를 `Box`로 감싸 `large_enum_variant` 오류를 수정했습니다.
- Kademlia 이벤트 값을 명시적으로 소비해 미사용 variant field 경고를 수정했습니다.
- 사용자 제공 `cargo fmt --check` 로그에 나온 전체 포맷 차이를 반영했습니다.
- RPC 주소 해석 시 `AccountWallet` 값에 잘못 지정된 `Wallet::address`
  메서드 참조를 `AccountWallet::address`로 수정했습니다.

### 제한

- `eth_sendRawTransaction`은 명확한 미지원 오류를 반환합니다.
- 사용자 주소와 seed는 geth/MetaMask 방식과 호환되지만 AAH 내부 거래는 아직
  Ethereum RLP raw transaction이 아닙니다.
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
# 0.0.5-2

- `max_segment_bytes`를 전체 활성 블록 합계인 `max_active_block_bytes`로 명확화
- 월별 상태 체크포인트, 활성/백업 분리, 100MB 활성 상한 추가
- 최근 월별·전전년도 연별 백업 병합 API 추가
- 모바일의 선택형 백업과 Explorer 서버의 전체 백업 운용 문서화
- Explorer용 블록·거래·영수증·저장 상태 JSON-RPC 추가
- 제네시스 초기 잔액을 u128로 확장해 10,000 AAH 이상 배분 지원
