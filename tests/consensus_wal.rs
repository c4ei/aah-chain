use aah_chain::{ConsensusMessage, ConsensusWal, Wallet};
use std::fs;

#[test]
fn wal_recovers_vote_and_blocks_double_sign() {
    let validator = Wallet::from_seed([7; 32]);
    let path = std::env::temp_dir().join(format!(
        "aah-consensus-wal-{}-{}.jsonl",
        std::process::id(),
        validator.address()
    ));
    let _ = fs::remove_file(&path);

    let first = ConsensusMessage::prevote(10, 2, &validator, "block-a");
    let conflicting = ConsensusMessage::prevote(10, 2, &validator, "block-b");

    {
        let mut wal = ConsensusWal::open(&path).unwrap();
        wal.record_before_broadcast(&first).unwrap();
    }

    // 프로세스를 재시작한 것처럼 WAL을 다시 열어도 과거 투표를 기억해야 합니다.
    let mut recovered = ConsensusWal::open(&path).unwrap();
    assert!(recovered.record_before_broadcast(&conflicting).is_err());
    recovered.record_before_broadcast(&first).unwrap();

    let _ = fs::remove_file(path);
}
