# IEUM Chain v0.19.5

## 처리 내용

- 동일한 `PeerId + 검증자 공개키`의 등록 수신이 반복되면 새 로그 행을 만들지 않고
  터미널 한 줄의 `(N회)` 숫자만 갱신합니다.
- 회전 파일 로그에는 최초와 10·100·1000회 누적 시점만 기록해 로그 증가를 줄입니다.
- `data/server.node.key`가 유효하게 존재하면 이 파일을 노드 신원의 기준으로 사용합니다.
- 초기화 marker의 과거 PeerId와 현재 키의 PeerId가 달라도 서버 실행을 중단하지 않습니다.
- 변경 전 marker는 `data/.ieum-initialized.previous`에 한 번 백업하고,
  `data/.ieum-initialized`를 현재 PeerId로 갱신합니다.

## 노드 키 주의사항

PeerId는 공개 식별자이므로 PeerId 문자열만으로 과거 `server.node.key` 개인키를 복구할
수 없습니다. v0.19.5는 현재 디스크에 존재하는 유효한 `data/server.node.key`를 계속
사용합니다. 해당 파일 자체가 사라진 경우에는 다른 노드가 되는 사고를 막기 위해
기존과 같이 실행을 중단하므로 백업 키를 복구해야 합니다.

## 적용

기존 `config`, `data`, `validator.key`, `server.node.key`를 삭제하지 않고 변경 파일만
덮어쓴 뒤 빌드합니다.

```bash
cargo build --release --locked
./target/release/ieum-chain server
```
