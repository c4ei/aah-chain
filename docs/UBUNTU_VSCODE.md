# Ubuntu + VS Code 실행

## 처음 한 번

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup component add rustfmt clippy
code .
```

VS Code가 추천하는 `rust-analyzer` 확장을 설치합니다.

## 확인 순서

```bash
cargo fmt
cargo test
cargo run -- --port 7001
```

첫 libp2p 빌드는 의존성이 많아 시간이 걸릴 수 있지만 두 번째부터는 변경된 부분만
다시 빌드합니다. QUIC는 UDP이므로 UFW를 사용한다면 다음처럼 허용합니다.

```bash
sudo ufw allow 7001/udp
```

컴파일 오류를 문의할 때는 아래 결과를 함께 보내주세요.

```bash
rustc --version
cargo --version
cargo test
```
