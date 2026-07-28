use ieum_chain::{
    Blockchain, EmbeddedDb, SnapshotDownload, SnapshotManifest, StateStore, SyncTip, TipQuorum,
};

#[test]
fn embedded_state_is_primary_restart_source() {
    let root = std::env::temp_dir().join(format!("ieum-v0101-state-{}", std::process::id()));
    let chain = Blockchain::with_chain_id(21_004, vec![("alice".into(), 1_000)]);
    let store = StateStore::new(&root).unwrap();
    store.commit(&chain).unwrap();
    let restored = store.load().unwrap().unwrap();
    assert_eq!(restored.chain_id, 21_004);
    assert_eq!(restored.state_root, chain.state_hash());
    assert!(EmbeddedDb::open(&root)
        .unwrap()
        .get("canonical/current")
        .is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn snapshot_resume_survives_reopen() {
    let root = std::env::temp_dir().join(format!("ieum-v0101-snapshot-{}", std::process::id()));
    let chain = Blockchain::with_chain_id(21_004, vec![("alice".into(), 1_000)]);
    let snapshot = ieum_chain::archive::StateSnapshot::from_chain(&chain);
    let (manifest, chunks) = SnapshotManifest::build(&snapshot, 16).unwrap();
    let mut download = SnapshotDownload::open(&root, manifest.clone()).unwrap();
    download.accept(chunks[0].clone()).unwrap();
    drop(download);
    let resumed = SnapshotDownload::open(&root, manifest).unwrap();
    assert!(!resumed.missing().contains(&0));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn one_peer_cannot_choose_sync_tip() {
    let tip = SyncTip {
        height: 42,
        block_hash: "block".into(),
        state_root: "state".into(),
    };
    let mut quorum = TipQuorum::new(2).unwrap();
    assert!(quorum.observe("peer-1", tip.clone()).is_none());
    assert_eq!(quorum.observe("peer-2", tip.clone()), Some(tip));
}
