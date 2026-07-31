# IEUM Chain v0.20.6 자동 복구 및 검증자 잔액 이전

## 요청

- `clean`, `init` 또는 키 재생성 뒤 PeerId 설정 불일치로 재시작 루프에 빠지지 않게 한다.
- 사용자가 장애 원인을 몰라도 한 명령으로 점검·복구할 수 있게 한다.
- 일반 정리에서는 노드 신원과 검증자 키를 보존한다.
- 기존 검증자 키가 소유한 잔액을 특정 지갑으로 옮길 수 있게 한다.
- GitHub Actions의 실제 4프로세스 BFT 테스트 실패를 수정한다.

## 처리

### 시작 시 네트워크 자동 복구

서버는 `data/server.node.key`에서 현재 PeerId를 계산합니다.
`config/network.json`의 자기 `advertise_address`에 과거 PeerId가 남아 있으면 현재
PeerId로 자동 교체하고 설정을 저장한 뒤 계속 기동합니다. 이 불일치만으로 systemd
재시작 루프에 빠지지 않습니다.

다른 서버의 bootstrap 항목은 자기 설정이 아니므로 임의로 변경하지 않습니다.

### 쉬운 장애 처리 명령

```bash
./ieum-chain node doctor
```

키, 초기화 표시, 원장 폴더, 검증자 설정과 광고 주소를 점검하고 안전하게 복구합니다.

원장이 손상되었거나 처음부터 다시 동기화해야 할 때:

```bash
sudo systemctl stop ieum-chain
cd /opt/ieum-chain
sudo ./ieum-chain node clean --yes
sudo systemctl start ieum-chain
```

`node clean`은 기존 원장을 `backups/ledger-clean-*`으로 이동하므로 되돌릴 수 있습니다.
다음 파일은 삭제하거나 재생성하지 않습니다.

- `config/validator.key`
- `data/server.node.key`
- `data/.ieum-initialized`

### 검증자 잔액 이전

RPC 노드가 실행 중인 상태에서 기존 `validator.key`로 소유권을 증명하고 전송합니다.

잔액 전체에서 수수료를 제외하고 이전:

```bash
./ieum-chain validator-key transfer \
  --to <받을_IEUM_주소> \
  --amount all
```

일부만 이전:

```bash
./ieum-chain validator-key transfer \
  --to <받을_IEUM_주소> \
  --amount 100 \
  --fee 0.000001
```

다른 키 또는 RPC 포트를 사용할 수도 있습니다.

```bash
./ieum-chain validator-key transfer \
  --key /백업/validator.key \
  --rpc-port 8989 \
  --to <받을_IEUM_주소> \
  --amount all
```

공개키만으로는 송금할 수 없습니다. 반드시 그 공개키에 대응하는 `validator.key`로
서명해야 하며, 이 제한은 타인의 잔액을 임의 이전하지 못하게 하는 필수 보안 규칙입니다.

### GitHub Actions 4프로세스 테스트

기존 테스트는 네 노드의 원장 경로를 같은 부모 폴더 바로 아래에 배치해
`.ieum-initialized` 표시 파일을 공유했습니다. 1번 노드가 표시를 만든 뒤 2~4번
노드는 자기 `validator.key`가 없는데도 기존 설치로 오인하여 종료했습니다.

각 노드가 다음처럼 완전히 독립된 상태 폴더를 사용하도록 수정했습니다.

```text
node-1/ledger, node-1/validator.key, node-1/server.node.key
node-2/ledger, node-2/validator.key, node-2/server.node.key
node-3/ledger, node-3/validator.key, node-3/server.node.key
node-4/ledger, node-4/validator.key, node-4/server.node.key
```

## 안전 원칙

- 자동 복구는 자기 광고 주소만 현재 키에 맞춥니다.
- 일반 clean은 검증자 키와 노드 키를 보존합니다.
- 원장은 삭제하지 않고 먼저 백업 폴더로 이동합니다.
- 잔액 이전은 기존 검증자 개인키의 유효한 서명을 요구합니다.
- 키가 실제로 분실된 경우 공개키만으로 잔액을 이동시키는 우회 기능은 제공하지 않습니다.
