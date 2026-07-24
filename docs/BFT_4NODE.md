# 4검증자 BFT 및 동기화

## 구현 범위

- `data/node.key`에 libp2p 키를 최초 1회 생성하고 재사용하여 PeerId 고정
- `config/bootstrap.json`의 multiaddr 목록 자동 로드
- 서명된 proposal의 높이, 라운드, 제안자, 블록 해시와 Ed25519 서명 검증
- `propose → prevote → precommit → finalized` 실행 코어
- 2/3 초과 precommit 이전 후보 블록 미저장
- timeout 후 proposer가 바뀌는 다음 round 진행
- 높이 기반 최대 128개 확정 블록 동기화 배치와 연속성 검증
- gossipsub proposal/vote/sync request/sync response 메시지
- 결정론적 4노드 통합 테스트

## bootstrap 설정

`config/bootstrap.json`:

```json
[
  "/ip4/203.0.113.10/udp/7001/quic-v1/p2p/12D3KooW..."
]
```

빈 배열이면 같은 LAN에서 mDNS 검색만 사용합니다. node key 파일을 삭제하면 PeerId가
바뀌므로 bootstrap 주소를 다시 배포해야 합니다. `data/node.key`는 백업하되 외부에
공개하지 마십시오.

## 검증

```bash
cargo fmt --check &&
cargo clippy --all-targets --all-features -- -D warnings &&
cargo test --locked
```

`tests/four_node_bft.rs`는 네 개의 독립 원장/검증자 실행 코어에 동일한 proposal과
서명 투표를 전달하여 다음을 검사합니다.

1. prevote만으로는 어느 원장에도 블록이 저장되지 않음
2. 3/4 precommit 뒤 네 원장이 동일한 블록을 확정·저장
3. timeout은 블록 저장 없이 round만 증가
4. 새 노드가 확정 블록 배치를 받아 같은 체인으로 동기화

## 현재 경계

합의 실행 코어와 P2P wire message는 구현됐지만, `main`의 RPC mempool과 합의 실행
코어를 하나의 영속 상태 서비스로 묶는 작업, 검증자 키의 암호화 저장/HSM, 상태
스냅샷 기반 fast sync, peer별 직접 sync 응답, 장시간 장애·분할망 시험은 다음
운영화 단계입니다. 따라서 이 버전은 사설 테스트넷 통합 개발본이며 실자산 운영본이
아닙니다.
