# aah-chain

AAH Chain을 배우며 확장하는 Rust 기반 경량 블록체인 테스트넷입니다.
현재 배포 버전은 `0.0.3`이며, geth/web3 계정·잔액·송금 스크립트를 위한
HTTP JSON-RPC 호환 계층을 추가했습니다.

> 주의: 학습·사설 테스트넷용 코드입니다. 실제 자산을 맡기는 메인넷에 사용하지 마세요.

## 현재 구현

- 사용자 계정: geth 호환 secp256k1 키와 Ethereum 20바이트 주소
- seed 지갑: BIP-39 및 `m/44'/60'/0'/0/n` HD 파생
- 합의 검증자·P2P: 역할을 분리한 기존 Ed25519 키
- 잔액, nonce, 수수료, mempool
- 블록 생성·검증, JSON 저장·복구, 체크포인트
- 거래가 없을 때 빈 블록 생략
- QUIC 기반 암호화 P2P
- mDNS(같은 LAN)와 Kademlia DHT(외부망) 피어 검색
- Gossipsub 다중 피어 메시지 전파
- 메시지 크기 제한, idle timeout, 잘못된 메시지 점수와 임시 차단
- stake 가중치 2/3 초과 prevote/precommit BFT 상태기계
- 합의 투표 Ed25519 서명 검증
- 합의 WAL 저장·복구와 로컬 이중서명 방지
- 주요 소스의 한국어 학습 주석
- 필요할 때만 접속하는 모바일·웹 클라이언트 구조 문서
- geth 호환 JSON-RPC: 계정 생성·개인키/seed 가져오기, 잔액, nonce, 관리형 계정 송금

상세 진행표는 [docs/ROADMAP.md](docs/ROADMAP.md)를 참고하세요.
전체 목표 구조는 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)에 정리했습니다.
간헐 접속 클라이언트 설계는 [docs/CLIENT.md](docs/CLIENT.md)를 참고하세요.
버전별 변경 내용은 [CHANGELOG.md](CHANGELOG.md)에 기록합니다.
geth 호환 범위와 예제는 [docs/GETH_COMPAT.md](docs/GETH_COMPAT.md)를 참고하세요.

## Ubuntu 설치

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup update stable
```

VS Code 확장은 `rust-analyzer`를 설치하면 됩니다.

## 빌드와 테스트

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- --port 7001
```

실행 후 P2P는 UDP `7001`, geth 호환 HTTP JSON-RPC는 로컬 TCP `8545`에서
대기합니다. 외부 공개가 꼭 필요한 경우가 아니면 `--rpc-addr 127.0.0.1`을
유지하세요.

```bash
curl -s http://127.0.0.1:8545 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_accounts","params":[]}'
```

같은 공유기 안에서 두 번째 터미널:

```bash
cargo run -- --port 7002
```

mDNS가 두 노드를 자동으로 찾습니다. 서버가 서로 다른 네트워크라면 첫 노드 출력의
`/ip4/.../udp/7001/quic-v1/p2p/...` 주소를 사용합니다.

```bash
cargo run -- --port 7002 \
  --peer /ip4/서버IP/udp/7001/quic-v1/p2p/첫노드PeerId
```

방화벽에서는 해당 포트의 **UDP**를 열어야 합니다. TCP 7001만 열면 QUIC가 연결되지 않습니다.

## 표준화한 폴더 구조

```text
aah-chain/
├── .vscode/              VS Code 공통 설정
├── config/               노드·검증자 설정 예제
├── docs/                 설계, 진행상태, 운영·보안 문서
├── src/
│   ├── chain.rs          블록체인 상태와 검증
│   ├── consensus.rs      PoS 가중 BFT 상태기계
│   ├── consensus_wal.rs  합의 투표 기록·복구와 이중서명 방지
│   ├── network.rs        QUIC/libp2p와 피어 검색
│   ├── peer_guard.rs     악성 피어 점수·차단
│   ├── rpc.rs            geth 호환 HTTP JSON-RPC 어댑터
│   ├── account.rs        secp256k1, Ethereum 주소, BIP-39/44 사용자 계정
│   ├── mempool.rs        거래 대기소
│   ├── model.rs          거래·블록 모델
│   ├── storage.rs        로컬 저장·복구
│   ├── checkpoint.rs     빈 구간 체크포인트
│   ├── wallet.rs         Ed25519 합의·기존 테스트 지갑
│   ├── lib.rs            재사용 가능한 코어 공개
│   └── main.rs           노드 실행 파일
├── tests/                통합 테스트(다음 단계에서 확대)
├── CHANGELOG.md           버전별 변경 기록
├── Cargo.toml            Rust 패키지와 의존성
└── README.md
```

프로젝트가 커지면 `crates/aah-network`, `crates/aah-consensus`,
`crates/aah-ledger`, `apps/aah-node`의 Cargo workspace로 분리합니다.
지금은 배우기 쉽도록 한 crate 안에서 모듈만 나눴습니다.
