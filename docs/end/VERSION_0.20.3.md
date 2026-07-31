# IEUM Chain v0.20.3

## 목적

채굴장, VMware 팜, 회사·가정 NAT, CGNAT처럼 여러 노드가 하나의 공인 IP를
공유하는 환경에서 노드를 추가할 때마다 공유기 포트를 열지 않고도 P2P 연결을
유지합니다.

## 연결 순서

1. 같은 LAN에서는 mDNS로 발견한 사설 IP에 직접 연결합니다.
2. 외부 노드는 `config/bootstrap.json`의 공개 노드에 발신 연결합니다.
3. 일반 노드는 공개 노드에 Circuit Relay v2 예약을 요청합니다.
4. 릴레이를 통해 서로 발견한 노드는 DCUtR 홀 펀칭으로 직접 연결 승격을
   시도합니다.
5. 대칭형 NAT나 CGNAT로 직접 연결이 불가능하면 릴레이 경로를 유지합니다.

## 포트 정책

- `node.ieum.aah.name`에 연결된 공개 부트스트랩 노드는 기존처럼 외부 UDP
  7001을 열어 둡니다.
- 같은 공인 IP 아래의 추가 VM과 일반 클라이언트는 외부 포트포워딩이 필요하지
  않습니다.
- 각 프로세스의 `data/server.node.key`는 반드시 달라야 하며, 복제된 VM은 첫
  실행 전에 중복 키를 제거해 각자 새 PeerId를 생성해야 합니다.
- 호스트 한 대에서 여러 프로세스를 실행할 때는 로컬 리스닝 포트가 충돌하므로
  프로세스별 `--port` 값은 달라야 합니다. 서로 다른 VM은 모두 내부 UDP
  7001을 사용할 수 있습니다.

## 운영 로그

정상적인 NAT 뒤 노드는 다음 로그를 남깁니다.

```text
[NAT 릴레이 예약 시도] ...
[NAT 릴레이 준비 완료] Relay PeerId: ...
[NAT 접근성 판정] ...
[NAT 홀 펀칭] ...
```

`[NAT 릴레이 준비 완료]`가 보이면 홀 펀칭 성공 여부와 관계없이 릴레이를 통한
수신 경로가 확보된 것입니다. 공개 노드는 다른 노드의 예약·회로 요청을
`[NAT 릴레이 서버]` 로그로 표시합니다.

## 빌드·패키징

```bash
cargo fetch
cargo fmt --all
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
scripts/make-node-package.sh 0.20.3
```

새 libp2p 기능을 처음 적용한 저장소에서는 `make-node-package.sh`가 먼저
`cargo fetch`를 실행해 기존 잠금 파일에 필요한 전이 의존성만 추가합니다.
생성되거나 변경된 `Cargo.lock`도 릴리스 파일과 함께 commit해야 합니다.

## 배포 전 확인

공개 부트스트랩 노드 한 대에서 UDP 7001이 외부에서 접근 가능해야 합니다.
릴레이는 모든 합의 트래픽의 영구 중앙 경로가 아니라 직접 연결이 불가능할 때의
안전 경로입니다. 운영 노드가 늘어나면 서로 다른 공인망의 공개 릴레이를 두 대
이상 `bootstrap.json`에 추가하는 것을 권장합니다.
