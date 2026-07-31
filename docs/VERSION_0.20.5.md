# IEUM Chain v0.20.5

## 요청

- 세 운영 노드를 당분간 기본 부트스트랩으로 고정합니다.
- 각 서버에서 부트스트랩과 외부 공개 광고 주소를 설정할 수 있게 합니다.
- 일반 사용자는 별도 설정 없이 실행하면 자동으로 연결되어야 합니다.
- 업데이트는 최초 로딩 시 한 번, 이후 6시간마다 확인합니다.

## 기본 자동 설정

설정 파일이 없거나 예전 `config/bootstrap.json`이 남아 있어도 기본 실행은 아래 세
운영 노드를 사용합니다.

```text
/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/12D3KooWGABnBEucGacnREpBieFwspL5q7Aa6RRuj1MtxEYwrPo2
/dns4/node.ieum.aah.name/udp/7002/quic-v1/p2p/12D3KooWFgNUFENiTt9ftxGU97PuRJN2kiayFgwNPDAPmUVB3xgD
/dns4/node.ieum.aah.name/udp/7003/quic-v1/p2p/12D3KooWQjeX3TQf4LGdFj39EFA4JUtF5bnZk7wxuFdjmZzXGt4L
```

일반 PC와 월렛 내장 노드는 기존처럼 옵션 없이 실행하면 됩니다.

```bash
./ieum-chain
```

## 서버별 공개 주소 설정

세 서버는 모두 로컬 UDP 7001에서 대기하지만 공유기의 외부 포트가 다르므로 각
서버에서 한 번씩 자신의 주소를 저장합니다. 주소의 PeerId는 현재
`data/server.node.key`와 반드시 일치해야 하며, 다르면 실행을 중단해 잘못된 광고를
막습니다.

7001 서버:

```bash
./ieum-chain network set \
  --advertise-address /dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/12D3KooWGABnBEucGacnREpBieFwspL5q7Aa6RRuj1MtxEYwrPo2
```

7002 서버:

```bash
./ieum-chain network set \
  --advertise-address /dns4/node.ieum.aah.name/udp/7002/quic-v1/p2p/12D3KooWFgNUFENiTt9ftxGU97PuRJN2kiayFgwNPDAPmUVB3xgD
```

7003 서버:

```bash
./ieum-chain network set \
  --advertise-address /dns4/node.ieum.aah.name/udp/7003/quic-v1/p2p/12D3KooWQjeX3TQf4LGdFj39EFA4JUtF5bnZk7wxuFdjmZzXGt4L
```

확인과 초기화:

```bash
./ieum-chain network show
./ieum-chain network reset
```

부트스트랩 목록을 직접 교체할 때는 `--bootstrap`을 주소 수만큼 반복합니다. 저장
위치는 `config/network.json`이며 직접 편집하지 않아도 됩니다.

## 업데이트 주기

`config/update.json`, 예제 설정, 노드 패키지 생성 설정의 기본 확인 주기를 모두
`21600`초로 변경했습니다. Tokio interval의 첫 tick은 즉시 발생하므로 실행 직후 한
번 확인하고 다음부터 6시간 간격으로 확인합니다.

## 로그

공개 주소가 적용되면 다음 로그가 출력됩니다.

```text
[P2P 공개 주소 광고] /dns4/node.ieum.aah.name/udp/7002/quic-v1/p2p/...
[자동 업데이트 활성] ... 확인 주기: 21600초 · 시작 즉시 확인
```
