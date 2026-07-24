# geth / EVM 스크립트 호환 범위

## 목적

v0.0.3은 기존 geth/web3 운영 스크립트 가운데 주소 생성, 계정 목록, 잔액,
nonce와 관리형 계정 송금을 먼저 호환합니다. HTTP JSON-RPC 기본 주소는
`http://127.0.0.1:8545`입니다.

AAH 원장은 현재 Ed25519를 사용합니다. RPC의 `0x` 20바이트 주소는 기존
공개키 주소에 연결되는 별칭입니다. 따라서 Ethereum 메인넷 주소 체계와
겉모양은 같지만 암호학적으로 동일한 계정 형식은 아닙니다.

## 지원 메서드

| 메서드 | 상태 | 설명 |
|---|---|---|
| `web3_clientVersion` | 지원 | AAH 노드 버전 |
| `net_version` | 지원 | 10진수 chain ID |
| `net_listening`, `net_peerCount` | 지원 | 네트워크 기본 상태 |
| `rpc_modules` | 지원 | 활성 namespace 목록 |
| `eth_chainId` | 지원 | hex chain ID |
| `eth_syncing` | 지원 | 현재는 `false` |
| `eth_blockNumber` | 지원 | 최신 확정 높이 |
| `eth_accounts` | 지원 | 노드 관리형 계정 |
| `eth_coinbase` | 지원 | 개발용 faucet 계정 |
| `personal_newAccount` | 개발용 지원 | 암호는 아직 저장하지 않음 |
| `personal_unlockAccount` | 개발용 지원 | 관리형 계정 여부만 확인 |
| `eth_getBalance` | 지원 | `latest` 조회 |
| `eth_getTransactionCount` | 지원 | 다음 nonce |
| `eth_gasPrice`, `eth_estimateGas` | 임시 지원 | v0.0.3 고정 개발값 |
| `eth_getCode` | 지원 | 계약 미지원이므로 `0x` |
| `eth_sendTransaction` | 개발용 지원 | 관리형 계정이 서명하고 즉시 소형 블록 생성 |
| `personal_sendTransaction` | 개발용 지원 | `eth_sendTransaction`과 동일 경로 |
| `eth_sendRawTransaction` | 미지원 | RLP/secp256k1 도입 후 구현 |

## curl 사용 예

계정 목록:

```bash
curl -s http://127.0.0.1:8545 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_accounts","params":[]}'
```

주소 생성:

```bash
curl -s http://127.0.0.1:8545 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":2,"method":"personal_newAccount","params":["개발용암호"]}'
```

잔액 조회:

```bash
curl -s http://127.0.0.1:8545 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":3,"method":"eth_getBalance","params":["0x주소","latest"]}'
```

송금:

```bash
curl -s http://127.0.0.1:8545 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":4,"method":"eth_sendTransaction","params":[{"from":"0x보내는주소","to":"0x받는주소","value":"0x64","gasPrice":"0x1"}]}'
```

모든 수량은 Ethereum JSON-RPC 관례대로 `0x` hex quantity입니다.

## 기존 geth JavaScript 예

```javascript
const Web3 = require("web3");
const web3 = new Web3("http://127.0.0.1:8545");

const accounts = await web3.eth.getAccounts();
const balance = await web3.eth.getBalance(accounts[0]);
console.log(accounts[0], balance);
```

web3.js 버전에 따라 `personal_newAccount`는 provider의 직접 RPC 호출 또는
`web3.eth.personal.newAccount()`를 사용합니다.

## 보안 주의

- RPC 기본 리스닝 주소는 localhost입니다.
- v0.0.3의 계정은 메모리 기반이라 노드 재시작 후 복구되지 않습니다.
- `personal_newAccount`의 암호는 v0.0.3에서 실제 암호화에 사용되지 않습니다.
- 테스트 코인만 사용하세요.
- 외부 공개 RPC에는 인증, TLS reverse proxy, 요청 속도 제한과 CORS 정책이
  추가되기 전까지 개인키 관리 메서드를 노출하면 안 됩니다.
