# IEUM Chain v0.20.8 검증자 설정 안전 저장 및 CI 격리

## 요청

- v0.20.7 GitHub Actions에서 발생한 `validators.json.tmp` 동시 저장 충돌을
  해결한다.
- 4프로세스 BFT 테스트가 운영 `node.ieum.aah.name` 노드에 접속하지 않도록
  완전히 격리한다.
- 설치·운영·복구와 최근 P2P 로그 판별 방법을 사용자 매뉴얼에 통합한다.

## 처리

### 검증자 설정 저장 경쟁 제거

기존에는 여러 프로세스가 같은 `validators.json.tmp`를 사용했습니다. 한
프로세스가 먼저 파일을 교체하면 다른 프로세스의 rename이 `No such file`로
실패할 수 있었습니다.

v0.20.8은 PID와 고해상도 시각을 포함한 고유 임시 파일을 배타적으로 생성하고,
내용을 디스크에 동기화한 다음 최종 파일로 원자 교체합니다. 테스트 스크립트도
노드별 `node-N/validators.json`을 사용합니다.

### 테스트망의 운영망 접속 차단

`server --no-default-bootstrap` 옵션을 추가했습니다. 이 옵션은 `--peer`가 없는
경우에도 내장 운영 bootstrap을 불러오지 않습니다. 4프로세스 테스트의 노드 1은
외부 운영망에 연결하지 않고, 노드 2~4는 로컬 노드 1 주소만 사용합니다.

이 옵션은 CI와 폐쇄형 개발망 전용입니다. 일반 운영 노드에서는 사용하지 않습니다.

### 사용자 매뉴얼 보강

`docs/USER_MANUAL_0.20.8.md`에 다음 내용을 반영했습니다.

- v0.20.8 설치·패키징·자동 업데이트
- 안전한 백업, `node doctor`, `node clean`의 차이
- `Unexpected peer ID`, QUIC timeout, AutoNAT `DialError` 판별
- 고유 피어 수와 동일 피어의 연결 수 구분
- 노드별 검증자 설정 분리와 격리된 4노드 CI 실행

## 검증 명령

```bash
cargo fmt --all
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
bash tests/four_process_network.sh target/release/ieum-chain
```
