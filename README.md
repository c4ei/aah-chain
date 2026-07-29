# IEUM Chain v0.17.1 쉬운 실행 안내

IEUM Chain은 Ubuntu와 Windows에서 실행할 수 있는 경량 블록체인 노드입니다.
현재 권장 테스트 구성은 **Ubuntu VM 검증자 3대 + Windows VM 검증자 1대**입니다.

> 아직 사설 테스트넷 단계입니다. 실제 자산이나 중요한 개인정보를 보관하지 마세요.

## 가장 먼저 알아둘 것

- VM 4대는 서로 다른 고정 IP를 사용합니다.
- P2P는 `UDP 7001`, RPC는 기본적으로 자기 PC만 접근 가능한 `127.0.0.1:8989`를 사용합니다.
- 네 서버의 `validator.key`, `server.node.key`, `data/ledger`는 서로 달라야 합니다.
- 처음 실행하면 위 파일과 폴더를 각 서버에서 자동 생성합니다.
- `config/validators.json`만 네 서버에 완전히 동일하게 복사합니다.
- 개인키 파일은 메일, 메신저, 클라우드 드라이브로 보내지 마세요.

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

현재 v0.17.1에는 채팅·화상·원격 제어 기능 자체는 포함하지 않았습니다. 안전한 구현
조건과 단계는 [보안 통신 설계 문서](docs/SECURE_PEER_COMMUNICATION.md)를 참고하세요.

개발자용 빌드, 테스트, 구조 설명은 [README_DEV.md](README_DEV.md)를 참고하세요.
