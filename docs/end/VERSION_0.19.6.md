# IEUM Chain v0.19.6

## Ubuntu 노드 자동 배포
scripts/make-node-package.sh 0.19.6

- `scripts/make-node-package.sh`가 실행파일, 공개 설정, 설치기와 systemd 서비스
  템플릿을 하나의 `tar.xz`로 생성합니다.
- `scripts/install-node-package.sh`는 `/opt/ieum-chain`에 설치하고 부팅 시 자동
  시작되는 `ieum-chain.service`를 등록합니다.
- 재설치 시 서버 고유 `validator.key`, `server.node.key`, 원장과 로그는
  보존합니다.
- `scripts/deploy-node-package.sh`는 SSH로 여러 서버에 순서대로 업로드하고
  SHA-256 확인 후 설치·실행합니다.
- 공개 부트스트랩 주소의 PeerId를 `192.168.1.148` 노드의 영구 PeerId
  `12D3KooWCngLfRL315jgBHezczSQtgsqjcVqvHMbhmUibkRT7Veb`로 갱신했습니다.

## 요청

- 완전한 신규 노드를 명시적으로 초기화하는 명령을 추가합니다.
- 기존 노드의 상태와 두 서버 키가 일치하는지 실행 전에 확인합니다.
- `server.node.key`가 최초 PeerId와 다르면 자동 채택하지 않습니다.
- v0.19.5에서 삭제되어 CI가 실패한 번들 제네시스를 확인하고 복원합니다.
- 신규 서버가 기존 합의망에 접속할 때 로컬 공개키를 수동으로
  `validators.json`에 추가하지 않아도 동기화 노드로 시작하게 합니다.

## 처리 내용

- `ieum-chain node init --new`
  - 기존 `data/` 전체, `config/validator.key`, `config/validators.json`을
    `backups/node-init-<UNIX시각>/`으로 자동 백업 이동합니다.
  - 신규 검증자 키, P2P 키, 원장과 marker를 생성하고, 검증자 설정은 내용이
    바뀌지 않은 번들 제네시스 기준으로 복원합니다.
- `ieum-chain node verify`
  - 기존 키 파일의 형식을 검사합니다.
  - 두 키에서 계산한 공개 신원을 최초 초기화 marker와 비교합니다.
  - 불일치하거나 원장이 없으면 실행하지 않고 오류를 반환합니다.
- 서버 시작 시 marker와 `server.node.key`의 PeerId가 다르면 marker를 자동 갱신하지
  않고 중단하도록 변경했습니다.
- `validators.json`이 없으면 신규 로컬 키 하나를 검증자로 기록하지 않고, 번들
  제네시스의 검증자 4명을 복원합니다.
- 로컬 키가 현재 검증자 집합에 없더라도 서버를 종료하지 않습니다.
  - P2P 연결과 원장 동기화는 정상 수행합니다.
  - `validator_id + PeerId` 소유권 서명을 자동 전송합니다.
  - 승인 전에는 proposal/prevote/precommit을 만들지 않는 일반 노드로 동작합니다.
- 수신 노드는 후보의 PeerId 형식과 Ed25519 소유권 서명을 검사합니다. 유효한
  후보는 대기 상태로 기록하지만, 단순 접속만으로 합의권을 주지 않습니다.
- 높이 0의 레거시 개발망에서 검증자가 4명 미만인 경우에만 기존 4노드 자동 구성
  경로를 유지합니다. 운영망 검증자 변경은 현재 검증자 승인과 다음 epoch 적용
  절차를 거쳐야 합니다.
- `config/genesis.json`은 `src/genesis.rs`의 `include_str!`로 바이너리에 컴파일되는
  필수 파일이므로 v0.19.4와 동일한 내용으로 복원했습니다.
- crate 버전을 `0.19.6`으로 올렸습니다.

## 사용

```bash
# 완전한 신규 노드
ieum-chain node init --new

# 기존 노드 상태·키 일치 확인
ieum-chain node verify

# 확인 후 실행
ieum-chain server
```

## 제네시스 주의사항

운영 네트워크가 시작된 뒤 제네시스 내용은 변경하면 안 됩니다. 신규 노드도 기존
네트워크와 동일한 제네시스가 필요합니다. 이번 변경은 삭제된 파일을 원래 내용 그대로
복원한 것이며 잔액, 검증자, chain ID는 변경하지 않았습니다.

## 신규 노드의 합의 참여 상태

```text
신규 키 생성
→ 기존 제네시스 검증자 집합으로 P2P/원장 동기화
→ 서명된 검증자 후보 자동 전송
→ PeerId·키 소유권 확인
→ 후보 대기(합의권 없음)
→ 기존 검증자 승인 및 다음 epoch 반영 후 합의 참여
```

v0.19.6은 미등록 노드의 안전한 자동 접속·동기화·후보 전송까지 처리합니다.
지분율, 국가별 순위, 관리자 승인을 실제 원장 상태와 결합해 다음 epoch에 확정하는
거버넌스 트랜잭션은 후속 프로토콜 변경 대상입니다. 해당 데이터 없이 P2P 접속만으로
검증자를 자동 승인하는 동작은 의도적으로 허용하지 않습니다.
