# IEUM Chain v0.19.3

## 장애 원인

`Unexpected peer ID`는 접속 주소에 기대한 PeerId와 해당 주소에서 실제 응답한 노드의 영구 PeerId가 다를 때 libp2p가 연결을 거부하는 정상 보안 검사입니다. 보고된 `127.0.0.1`, `172.18.0.1`은 다른 로컬 프로세스 또는 Docker bridge가 mDNS/Identify로 광고한 주소여서, 다른 PeerId의 주소로 잘못 재사용될 수 있었습니다.

## 처리 내용

- mDNS와 Identify에서 loopback, unspecified, Docker bridge 대역 주소를 원격 피어 주소로 등록하지 않습니다.
- 내장 및 기본 `bootstrap.json` PeerId를 현재 운영 서버 영구 키의 `12D3KooWPw75...h5j69`로 맞췄습니다.
- 같은 PeerId·같은 오류의 반복 접속 실패는 터미널 한 줄에서 `(N회)`만 갱신합니다.
- 회전 로그에는 최초 오류만 저장해 동일 오류로 로그 파일이 급증하지 않게 했습니다.
- `Unexpected peer ID` 발생 시 `bootstrap.json`과 운영 `server.node.key` 불일치 확인 안내를 출력합니다.

## 운영 확인

운영 서버에서 `data/server.node.key`를 삭제하거나 다른 서버 파일로 덮어쓰면 PeerId가 바뀝니다. 서버 시작 시 출력되는 실제 PeerId와 `config/bootstrap.json`의 `/p2p/...` 값을 반드시 같게 유지해야 합니다.

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
```
