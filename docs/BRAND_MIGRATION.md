# IEUM 개발명에서 IEUM으로 변경

## 구분

| 항목 | 기존 운영 체인 | 새 Rust 체인 |
|---|---|---|
| 이름 | IEUM | IEUM |
| 구현 | geth/Clique | Rust PoS/BFT |
| Chain ID | 21133 | 21004 |
| 코인 심볼 | IEUM | IEUM |
| 프로젝트 | 기존 운영 서비스 | `ieum-chain` |

IEUM 운영 체인은 이름이나 Chain ID를 변경하지 않습니다. 이 저장소에서 개발 중인
새 체인만 IEUM으로 부릅니다.

## 호환성 주의

브랜드 변경과 함께 P2P 토픽, identify protocol, 합의 서명 도메인이
`ieum-chain`/`IEUM-CONSENSUS`로 변경됐습니다. 따라서 이전 개발명의 테스트
노드와 IEUM 노드는 서로 다른 네트워크로 취급됩니다.

실제 제네시스를 확정할 때는 다음 값을 한곳의 체인 사양 파일로 모아 해시를
고정해야 합니다.

- Chain ID `21004`
- 네트워크 이름 `IEUM`
- 네이티브 코인 심볼과 decimals
- 최초 계정 및 공급량
- 최초 검증자와 투표권
- 블록/세그먼트 제한
