# IEUM Chain v0.19.9 사용자 매뉴얼

## 1. 프로그램 역할

IEUM Chain 노드는 IEUM 거래와 확정 원장을 보관하고 QUIC P2P로 다른 노드와
통신합니다. 기본 포트는 P2P `7001/UDP`, 로컬 JSON-RPC
`127.0.0.1:8989`입니다. RPC 8989를 인터넷에 직접 열지 말고 Caddy 같은 HTTPS
reverse proxy 뒤에 둡니다.

`server`는 외부 연결과 합의 참여를 위한 운영 노드이고, `client` 또는 옵션 없는
실행은 일반 PC·월렛용 노드입니다.

## 2. 가장 간단한 실행

일반 PC:

```bash
./ieum-chain
```

운영 서버:

```bash
./ieum-chain server
```

직접 실행은 터미널을 닫거나 서버를 재부팅하면 종료될 수 있습니다. 운영 서버는
아래 systemd 설치 방식을 사용합니다.

## 3. 배포본 설치와 systemd

`make-node-package.sh`는 압축파일만 만들며 서비스를 설치하지 않습니다.
배포본 안의 `install.sh`를 `sudo`로 실행할 때만 systemd 서비스가 등록됩니다.

```bash
tar -xJf ieum-chain_node_ubuntu_x86_64_v0.19.9.tar.xz
cd ieum-chain-node-v0.19.9
sudo ./install.sh
```

기본 설치 경로는 `/opt/ieum-chain`, 서비스명은 `ieum-chain.service`입니다.

```bash
sudo systemctl status ieum-chain --no-pager
sudo journalctl -u ieum-chain -f
sudo systemctl restart ieum-chain
sudo systemctl stop ieum-chain
```

상태 확인:

```bash
curl -sS -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"ieum_nodeStatus","params":[]}' \
  http://127.0.0.1:8989
```

## 4. 방화벽과 공유기

- 서버 인바운드: `7001/UDP`
- 로컬 전용: `8989/TCP`
- 같은 공유기 밖에서 받을 노드는 공유기 UDP 7001 포트포워딩이 필요합니다.
- Cloudflare 프록시는 일반 QUIC/libp2p UDP 포트를 대신 전달하지 않습니다.
  `node.ieum.aah.name`은 DNS only로 운영합니다.

확인:

```bash
sudo ss -lunp | grep ':7001'
pgrep -af '[/]ieum-chain'
```

## 5. 주요 파일과 백업

반드시 백업할 서버 고유 파일:

- `config/validator.key`
- `data/server.node.key`
- `data/reward.key`
- `data/ledger/`
- 운영 중인 `config/validators.json`, `config/events.json`,
  `config/upgrades.json`, `config/bootstrap.json`

개인키는 Git, 배포 압축, 메신저에 넣지 않습니다. 복구 전에는 서비스를 멈추고
전체 디렉터리를 별도 위치에 보존합니다.

```bash
sudo systemctl stop ieum-chain
sudo cp -a /opt/ieum-chain /opt/ieum-chain.backup-$(date +%Y%m%d)
sudo systemctl start ieum-chain
```

노드 일치 검사:

```bash
cd /opt/ieum-chain
./ieum-chain node verify
```

## 6. 무인 자동 업데이트

쉘·systemd 노드는 사람의 `y/n` 입력에 의존하면 안 됩니다. IEUM은
`config/update.json`이 활성화된 경우 다음 절차로 비대화형 업데이트합니다.

1. 로컬 HTTPS 주소에서 manifest 다운로드
2. 노드에 고정한 Ed25519 릴리스 공개키로 manifest 서명 확인
3. 현재 플랫폼 바이너리의 SHA-256 확인
4. 기존 바이너리를 `.previous`로 보존하고 원자적으로 교체
5. 노드 종료 후 systemd가 재시작

```bash
cp config/update.example.json config/update.json
chmod 640 config/update.json
```

`manifest_url`과 `release_public_key`는 운영 릴리스 값으로 교체해야 합니다.
P2P의 새 버전 알림은 확인 시점을 앞당기는 힌트일 뿐이며, 알림을 보낸 피어를
신뢰해 설치하지 않습니다.

검증자 4대 이상은 전부 동시에 자동 업데이트하지 않습니다. 프로토콜 호환
업데이트는 한 대씩 적용하고 RPC·피어·합의 높이를 확인한 뒤 다음 노드로
진행합니다. 합의 규칙이 바뀌는 업데이트는 `config/upgrades.json`의 활성 높이를
검증자들이 사전에 동일하게 합의·배포한 뒤 실행합니다.

긴급 복구:

```bash
sudo systemctl stop ieum-chain
sudo cp --preserve=mode,ownership,timestamps \
  /opt/ieum-chain/ieum-chain.previous /opt/ieum-chain/ieum-chain
sudo systemctl start ieum-chain
```

## 7. 합의는 y/n 입력으로 결정하지 않음

블록 승인, 검증자 변경, 프로토콜 업그레이드, 보상 지급은 월렛의 팝업이나 서버
콘솔 입력으로 결정하면 안 됩니다. 노드는 서명된 제안·prevote·precommit과
확정 인증서로 자동 합의합니다.

월렛은 다음 관리 화면을 제공할 수 있습니다.

- 노드 버전·피어·동기화·검증자 상태 조회
- 운영자가 업그레이드 제안에 서명하는 거버넌스 UI
- 릴리스 서명과 체크섬 표시
- 보상 내역·중계 기여 내역 표시

월렛이 꺼져 있어도 이미 승인된 합의와 예정된 업그레이드는 노드끼리 진행되어야
합니다. 월렛은 개인키 서명과 상태 표시 UI이지 합의 엔진의 중앙 제어기가 아닙니다.

## 8. 로그 해석

시작 직후 다음 메시지는 피어의 topic 구독 전이면 발생할 수 있습니다.

```text
P2P 메시지 전파 실패: NoPeersSubscribedToTopic
```

피어 연결 뒤에도 계속 누적되면 상대 노드 버전, topic 구독, 방화벽을 확인합니다.
v0.19.9부터 반복 메시지는 최초와 10·100·1000회에만 새 줄로 기록되어 다른
비동기 로그와 섞이지 않습니다.

`Unexpected peer ID`는 주소 끝 PeerId와 실제 상대 노드 키가 다르다는 뜻입니다.
`config/bootstrap.json`과 상대의 `data/server.node.key`를 확인합니다.

## 9. v0.19.8 트래픽 분산 기능의 현재 상태

근거리·국가/ASN 다양성·저부하 슬롯 선택과 이벤트 추첨 코어는 구현되어 있지만
현재 실제 P2P 연결에는 아직 활성화하지 않았습니다. 안전한 활성화에는 다음이
추가로 필요합니다.

- 관측 RTT, 실제 연결 수, 가동률 수집
- 서명되고 만료되는 국가/ASN/수용량 광고
- 자기 신고와 외부 관측의 교차 검증
- NAT 뒤 일반 PC가 중계 가능한지 판정하는 reachability
- Sybil·동일 ASN 군집·담합 시뮬레이션
- 중계 영수증 root에 대한 검증자 합의

국가별 보유 순위만으로 일반 연결을 정하면 소형 노드가 배제되고 위치 위조가
보상을 좌우할 수 있습니다. 국가 순위는 검증자 후보 조건 중 하나로만 사용하고,
일반 트래픽은 실제 품질·다양성·저부하를 함께 사용합니다.

## 10. AAH ↔ IEUM 교환

월렛 화면만 추가해서는 신뢰 없는 DEX가 되지 않습니다. AAH가 EVM 체인이고
IEUM이 별도 체인이므로 양쪽의 확정을 검증하고 자산을 잠그거나 발행·해제하는
브리지 계층이 필요합니다.

권장 구성:

- IEUM 체인: swap/HTLC 또는 검증자 서명 기반 bridge 시스템 모듈
- AAH 체인: 대응 smart contract
- relayer: 양 체인의 확정 이벤트 전달
- IEUM Wallet: 교환 견적·승인·진행 상태 UI
- 선택 사항: 별도 웹/앱 DEX UI

초기에는 소액·일일 한도·다중서명·지연 출금·비상정지를 둔 제한형 swap으로
시작하고, 독립 감사 후 유동성 풀 방식으로 확장합니다. 운영 서버가 양쪽 개인키를
단독 보유하는 단순 교환 API는 중앙화 거래소이며 DEX라고 부르면 안 됩니다.

## 11. 운영 전 필수 점검

- 4개 이상 서로 다른 서버·망 사업자에서 BFT 장시간 테스트
- 노드 한 대 중단·복귀·원장 재동기화 시험
- 이중투표, 잘못된 state root, 손상된 snapshot 거부 시험
- 키 분실·유출·교체 절차와 validator 제거 절차
- 서명 릴리스 키의 오프라인 보관과 복구 키
- 백업 복원 훈련과 체크섬 검증
- RPC rate limit, WAF, CORS, method allowlist
- 디스크 부족, 시간 오차, 메모리, FD, 피어 수 모니터링
- 100~1,000 노드 부하·Sybil·NAT 시뮬레이션
- 보상·브리지·DEX 활성화 전 보안 감사

현재 단계는 기능 테스트넷/파일럿 운영에 적합하며, 대규모 실자산 운영은 위 항목과
트래픽 분산·보상 영수증·브리지 감사를 마친 뒤 진행하는 것이 안전합니다.

