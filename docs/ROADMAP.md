# 개발 진행표

기준일: 2026-07-24

## 처리완료

| 단계 | 상태 | 내용 |
|---|---|---|
| Step 1 | 처리완료 | 지갑, Ed25519 서명, 송금, 블록 해시 연결, 체인 검증 |
| Step 2 | 처리완료 | 모듈 분리, mempool, 수수료, JSON 저장·복구, 체크포인트 |
| Step 3 | 처리완료 | 두 노드 간 블록 전달 개념과 원본 블록 검증 |
| Step 4 | 처리완료 | QUIC/libp2p, mDNS, Kademlia DHT, Gossipsub, 다중 피어 |
| Step 4 | 처리완료 | 메시지 크기 제한, idle timeout, 오류 점수, 임시 차단 |
| Step 5 | 코어 처리완료 | stake 가중치, 제안자 순환, prevote/precommit, 2/3 초과 확정 |
| Step 6-A | 처리완료 | 합의 투표 Ed25519 서명·검증, 단계별 투표 제한 |
| Step 6-B | 처리완료 | WAL 영속 저장·복구, 재시작 후 로컬 이중서명 방지 |

## 현재 제한

- 합의 상태기계와 P2P 전파는 구현됐지만 노드 실행 루프에서 완전히 결합되지 않았습니다.
- Proposal 자체의 검증자 서명과 proposal/block 매핑 검증은 아직 없습니다.
- 라운드 timeout, 잠금(locked value), valid value 규칙은 아직 없습니다.
- 블록 전체 동기화 대신 새 블록/합의 메시지 전파가 중심입니다.
- PeerId 키가 재시작할 때 바뀌므로 운영용 node key 저장이 필요합니다.
- IP 단위 연결 속도 제한, subnet diversity, ASN 다양성은 아직 없습니다.
- 현재 JSON 저장은 큰 체인에 적합하지 않습니다.

## 다음 단계

### Step 6-C: BFT와 노드 완전 결합

- Proposal 서명과 후보 블록 해시 검증
- prevote/precommit 전 WAL 기록 후 P2P 전파
- 외부 검증자의 이중투표 증거 저장·전파
- Tendermint 계열 locked value / valid value 안전 규칙
- propose/prevote/precommit timeout과 라운드 변경
- 확정된 블록만 ledger에 반영
- 검증자 4대 Docker/프로세스 통합 테스트

### Step 7: 안전한 체인 동기화

- Status, GetHeaders, GetBlocks 요청/응답 프로토콜
- 헤더 우선 검증과 batch 크기 제한
- 상태 snapshot, state root, fast sync
- 오래된 블록 pruning과 archive node 분리
- 잘못된 체인을 제공한 피어 감점

### Step 8: PoS 운영

- 스테이킹·언스테이킹·위임
- validator set 변경은 epoch 경계에서만 반영
- double-sign 및 장시간 offline slashing
- 보상, 수수료 분배, 최소 stake
- governance 기반 네트워크 업그레이드

### Step 9: 라이트 클라이언트와 신원

- 웹·모바일용 헤더 검증
- Merkle proof 기반 잔액/거래 증명
- DID 문서, VC 발급기관, 폐기 registry
- 개인정보 원문은 체인 밖에 보관

## 메인넷 전 필수

외부 보안감사, Byzantine/fuzz/property 테스트, 경제 모델 검증, 키 복구,
업그레이드 절차, 재해복구 훈련과 관련 법률 검토가 모두 끝나야 합니다.
