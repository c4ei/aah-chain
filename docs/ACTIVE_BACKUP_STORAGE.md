# 활성 블록과 월별 백업

## 설정의 정확한 의미

```json
{
  "max_block_bytes": 1048576,
  "max_active_block_bytes": 99000000
}
```

- `max_block_bytes`: 블록 **한 개**의 최대 직렬화 크기(기본 1 MiB)
- `max_active_block_bytes`: `data/ledger/active`에 있는 **활성 블록 전체 합계**의 최대 크기(99 MB)

기존 `max_segment_bytes` 이름은 과거 설정을 읽기 위한 별칭으로만 남겼습니다.

## 잔액 계산

매번 제네시스부터 재실행하지 않습니다. 월이 바뀌거나 활성 블록 합계가 제한에
도달하면 다음 정보가 체크포인트에 원자적으로 저장됩니다.

- 마지막 확정 높이와 블록 해시
- 전체 계정 잔액과 다음 nonce
- 결정론적 `state_hash`
- chain ID

그 다음 기간은 이 상태를 기준으로 새 활성 블록만 적용할 수 있습니다. 백업 블록은
Explorer, 감사, 전체 재검증용이며 일반 노드의 현재 잔액 조회에 필수 파일이 아닙니다.
체크포인트는 향후 검증자 2/3 서명 또는 상태 증명을 결합해야 공개 운영망 수준의
신뢰 기준이 됩니다.

## 파일 구조와 보존

```text
data/ledger/
  active/
    period
    blocks.jsonl
  checkpoints/
    202607M-h000000001234.json
  backup/
    202607M.jsonl
    202606M.jsonl
    2025.jsonl
```

- 현재 기간: `active/`만 필수
- 최근·직전 연도: `YYYYMM M.jsonl` 월 단위
- 전전년도 이전: `compact_old_backups()` 실행 시 `YYYY.jsonl` 연 단위
- 같은 달에 99 MB를 채우면 `YYYYMM M-part02.jsonl`처럼 분할
- 모바일: 기본적으로 체크포인트와 활성 블록만 보관, 백업은 사용자가 선택
- Explorer 서버: 백업을 보관해 과거 블록/거래 검색 제공

운영 중 데이터 삭제는 체크포인트가 다른 검증자와 일치하고 백업 복제본이 있는지
확인한 뒤 수행해야 합니다.
