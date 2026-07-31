# IEUM Chain v0.19.4

## 처리 내용

- `ieum-chain update` 비대화형 명령을 추가했습니다.
- manifest Ed25519 서명과 바이너리 SHA-256을 모두 확인한 뒤 실행파일만 교체합니다.
- Linux에서 `tar.xz`를 실행파일로 잘못 교체하지 않도록 ELF 형식을 검사합니다.
- systemd용 업데이트 스크립트가 서버 정지, 교체, 재시작, RPC 확인을 수행합니다.
- 새 버전의 RPC 확인이 60초 안에 실패하면 `.previous` 바이너리를 복구합니다.
- 최초 초기화 marker의 검증자 공개키와 PeerId를 재시작 때 실제 키와 비교합니다.
- 업데이트는 `validator.key`, `server.node.key`, `config`, `data/ledger`를 변경하지 않습니다.
- 기본 부트스트랩 PeerId를 운영 서버의 실제 영구 PeerId
  `12D3KooWAVRZjnbP8nXp8vD6irYFAXdLJVyczEFWdKLFzKnKDATx`로 맞췄습니다.
- 같은 P2P 오류는 터미널 한 줄에서 메시지별 누적 횟수만 갱신합니다.
- `NoPeersSubscribedToTopic`도 반복 로그 집계 대상이며 파일 로그는 최초와
  10·100·1000회 요약만 기록합니다.
- `rolling-deploy.sh`는 빌드 서버에서 검증자 서버를 한 대씩 업데이트하고,
  각 서버의 RPC 확인 실패 시 이전 바이너리로 복구한 뒤 다음 배포를 중단합니다.

## 배포파일 생성

```bash
chmod +x scripts/*.sh
scripts/build-ubuntu-release.sh 0.19.4
```

생성되는 Git 추가 대상:

```text
download/ieum-chain_ubuntu_x86_64_v0.19.4
download/ieum-chain_ubuntu_x86_64_v0.19.4.tar.xz
download/SHA256SUMS-v0.19.4.txt
```

자동 업데이트 manifest의 Linux URL은 압축파일이 아니라 위 무압축 바이너리의
GitHub raw URL이어야 합니다. manifest는 오프라인으로 보관한 Ed25519 릴리스
개인키로 서명해야 하며 개인키는 저장소나 서버에 올리지 않습니다.

## systemd 업데이트 실행

검증자는 한 대씩 순서대로 실행하고 정상 복귀를 확인한 후 다음 검증자를 업데이트합니다.

```bash
sudo env \
  IEUM_SERVICE_NAME=ieum-chain.service \
  IEUM_BINARY_PATH=/opt/ieum-chain/ieum-chain \
  IEUM_UPDATE_MANIFEST_URL=https://example.com/manifest.json \
  IEUM_RELEASE_PUBLIC_KEY=<64자리_hex_공개키> \
  bash scripts/ieum-chain-systemd-update.sh
```

네 검증자에서 동시에 실행하면 합의가 멈출 수 있으므로 cron을 같은 시각으로 설정하지
않습니다. 우선 수동 순차 실행으로 검증한 뒤 노드별 시간차를 둔 systemd timer를
적용합니다.

## 빌드 서버에서 다른 서버로 순차 배포

대상 서버는 SSH 공개키 로그인과 `systemctl`, `install`, `cp`에 대한 비대화형 sudo가
준비되어 있어야 합니다. 서버별 `config`, `data`, 검증자 키와 노드 키는 건드리지
않습니다.

```bash
scripts/rolling-deploy.sh \
  dev@validator-1 \
  dev@validator-2 \
  dev@validator-3 \
  dev@validator-4
```

기본 설치 경로가 다르면 환경변수로 지정합니다.

```bash
IEUM_REMOTE_BINARY=/home/dev/www/ieum-chain/target/release/ieum-chain \
IEUM_SERVICE_NAME=ieum-chain.service \
scripts/rolling-deploy.sh dev@server2 dev@server3
```
