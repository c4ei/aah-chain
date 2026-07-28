use ieum_chain::{Blockchain, Mempool, StateStore, UpgradeSchedule, Wallet};
use std::fs;

#[test]
fn mempool_replaces_same_nonce_only_with_higher_fee() {
    let sender = Wallet::from_seed([1; 32]);
    let receiver = Wallet::from_seed([2; 32]);
    let mut pool = Mempool::with_limits(10, 1024 * 1024);
    pool.add(sender.sign_transfer(receiver.address(), 10, 10, 0))
        .unwrap();
    assert!(
        pool.add(sender.sign_transfer(receiver.address(), 10, 10, 0))
            .is_err()
    );
    pool.add(sender.sign_transfer(receiver.address(), 10, 11, 0))
        .unwrap();
    assert_eq!(pool.len(), 1);
    assert_eq!(pool.drain(1)[0].fee, 11);
}

#[test]
fn canonical_state_contains_state_root_and_indexes() {
    let sender = Wallet::from_seed([3; 32]);
    let receiver = Wallet::from_seed([4; 32]);
    let producer = Wallet::from_seed([5; 32]);
    let mut chain = Blockchain::new(vec![(sender.address(), 100)]);
    chain
        .add_block(
            vec![sender.sign_transfer(receiver.address(), 20, 1, 0)],
            producer.address(),
        )
        .unwrap();
    let root = std::env::temp_dir().join(format!("ieum-state-{}", std::process::id()));
    let store = StateStore::new(&root).unwrap();
    let saved = store.commit(&chain).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.state_root, chain.state_hash());
    assert_eq!(loaded.block_hash, chain.tip_hash());
    assert_eq!(saved.transaction_index.len(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn unsupported_protocol_height_stops_node() {
    let root = std::env::temp_dir().join(format!("ieum-upgrade-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("upgrades.json");
    fs::write(
        &path,
        r#"{"upgrades":[{"name":"v2","activation_height":100,"protocol_version":2}]}"#,
    )
    .unwrap();
    let schedule = UpgradeSchedule::load(&path).unwrap();
    assert!(schedule.ensure_supported(99, 1).is_ok());
    assert!(schedule.ensure_supported(100, 1).is_err());
    let _ = fs::remove_dir_all(root);
}
