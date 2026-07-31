# IEUM Chain v0.20.7 자동 복구 및 P2P 주소 안정화

## 요청

- `clean`, `init` 또는 키 재생성 후 사용자가 수동으로 PeerId를 맞추지 않아도
  자동으로 정상 기동한다.
- 장애 시 한 명령으로 점검·복구하고, 일반 정리에서는 노드 신원과 검증자 키를
  보존한다.
- 기존 검증자 키의 잔액을 소유권 서명 후 특정 지갑으로 이전한다.
- 외부·운영 노드 로그에서 확인된 오래된 7003 PeerId, DNS 학습 주소, 중첩 릴레이
  주소와 잘못 표시되는 연결 수를 수정한다.
- GitHub Actions 4프로세스 테스트의 노드 상태 공유 문제를 수정한다.

## 처리

### 자동 복구와 안전한 정리

서버 시작 시 `data/server.node.key`의 현재 PeerId와 자기
`config/network.json` 광고 주소를 비교합니다. 서로 다르면 자기 주소만 자동
보정·저장하고 계속 기동합니다. 다른 서버의 bootstrap 설정은 임의로 변경하지
않습니다.

```bash
./ieum-chain node doctor
```

키, 초기화 표시, 원장 폴더, 검증자 설정과 광고 주소를 점검하고 안전하게
복구합니다.

```bash
sudo systemctl stop ieum-chain
sudo ./ieum-chain node clean --yes
sudo systemctl start ieum-chain
```

`node clean`은 원장을 `backups/ledger-clean-*`으로 이동하며 다음 파일을
보존합니다.

- `config/validator.key`
- `data/server.node.key`
- `data/.ieum-initialized`

### 검증자 잔액 이전

```bash
./ieum-chain validator-key transfer \
  --to <받을_IEUM_주소> \
  --amount all
```

공개키만으로는 송금할 수 없고 해당 공개키의 `validator.key` 서명이 반드시
필요합니다.

### P2P 주소 안정화

- 운영 7003 bootstrap 주소를 실제 PeerId
  `12D3KooWNCudCpi43r6bBU2gTyLsm18Vn8G7mxDK4XBpNzmCh7zb`로 갱신했습니다.
- bootstrap뿐 아니라 Identify/Kademlia에서 학습한 `/dns4` 주소도 IPv4로 변환한
  뒤 등록합니다.
- Kademlia가 별도로 관리하는 목적지 PeerId는 주소 끝에서 제거합니다.
- `/p2p-circuit`이 두 번 이상 중첩된 주소나 circuit 뒤에 다른 relay가 이어지는
  주소는 등록하지 않습니다.
- 로컬·가상 인터페이스 주소는 Kademlia에 넣기 전에 제외합니다.

### 연결 수와 전파 로그

한 PeerId와 AutoNAT/QUIC 연결이 여러 개 생겨도 노드 수가 부풀려지지 않도록
RPC 피어 수는 고유 PeerId 수로 기록합니다. 운영 로그에는 다음을 구분합니다.

- 고유 연결 피어
- 해당 PeerId와 맺은 QUIC 연결

피어가 Gossipsub 토픽에 가입하기 전의 `NoPeersSubscribedToTopic`은 통신 장애
오류가 아니라 `[P2P 전파 대기]` 상태로 반복 집계합니다.

### GitHub Actions

4프로세스 테스트의 원장, `validator.key`, `server.node.key`와 초기화 표시를
각 노드 폴더 아래로 분리해 다른 노드의 설치 상태를 공유하지 않게 했습니다.

## 검증 명령

```bash
cargo fmt --all
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
bash tests/four_process_network.sh target/release/ieum-chain
```
