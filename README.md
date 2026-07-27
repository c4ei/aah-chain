# IEUM Chain

## 노드와 지갑 RPC 실행

```bash
cargo run -- --port 7001
```

- `--port`: 노드 간 QUIC/P2P UDP 포트
- `--rpc-port`: 월렛/geth 호환 HTTP JSON-RPC TCP 포트(기본 `8989`)
- RPC 기본 리스닝 주소는 외부에서 접근할 수 없는 `127.0.0.1`입니다.
- 운영 서버에서 Caddy는 `127.0.0.1:8989`로 reverse proxy하고, RPC 포트를 인터넷에
  직접 열지 않습니다.
- 두 번째 노드는 `--port 7002 --rpc-port 8990`처럼 두 포트를 모두 다르게 지정합니다.

> 사람과 사람, 체인과 체인, 가치와 생활을 잇는 가벼운 블록체인

- 네트워크 이름: `IEUM`
- 네이티브 코인 심볼: `IEUM`
- EVM 호환 Chain ID: `21004`
- 프로젝트/실행 파일: `ieum-chain`
- 기존 `IEUM` geth 네트워크(Chain ID `21133`)와는 별개의 체인입니다.

IEUM Chain을 배우며 확장하는 Rust 기반 경량 블록체인 테스트넷입니다.
현재 버전은 `0.6.4`이며, 기존 Step 1~5 기능에 서명된 BFT 투표와 합의 WAL 코어를 추가했습니다.

> 주의: 학습·사설 테스트넷용 코드입니다. 실제 자산을 맡기는 메인넷에 사용하지 마세요.

## 현재 구현

- Ed25519 지갑, 주소, 서명 거래
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

상세 진행표는 [docs/ROADMAP.md](docs/ROADMAP.md)를 참고하세요.
전체 목표 구조는 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)에 정리했습니다.
간헐 접속 클라이언트 설계는 [docs/CLIENT.md](docs/CLIENT.md)를 참고하세요.
버전별 변경 내용은 [CHANGELOG.md](CHANGELOG.md)에 기록합니다.

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
ieum-chain/
├── .vscode/              VS Code 공통 설정
├── config/               노드·검증자 설정 예제
├── docs/                 설계, 진행상태, 운영·보안 문서
├── src/
│   ├── chain.rs          블록체인 상태와 검증
│   ├── consensus.rs      PoS 가중 BFT 상태기계
│   ├── consensus_wal.rs  합의 투표 기록·복구와 이중서명 방지
│   ├── network.rs        QUIC/libp2p와 피어 검색
│   ├── peer_guard.rs     악성 피어 점수·차단
│   ├── mempool.rs        거래 대기소
│   ├── model.rs          거래·블록 모델
│   ├── storage.rs        로컬 저장·복구
│   ├── checkpoint.rs     빈 구간 체크포인트
│   ├── wallet.rs         키·주소·서명
│   ├── lib.rs            재사용 가능한 코어 공개
│   └── main.rs           노드 실행 파일
├── tests/                통합 테스트(다음 단계에서 확대)
├── CHANGELOG.md           버전별 변경 기록
├── Cargo.toml            Rust 패키지와 의존성
└── README.md
```

프로젝트가 커지면 `crates/ieum-network`, `crates/ieum-consensus`,
`crates/ieum-ledger`, `apps/ieum-node`의 Cargo workspace로 분리합니다.
지금은 배우기 쉽도록 한 crate 안에서 모듈만 나눴습니다.
