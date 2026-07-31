# IEUM Chain v0.20.2

## 요청

`scripts/make-node-package.sh 0.20.2`로 릴리스 파일을 생성하고 Git에 commit/push하면
실행 중인 클라이언트 노드가 새 manifest를 확인해 자동으로 업데이트되어야 합니다.

## 수정 내용

- 현재 작업 디렉터리에만 의존하던 `config/update.json` 탐색을 수정했습니다.
  설치 실행 파일의 부모 경로에 있는 `config/update.json`을 먼저 확인하고, 소스에서
  직접 실행하는 경우 기존 상대 경로를 확인합니다.
- 자동 업데이트가 활성화된 노드는 시작 즉시 manifest를 한 번 확인하고 이후
  `check_interval_secs` 주기로 다시 확인합니다.
- GitHub raw/CDN에 남은 이전 manifest를 받지 않도록 매 검사 URL에 고유 query를
  붙이고 `Cache-Control: no-cache`, `Pragma: no-cache` 요청 헤더를 사용합니다.
- 다운로드한 manifest의 Ed25519 서명과 실행 파일 SHA-256 검증, 현재 실행 파일의
  `.previous` 백업, 원자적 교체 절차는 유지합니다.
- 새 파일 설치 후 서버 이벤트 루프가 정상 종료되며 systemd의 `Restart=always`에
  의해 새 실행 파일로 다시 시작합니다.

## 릴리스 생성

```bash
scripts/make-node-package.sh 0.20.2
```

명령이 성공하면 다음 파일이 생성 또는 갱신됩니다.

```text
download/ieum-chain_node_ubuntu_x86_64_v0.20.2.tar.xz
download/ieum-chain_node_ubuntu_x86_64_v0.20.2.sha256
download/ieum-chain_linux_x86_64_v0.20.2
download/update-manifest.json
config/update.json
```

이 파일들과 소스 변경을 같은 commit으로 push해야 합니다. manifest가 먼저 보이고
바이너리가 아직 보이지 않는 짧은 시간에는 업데이트가 실패할 수 있으나, 서비스는
현재 버전으로 계속 실행되고 다음 주기에 다시 시도합니다.

## 정상 로그

```text
[자동 업데이트 활성] 설정: /opt/ieum-chain/config/update.json · 확인 주기: 300초 · 시작 즉시 확인
[자동 업데이트 확인] 서명된 최신 manifest를 확인합니다.
[자동 업데이트 완료] 새 실행 파일을 설치했습니다. 서비스를 재시작합니다.
```

재시작 뒤 확인:

```bash
/opt/ieum-chain/ieum-chain --version
systemctl is-active ieum-chain
journalctl -u ieum-chain -n 100 --no-pager
```

정상 버전은 `ieum-chain 0.20.2`입니다.
