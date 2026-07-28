# 개발 진행표

기준일: 2026-07-28

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
| v0.6.1 | 처리완료 | 한글 byte string 컴파일 오류 수정, 간헐 접속 클라이언트 설계 문서 추가 |
| v0.7.1 | 테스트넷 구현 | 실행 루프의 4노드 BFT 확정, 확정 인증서 기반 신규 노드 동기화 |
| v0.8.1 | 테스트넷 구현 | canonical 규칙, 상태 root·인덱스, keystore, mempool·운영 RPC |
| v0.8.2 | 처리완료 | 컴파일 수정과 fmt·clippy·test·release build CI |
| v0.9.1 | 테스트넷 구현 | locked/valid value, 단계별 timeout, 이중투표 증거, 실제 4프로세스 시험 |
| v0.10.1 | 테스트넷 구현 | snapshot chunk 재개, 다중 피어 tip/state root quorum, embedded DB, 외부 signer |
| v0.11.1 | 테스트넷 기반 | snapshot 병렬 재시도, embedded WAL, signer timeout·HA, metrics 코어 |
| v0.11.2 | CI 안정화 | Rust 1.97 fmt/clippy 기준과 전체 회귀 검사 |
| v0.12.1 | 합의 확장 | nil vote, round-change 인증서, 지연된 validator era |
| v0.13.1 | 앱 모듈 | 모듈 prefix/router, 결정론적 상태 전이, 상태 migration |
| v0.14.1 | 운영 기반 | pruning/archive, backup/restore manifest, metrics HTTP |
| v0.15.1 | 계정 보안 | 다중키, 가중치 권한, 고액 송금·복구 임계값 |
| v0.15.2 | 저장 엔진 | SQLite WAL, 원자적 batch, 기존 JSON 상태 자동 이관 |
| v0.16.1 | 수수료 정책 | 검증자 80%, 재단 20%, 소각 0% 합의 분배 |

## 현재 제한

- 4노드 BFT와 P2P 실행 루프는 결합됐지만 실제 서로 다른 서버 4대의 장기 장애 시험이 필요합니다.
- locked/valid value 규칙은 구현했지만 nil vote와 장기 장애·네트워크 분할 시험은 더 필요합니다.
- snapshot chunk 저장·재개 코어는 구현됐지만 P2P 병렬 chunk 요청과 peer별 재시도는 아직 없습니다.
- 외부 signer 경계는 구현됐지만 제품별 PKCS#11/Vault/KMS adapter와 HA failover는 별도입니다.
- IP 단위 연결 속도 제한, subnet diversity, ASN 다양성은 아직 없습니다.
- embedded key-value backend는 작은 테스트넷용이며 메인넷 전 RocksDB/SQLite WAL급 backend가 필요합니다.

## 다음 단계

### Step 6-C: BFT 안전 규칙과 실서버 검증

- 외부 검증자의 이중투표 증거 저장·전파
- [완료 0.9.1] Tendermint 계열 locked value / valid value 안전 규칙
- [완료 0.9.1] propose/prevote/precommit 단계별 timeout과 라운드 변경
- [완료 0.9.1] 외부 검증자 이중투표 증거 영속 저장·전파
- [완료 0.9.1] 실제 4프로세스 QUIC 네트워크 통합 테스트

## 0.10.1 처리 범위

- [완료] snapshot chunk와 다운로드 재개 지점 영속화
- [완료] 최소 2~3개 피어의 tip·state root 교차 검증
- [완료] 작은 테스트넷용 embedded DB backend
- [완료] 외부 signer/HSM adapter 경계
- 검증자 4대 Docker/프로세스 통합 테스트와 장기 장애 시험

## 0.11.1 권장 범위

- [완료] snapshot P2P 병렬 chunk 할당과 peer별 timeout·재시도 코어
- [완료] 기존 embedded DB의 WAL 우선 복구·checkpoint
- [후속] nil vote/proposal과 round-change certificate의 합의·P2P 결합
- [완료] 외부 signer timeout·rate limit·HA failover
- [완료] Prometheus text metrics 수집 코어
- [후속] metrics HTTP endpoint·Grafana dashboard
- [후속] RocksDB/SQLite 실제 backend 선택과 migration 도구

### Step 7: 안전한 체인 동기화

- Status, GetHeaders, GetBlocks 요청/응답 프로토콜
- 헤더 우선 검증과 batch 크기 제한
- 상태 snapshot, state root, fast sync
- 오래된 블록 pruning과 archive node 분리
- 잘못된 체인을 제공한 피어 감점
- 간헐 접속 클라이언트용 최신 확정 상태 질의와 재연결 처리

### Step 8: PoS 운영

- 스테이킹·언스테이킹·위임
- validator set 변경은 epoch 경계에서만 반영
- double-sign 및 장시간 offline slashing
- 보상, 수수료 분배, 최소 stake
- governance 기반 네트워크 업그레이드

### Step 9: 라이트 클라이언트와 신원

- 웹·모바일용 헤더 검증
- Merkle proof 기반 잔액/거래 증명
- 여러 RPC 응답 교차 확인과 신뢰 가능한 체크포인트 고정
- 마지막 동기화 지점 이후의 변경분만 받는 증분 동기화
- 거래 전송 후 연결 종료 및 다음 접속 시 확정 여부 확인
- DID 문서, VC 발급기관, 폐기 registry
- 개인정보 원문은 체인 밖에 보관

## 메인넷 전 필수

외부 보안감사, Byzantine/fuzz/property 테스트, 경제 모델 검증, 키 복구,
업그레이드 절차, 재해복구 훈련과 관련 법률 검토가 모두 끝나야 합니다.
# v0.15.2 실제 저장 엔진

- [x] SQLite WAL 기반 실제 key-value 저장
- [x] 원자적 batch와 schema/generation metadata
- [x] v0.15.1 JSON image 자동 이관 및 원본 보존
- [x] WAL checkpoint 기반 backup
- [ ] 부하 측정 후 필요할 때 검증자 전용 RocksDB backend 검토
