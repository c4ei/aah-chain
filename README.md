# IEUM Chain v0.19.1 쉬운 실행 안내

IEUM Chain은 Ubuntu와 Windows에서 실행할 수 있는 경량 블록체인 노드입니다.
첫 노드는 제네시스 상태에서 단독으로 시작할 수 있습니다. 장애 허용 BFT 검증은
**Ubuntu VM 검증자 3대 + Windows VM 검증자 1대**처럼 4명 이상일 때 활성화합니다.

> 아직 사설 테스트넷 단계입니다. 실제 자산이나 중요한 개인정보를 보관하지 마세요.

## 가장 먼저 알아둘 것

- VM 4대는 서로 다른 고정 IP를 사용합니다.
- P2P는 `UDP 7001`, RPC는 기본적으로 자기 PC만 접근 가능한 `127.0.0.1:8989`를 사용합니다.
- 네 서버의 `validator.key`, `server.node.key`, `data/ledger`는 서로 달라야 합니다.
- 처음 실행하면 위 파일과 폴더를 각 서버에서 자동 생성합니다.
- 최초 실행에서는 로컬 공개키 1개로 `config/validators.json`이 자동 생성됩니다.
- 2번째 이후 후보는 등록·검증을 거쳐 다음 epoch의 공통 검증자 세트에 반영합니다.
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
v0.19.0에는 이 결과로 모든 노드가 동일한 후보를 고르는 정책 코어가 포함됐으며,
등록 메시지의 P2P 전파와 다음 epoch 자동 반영은 후속 작업입니다.

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
config/events.json
data/.ieum-initialized
```

화면에 표시되는 **검증자 공개키**만 기록합니다. 공통 `validators.json`이 아직
없으면 서버는 안전하게 멈춥니다. 오류가 아니라 최초 등록 단계입니다.

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

개발자용 빌드, 테스트, 구조 설명은 [README_DEV.md](README_DEV.md)를 참고하세요.
