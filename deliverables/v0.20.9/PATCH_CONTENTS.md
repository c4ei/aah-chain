# IEUM Chain v0.20.9 변경 파일

이 묶음은 v0.20.8 소스 위에 덮어쓰는 변경 파일입니다.

- `Cargo.toml`, `Cargo.lock`: 버전 0.20.9
- `src/recovery.rs`: 검증자 수 또는 투표권 3/4 복구 승인 판정
- `src/lib.rs`: 복구 승인 API 공개
- `src/main.rs`: `recovery policy` 명령
- `README.md`, `CHANGELOG.md`: v0.20.9 안내와 변경 기록
- `docs/USER_MANUAL_0.20.9.md`: 사용자·운영자 사고 복구 매뉴얼
- `docs/VERSION_0.20.9.md`: 버전별 설계 및 안전 기준

## 적용 후 검증

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
./target/release/ieum-chain recovery policy
```

현재 변경은 복구 승인 판정 기반과 운영 정책을 추가합니다. 강제 차감·동결·체크포인트
적용 명령은 복구 계획 서명 검증과 원자적 적용 경로가 완성되기 전까지 제공하지 않습니다.
