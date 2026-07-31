# IEUM 서버·일반 PC 실행 구분

## 1. 운영 서버

운영 서버는 외부 노드가 접속하는 부트스트랩 노드입니다.

```bash
cargo run --release -- server \
  --port 7001 \
  --rpc-host 127.0.0.1 \
  --rpc-port 8989 \
  --node-key data/server.node.key
```

한 번 빌드한 뒤 운영할 때는 바이너리를 직접 실행합니다.

```bash
cargo build --release

./target/release/ieum-chain server \
  --port 7001 \
  --rpc-host 127.0.0.1 \
  --rpc-port 8989 \
  --node-key data/server.node.key

./target/release/ieum-chain server --port 7001 --rpc-host 127.0.0.1 --rpc-port 8989 --node-key data/server.node.key
```

- `node.ieum.aah.name`: Cloudflare DNS only, 서버 공인 IP
- `7001/UDP`: 외부 P2P용으로 개방
- `127.0.0.1:8989`: 서버 내부 RPC이며 외부에 직접 개방하지 않음
- `data/server.node.key`: 재시작해도 서버 PeerId를 유지하므로 백업 필요

서버 최초 실행 로그에 출력된 PeerId를 일반 PC의 `--peer` 끝에 사용합니다.

## 2. 일반 PC

일반 PC는 운영 서버를 부트스트랩 피어로 지정합니다.

```bash
cargo run --release -- client \
  --port 7002 \
  --rpc-host 127.0.0.1 \
  --rpc-port 8991 \
  --node-key data/client.node.key \
  --peer /dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/<서버_PeerId>
```

빌드된 바이너리 실행:

```bash
./target/release/ieum-chain client \
  --port 7002 \
  --rpc-host 127.0.0.1 \
  --rpc-port 8991 \
  --node-key data/client.node.key \
  --peer /dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/<서버_PeerId>
```

같은 PC에서 서버와 일반 노드를 함께 실행하면 포트가 겹치지 않도록 일반 PC 예제는
P2P `7002/UDP`, RPC `8991/TCP`를 사용합니다. 서로 다른 PC라면 일반 PC도 기본 포트
`7001`, `8989`를 사용할 수 있습니다.

## 3. 공개 RPC

지갑과 외부 서비스는 노드의 `8989` 포트에 직접 접속하지 않습니다.

```text
https://rpc.ieum.aah.name
  -> Cloudflare
  -> Caddy
  -> 127.0.0.1:8990 IEUM RPC Gateway
  -> 127.0.0.1:8989 IEUM 서버 노드
```
