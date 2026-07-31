# IEUM Chain v0.20.1

## 장애 원인

검증자 네 명의 자동 등록이 끝난 뒤 합의 단계가 이미 `Prevote`로 변경됐는데도
로컬 노드가 같은 라운드의 블록 제안을 다시 처리할 수 있었습니다. 이때 발생한
`현재 단계에서는 블록을 제안할 수 없습니다.` 오류가 서버 이벤트 루프까지
전파되어 systemd 재시작 루프가 발생했습니다.

## 수정

- `Propose` 단계이면서 현재 라운드 제안자인 노드만 로컬 제안을 생성합니다.
- 제안 생성과 실제 수신 처리 사이에 단계가 변경되면 거래를 mempool로 복원하고
  다음 tick에서 다시 시도합니다.
- 정상적인 합의 단계 경쟁은 로그로 남기되 노드 프로세스를 종료하지 않습니다.
- 연결 직후 검증자 등록 전송은 유지하고 반복 heartbeat는 60초로 조정했습니다.
- `Prevote` 이후 중복 제안을 거부하는 회귀 테스트를 추가했습니다.

## 운영 확인

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
./target/release/ieum-chain --version
```

정상 버전 출력은 `ieum-chain 0.20.1`입니다. 배포 후에는 검증자 `4/4` 전환 뒤에도
서비스 PID가 유지되고 블록 제안 단계 오류로 systemd 재시작 횟수가 증가하지 않는지
확인합니다.
