# Block Explorer 연동

Explorer는 HTTP JSON-RPC `127.0.0.1:8545`에 연결합니다. 공개 인터넷에 RPC를 직접
노출하지 말고 Explorer 백엔드 또는 인증된 reverse proxy만 접근하게 하십시오.

지원 메서드:

- `eth_chainId`, `eth_blockNumber`
- `eth_getBalance`, `eth_getTransactionCount`
- `eth_getBlockByNumber(number, fullTransactions)`
- `eth_getBlockByHash(hash, fullTransactions)`
- `eth_getTransactionByHash`
- `eth_getTransactionReceipt`
- `ieum_getStorageStatus`

예:

```bash
curl -s http://127.0.0.1:8545 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_getBlockByNumber","params":["latest",true]}'
```

현재 RPC는 메모리에 적재된 블록을 조회합니다. `backup/*.jsonl`을 직접 읽는
`ArchiveStore::read_backup_blocks()`도 제공하므로 독립 Explorer 인덱서가 이를
PostgreSQL/SQLite에 적재할 수 있습니다. 대규모 운영에서는 RPC가 매 요청마다 백업
파일을 전수 검색하지 않도록 별도 인덱서 사용을 권장합니다.
