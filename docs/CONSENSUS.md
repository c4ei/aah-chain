# BFT 합의 설계

## 현재 흐름

1. stake 가중치 validator set에서 높이와 라운드로 제안자를 순환 선택합니다.
2. 제안자가 후보 블록 해시를 제안합니다.
3. 투표권 2/3 초과의 prevote를 모읍니다.
4. 투표권 2/3 초과의 precommit을 모으면 블록을 확정합니다.

검증자 4명이 같은 지분이면 3명의 precommit이 필요합니다. 2명은 정확히
1/2이므로 확정할 수 없습니다.

## 이번 단계 처리완료

- 검증자 공개키를 validator ID로 사용
- Ed25519로 prevote/precommit 서명
- 수신 투표의 등록 검증자 여부와 서명 검증
- 현재 합의 단계와 다른 투표 거부
- 투표권 2/3 초과에서만 확정
- 동일 검증자의 같은 높이·라운드 이중투표 탐지
- 투표 전 WAL 저장과 디스크 동기화
- 재시작 시 WAL 복구 및 로컬 이중서명 차단

## 다음 단계에서 반드시 추가

- Proposal 서명과 후보 블록 본문의 연결 검증
- chain ID를 포함한 서명 도메인 분리
- 2/3 prevote에서 block lock, 더 높은 round의 proof가 있을 때만 unlock
- 제안자 장애 시 timeout 후 round 증가
- double-sign evidence의 전파와 slashing
- epoch 동안 validator set 고정

현재 `consensus.rs`와 `consensus_wal.rs`는 흐름을 이해하고 단위 테스트하기
위한 코어입니다. 잠금 규칙과 노드 실행 루프 결합이 끝나기 전에 실제 자산을
합의하면 안 됩니다.
