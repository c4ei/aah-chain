# IEUM Chain v0.19.7

## 요청

- 최초 서버 노드 4개가 연결되고 키 소유권까지 확인되었는데 후보 대기만 반복되는 문제 수정
- 서버에서 새 버전을 배포하면 연결된 P2P 서버 노드도 자동 업데이트
- Ubuntu x86_64 실행 파일과 압축 배포본 생성

## 처리

### 최초 4검증자 합의 전환

v0.19.6은 `validators.json`에 제네시스 검증자 4명이 이미 있으면
`validators.len() < 4` 조건 때문에, 실제 P2P에서 서명 확인된 서버 4명이 모여도
부트스트랩 검증자 교체를 실행하지 못했습니다.

v0.19.7은 체인 높이가 0이고 서명 확인된 서로 다른 등록이 4개 이상이면 현재 설정의
검증자 수와 관계없이 정렬된 첫 4개를 공통 검증자 집합으로 한 번 전환합니다.
블록이 생성된 뒤 새 후보는 기존처럼 승인 및 다음 epoch 적용 전까지 합의권을 받지
않습니다.

### P2P 자동 업데이트

각 서버에 `config/update.json`을 배치하면 다음 동작이 활성화됩니다.

- 설정 주기마다 HTTPS manifest 확인
- 연결된 P2P 노드에 현재 버전 알림 전파
- P2P 알림을 받은 노드는 즉시 자기 manifest를 확인
- 로컬에 고정된 Ed25519 릴리스 공개키로 manifest 서명 검증
- 대상 실행 파일의 SHA-256 검증
- 기존 실행 파일을 `ieum-chain.previous`로 백업하고 새 실행 파일로 원자적 교체
- 정상 종료하여 systemd의 재시작 정책으로 새 버전 실행

P2P 알림에는 다운로드 URL이나 공개키를 넣지 않습니다. 악성 피어가 업데이트 알림을
보내더라도 각 노드는 로컬 `config/update.json`의 HTTPS URL과 공개키만 신뢰합니다.

설정 예:

```json
{
  "enabled": true,
  "manifest_url": "https://download.example.com/ieum-chain/update-manifest.json",
  "release_public_key": "32바이트_ED25519_공개키_HEX",
  "check_interval_secs": 300
}
```

`config/update.example.json`을 `config/update.json`으로 복사한 뒤 실제 값을
입력합니다. systemd 서비스에는 `Restart=always` 또는 `Restart=on-success`를
설정해야 자동 교체 직후 새 바이너리가 다시 시작됩니다.

개인키, `validator.key`, `server.node.key`, 원장과 설정 파일은 업데이트 대상이
아니며 실행 파일만 교체합니다.
