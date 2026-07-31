# IEUM Chain v0.20.8 쉬운 실행 안내

설치, systemd, 백업, 자동 업데이트 등 전체 운영 방법은
[`docs/USER_MANUAL_0.20.8.md`](docs/USER_MANUAL_0.20.8.md)에 정리되어 있으며,
v0.20.8 검증자 설정 안전 저장과 CI 격리 내용은
[`docs/VERSION_0.20.8.md`](docs/VERSION_0.20.8.md)를 참고하세요.

## Ubuntu 신규 노드 배포본 자동 생성

릴리스 빌드 후 설치 가능한 노드 압축을 한 번에 만듭니다.

```bash
scripts/make-node-package.sh 0.20.8
```

위 명령은 포맷·Clippy·전체 테스트·릴리스 빌드까지 통과한 경우에만 압축을
생성합니다.

대상 서버에 직접 복사한 경우:

```bash
tar -xJf ieum-chain_node_ubuntu_x86_64_v0.20.8.tar.xz
cd ieum-chain-node-v0.20.8
sudo ./install.sh
```

SSH 키와 sudo 권한이 준비된 서버에는 자동 업로드·설치할 수 있습니다.

```bash
scripts/deploy-node-package.sh \
  download/ieum-chain_node_ubuntu_x86_64_v0.20.8.tar.xz \
  dev@192.168.1.148
```

설치기는 `/opt/ieum-chain`에 systemd 서비스를 등록해 부팅 시 자동 실행합니다.
기존 `config/validator.key`, `data/server.node.key`, 원장과 로그는 보존하며,
배포 압축에는 개인키·원장·백업을 포함하지 않습니다.

IEUM Chain은 Ubuntu와 Windows에서 실행할 수 있는 경량 블록체인 노드입니다.
첫 노드는 제네시스 상태에서 단독으로 시작할 수 있습니다. 장애 허용 BFT 검증은
**Ubuntu VM 검증자 3대 + Windows VM 검증자 1대**처럼 4명 이상일 때 활성화합니다.

> 아직 사설 테스트넷 단계입니다. 실제 자산이나 중요한 개인정보를 보관하지 마세요.

## 가장 먼저 알아둘 것

- VM 4대는 서로 다른 고정 IP를 사용합니다.
- P2P는 `UDP 7001`, RPC는 기본적으로 자기 PC만 접근 가능한 `127.0.0.1:8989`를 사용합니다.
- 네 서버의 `validator.key`, `server.node.key`, `data/ledger`는 서로 달라야 합니다.
- 처음 실행하면 위 파일과 폴더를 각 서버에서 자동 생성합니다.
- `config/validators.json`이 없으면 번들 제네시스의 검증자 4개가 자동 복원됩니다.
- 신규 서버의 로컬 키가 현재 검증자 집합에 없으면 일반 동기화 노드로 시작합니다.
- 신규 서버는 P2P 연결 뒤 공개키·PeerId 소유권 서명을 후보 등록 메시지로 자동 전송합니다.
- 후보는 현재 검증자 승인과 다음 epoch 반영 전까지 합의 투표를 하지 않습니다.
- 개인키 파일은 메일, 메신저, 클라우드 드라이브로 보내지 마세요.

`--allow-insecure-test-keys`를 사용하는 개발망과 CI는 검증자 설정 4개를 자동
생성하므로 `validators.json`을 사람이 만들 필요가 없습니다. 운영망에서는 이 옵션을
절대 사용하지 않습니다.

## 운영 검증자 자동 선발 기준

운영 후보는 다음 자격 중 하나를 충족해야 합니다.

- 확정 유통량의 1% 이상을 보유하거나 온체인으로 위임받음
- 국가가 인증된 계좌 중 해당 국가의 IEUM 보유량 상위 50위
- 관리자 또는 거버넌스의 명시적 승인

세 경로 모두 계좌 소유권 서명, `node.ieum.aah.name` 등록 창구의 실제 UDP/QUIC
접속 확인, 기존 노드의 95% 이상 가동률을 통과해야 합니다. 차단·이중서명 후보는
지분과 승인 여부와 관계없이 제외합니다.

`ping node.ieum.aah.name` 성공은 DNS와 ICMP 연결만 확인하며 검증 노드 자격은
증명하지 못합니다. 최종 자동 등록은 노드가 부트스트랩 서버에 접속한 뒤 임의 nonce에
서명하고, 서버가 후보의 UDP/QUIC 주소로 역접속하는 challenge 방식으로 진행합니다.
v0.19.6은 신규 노드의 P2P 동기화와 서명 후보 등록을 자동화합니다. 운영망 후보는
P2P 접속만으로 합의권을 얻지 않으며 기존 선발 정책과 epoch 변경 절차를 통과해야
합니다.

## Ubuntu에서 실행

처음 한 번만 설치합니다.

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

소스를 받은 폴더에서 빌드하고 실행합니다.

```bash
cargo build --release --locked
./target/release/ieum-chain server
```

처음 실행하면 자동으로 다음 항목을 만듭니다.

```text
config/validator.key
data/server.node.key
data/ledger/
config/validators.json
config/events.json
data/.ieum-initialized
```

화면에 표시되는 **검증자 공개키**만 기록합니다. 제네시스 검증자 키를 가진 서버는
합의에 참여하고, 그 외 신규 서버는 원장을 동기화하면서 서명된 후보 등록을 자동으로
전송합니다.

## Windows에서 실행

미리 빌드된 `ieum-chain.exe`가 있으면 원하는 폴더에 아래처럼 둡니다.

```text
C:\ieum-chain\ieum-chain.exe
```

PowerShell에서 실행합니다.

```powershell
cd C:\ieum-chain
.\ieum-chain.exe server
```

직접 빌드하려면 Rustup, Git for Windows, Visual Studio 2022 Build Tools의
`Desktop development with C++`를 설치한 뒤 실행합니다.

```powershell
git clone https://github.com/c4ei/ieum-chain.git
cd ieum-chain
cargo build --release --locked
.\target\release\ieum-chain.exe server
```

관리자 PowerShell에서 P2P 방화벽을 한 번 허용합니다.

```powershell
New-NetFirewallRule -DisplayName "IEUM P2P UDP 7001" -Direction Inbound -Protocol UDP -LocalPort 7001 -Action Allow
```

## 네 검증자 등록

네 서버를 각각 한 번 실행해 공개키 4개를 모읍니다. Ubuntu 1번에서 공통 설정을 만듭니다.

```bash
./target/release/ieum-chain validator-key create-config \
  --public-key <Ubuntu1_공개키> \
  --public-key <Ubuntu2_공개키> \
  --public-key <Ubuntu3_공개키> \
  --public-key <Windows_공개키> \
  --output config/validators.json
```

생성된 `config/validators.json`을 나머지 세 서버의 같은 위치에 복사합니다.
공개키를 넣은 순서가 서버 번호입니다.

```text
Ubuntu 1 = validator-index 1
Ubuntu 2 = validator-index 2
Ubuntu 3 = validator-index 3
Windows  = validator-index 4
```

각 서버는 자기 번호로 실행합니다.

```bash
./target/release/ieum-chain server --validator-index 1
```

```powershell
.\ieum-chain.exe server --validator-index 4
```

다른 네트워크의 서버에 연결할 때만 1번 서버의 IP와 PeerId를 넣습니다.

```bash
./target/release/ieum-chain server --validator-index 2 \
  --peer "/ip4/192.168.1.201/udp/7001/quic-v1/p2p/<1번_PeerId>"
```

## 키나 원장이 없어졌다는 오류가 나오면

처음 설치가 끝난 뒤 `validator.key`, `server.node.key`, `data/ledger`가 없어지면
프로그램은 새 파일을 만들지 않고 중단합니다. 자동으로 다시 만들면 다른 검증자로
바뀌기 때문입니다. 이때는 백업을 복구해야 합니다.

백업할 항목:

```text
config/validator.key
data/server.node.key
data/ledger/
data/.ieum-initialized
```

## 신규 노드 초기화와 기존 노드 확인

원본 서버의 `data/`를 복사한 경로는 신규 노드가 아닙니다. 완전한 신규 노드는
명시적으로 초기화합니다.

```bash
# 완전한 신규 노드
./target/release/ieum-chain node init --new

# 기존 노드 상태·키 일치 확인
./target/release/ieum-chain node verify

# 확인 후 실행
./target/release/ieum-chain server
```

`node init --new`는 기존 `data/` 전체와 `config/validator.key`를
`backups/node-init-<UNIX시각>/`으로 자동 백업 이동한 뒤 새 검증자 키, PeerId,
원장과 초기화 표식을 생성합니다. `config/genesis.json`, `validators.json`,
이벤트·업그레이드 설정은 네트워크 공통 파일이므로 유지합니다. 백업은 자동 삭제되지
않습니다.

`node verify`는 `validator.key`, `server.node.key`, 원장,
`data/.ieum-initialized`의 신원이 모두 일치할 때만 성공합니다.

## 메시지·화상채팅·원격 도움

peer 간 메시지와 화상채팅은 기술적으로 가능합니다. 다만 블록체인 원장에 메시지,
통화 내용, 화면 영상을 기록하면 안 됩니다. 별도의 종단간 암호화 통신 계층으로
구현해야 합니다.

v0.18.1에는 지갑이 화상채팅을 연결할 때 필요한 암호화 통신 신호 전달 코어가
포함되었습니다. 실제 카메라·마이크 화면은 `ieum-wallet`에서 구현해야 합니다.
원격 키보드·마우스 제어는 보안 감사 전까지 포함하지 않습니다. 자세한 조건과 RPC는
[보안 통신 설계 문서](docs/SECURE_PEER_COMMUNICATION.md)를 참고하세요.

## 다른 사용자에게 IEUM 보내기

잔액·서명 거래·BFT 확정은 이미 체인 핵심 기능에 포함되어 있습니다. 실제 사용자는
`ieum-wallet`의 받는 주소, 금액, 수수료 확인 화면에서 전송합니다. 지갑은 로컬 노드의
`eth_sendTransaction` 또는 서명된 거래용 `eth_sendRawTransaction`을 호출합니다.

검증자 콘솔에서 개인키를 붙여 넣어 송금하지 마세요. 송금 개인키는 지갑의 암호화
keystore 또는 운영체제 보안 저장소에만 보관해야 합니다.

v0.19.7의 최초 참여 보상 주소와 잔액 전송은 노드에서 다음 명령으로 처리합니다.

```bash
./ieum-chain reward address
./ieum-chain reward send --to 0x받는지갑주소 --amount 1 --fee 0.000001
```

`reward send`는 실행 중인 로컬 RPC `127.0.0.1:8989`에서 nonce를 조회하고
`data/reward.key`로 서명한 뒤 거래를 제출합니다. 보상 키 파일은 다른 서버와
공유하거나 Git에 커밋하지 마세요.

개발자용 빌드, 테스트, 구조 설명은 [README_DEV.md](README_DEV.md)를 참고하세요.
