# IEUM Chain v0.21.2 한 서버 4노드 운영 구성

## 요청 사항

- 현재 운영 키와 원장을 유지하면서 한 Ubuntu 서버에서 검증자 노드 4개를 실행한다.
- 추가 노드가 `/usr/local/bin/ieum-chain` 공용 복사본을 사용하지 않게 한다.
- 잘못된 내장 공인 PeerId 대신 같은 서버의 LAN 주소와 실제 PeerId를 사용한다.
- 변경된 파일만 배포할 수 있게 한다.

## 처리 내용

- 버전을 `0.21.1`에서 `0.21.2`로 올렸다.
- `ieum-chain node peer-id --key 경로` 명령을 추가했다. 기존 키가 있으면 읽기만
  하며, 없을 때만 영구 노드 키를 새로 생성한다.
- `scripts/configure-single-server-four-nodes.sh`를 추가했다.
  - `/opt/ieum-chain`, `/opt/ieum-node1`, `/opt/ieum-node2`,
    `/opt/ieum-node3`의 실행파일을 각각 사용한다.
  - 기존 `validator.key`, 노드 키, 원장 파일을 삭제하거나 덮어쓰지 않는다.
  - 서비스를 중지하기 전에 네 노드의 필수 설정 파일과 동일한
    `validators.json` 사용 여부를 검사한다.
  - 각 노드의 실제 PeerId를 계산해 다른 세 노드를 `config/network.json`에 등록한다.
  - 내부 연결은 LAN IP를 사용하고, 외부 공개 광고는
    `node.ieum.aah.name:7001~7004/UDP`와 각 노드의 실제 PeerId를 사용한다.
  - P2P 포트는 `7001~7004`, RPC 포트는 `8989~8992`를 사용한다.
  - systemd 서비스가 각 작업 폴더의 `ieum-chain`을 직접 실행하도록 교체한다.
  - 서비스 설정 교체가 끝난 후 과거 공용 복사본 `/usr/local/bin/ieum-chain`을 제거한다.
- `scripts/ieum-chain-four-node.service.in` systemd 템플릿을 추가했다.
- 신규 외부 사용자가 별도 설정 없이 접속할 수 있도록 내장 부트스트랩을 현재
  4개 노드의 실제 PeerId와 `7001~7004/UDP` 주소로 갱신했다.

## 적용 전 확인

네 작업 폴더에 서로 다른 `validator.key`와 노드 키가 있어야 하며,
`config/validators.json`은 네 노드에서 동일해야 한다. 기존 파일은 먼저 별도로
백업하는 것을 권장한다.

```bash
sha256sum \
  /opt/ieum-chain/config/validators.json \
  /opt/ieum-node1/config/validators.json \
  /opt/ieum-node2/config/validators.json \
  /opt/ieum-node3/config/validators.json
```

## 빌드 및 적용

```bash
cargo build --release --locked

sudo IEUM_RUN_USER=dev \
  IEUM_SOURCE_BINARY="$PWD/target/release/ieum-chain" \
  ./scripts/configure-single-server-four-nodes.sh 192.168.1.148
```

LAN IP `192.168.1.148`은 서버의 실제 주소에 맞춰 지정한다. 스크립트는 기존
systemd unit을 `.bak_날짜_시간` 파일로 백업한 뒤 네 서비스를 다시 시작한다.

외부 신규 사용자가 접속하려면 DNS의 `node.ieum.aah.name`이 이 서버의 공인 IP를
가리켜야 하며, 공유기/방화벽에서 UDP `7001~7004`가 이 서버의 같은 포트로 전달돼야
한다. QUIC는 TCP가 아니라 UDP이므로 네 포트 모두 UDP 규칙으로 연다.

## 확인

```bash
systemctl status ieum-chain ieum-node1 ieum-node2 ieum-node3 --no-pager

for port in 8989 8990 8991 8992; do
  curl -s http://127.0.0.1:$port \
    -H 'content-type: application/json' \
    --data '{"jsonrpc":"2.0","id":1,"method":"ieum_nodeStatus","params":[]}'
  echo
done
```

## 주의 사항

- 이 구성은 한 서버에서 기능과 합의를 확인하는 임시 구성이다. 서버 한 대가
  중단되면 검증자 네 개가 동시에 중단되므로 실제 분산 운영의 장애 내성은 없다.
- 네 검증자가 동일한 물리 서버에 있으므로 운영 메인넷의 최종 구조로 사용하지 않는다.
