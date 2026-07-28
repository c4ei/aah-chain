# 서버·일반 PC 실행 명령 분리

## 요청 사항

- 운영 서버 실행과 일반 PC 실행 명령을 명확히 구분한다.
- 운영 서버 재시작 후에도 PeerId가 유지되게 한다.
- 일반 PC는 `node.ieum.aah.name`을 통해 운영 서버에 연결한다.

## 처리 내용

- `server`와 `client` 하위 명령을 추가했다.
- `server`는 부트스트랩 피어 없이 외부 접속을 기다린다.
- `client`는 한 개 이상의 `--peer`를 필수로 받는다.
- 미사용 상태였던 영구 노드 키 기능을 P2P 네트워크에 연결했다.
- `--node-key` 기본값을 `data/node.key`로 지정했다.
- 서버와 일반 PC의 실행 예제를 `docs/IEUM_SERVER_CLIENT_RUN.md`에 작성했다.

## 주의 사항

- 운영 서버의 node key 파일은 유출하지 말고 반드시 백업한다.
- node key 파일을 잃어버리면 PeerId가 바뀌므로 일반 PC의 `--peer` 주소도 바꿔야 한다.
- 공개 RPC는 `rpc.ieum.aah.name` 게이트웨이를 사용하고 노드 RPC 포트는 직접 공개하지 않는다.
