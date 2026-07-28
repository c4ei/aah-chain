use ieum_chain::model::Block;
use ieum_chain::{
    BftConsensus, ConsensusMessage, DoubleVoteEvidence, EvidenceStore, SignedProposal, Validator,
    Wallet,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn validators() -> (Vec<Wallet>, Vec<Validator>) {
    let wallets: Vec<_> = (1_u8..=4)
        .map(|index| Wallet::from_seed([index; 32]))
        .collect();
    let validators = wallets
        .iter()
        .map(|wallet| Validator::new(wallet.address(), 100))
        .collect();
    (wallets, validators)
}

#[test]
fn quorum_prevote_locks_and_preserves_valid_value_across_rounds() {
    let (wallets, validators) = validators();
    let mut consensus = BftConsensus::new(validators).unwrap();
    consensus.start_round(1, 0).unwrap();
    let proposer = consensus.expected_proposer().to_string();
    consensus.propose(&proposer, "block-a").unwrap();
    for wallet in wallets.iter().take(3) {
        consensus
            .handle(ConsensusMessage::prevote(1, 0, wallet, "block-a"))
            .unwrap();
    }
    assert_eq!(consensus.locked_value(), Some(("block-a", 0)));
    assert_eq!(consensus.valid_value(), Some(("block-a", 0)));

    consensus.on_timeout().unwrap();
    assert_eq!(consensus.locked_value(), Some(("block-a", 0)));
    assert_eq!(consensus.valid_value(), Some(("block-a", 0)));
}

#[test]
fn locked_node_rejects_different_value_without_valid_round() {
    let (wallets, validators) = validators();
    let mut consensus = BftConsensus::new(validators).unwrap();
    consensus.start_round(1, 0).unwrap();
    let proposer = consensus.expected_proposer().to_string();
    consensus.propose(&proposer, "block-a").unwrap();
    for wallet in wallets.iter().take(3) {
        consensus
            .handle(ConsensusMessage::prevote(1, 0, wallet, "block-a"))
            .unwrap();
    }
    consensus.on_timeout().unwrap();

    let expected = consensus.expected_proposer().to_string();
    let proposer_wallet = wallets
        .iter()
        .find(|wallet| wallet.address() == expected)
        .unwrap();
    let block = Block::new(1, Block::genesis().hash, 2, expected, vec![]);
    let proposal = SignedProposal::new(1, 1, proposer_wallet, block);
    assert!(consensus.handle_proposal(&proposal).is_err());
}

#[test]
fn double_vote_evidence_survives_restart_and_deduplicates() {
    let (wallets, _) = validators();
    let evidence = DoubleVoteEvidence::new(
        ConsensusMessage::prevote(8, 2, &wallets[0], "block-a"),
        ConsensusMessage::prevote(8, 2, &wallets[0], "block-b"),
    )
    .unwrap();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ieum-evidence-{unique}"));
    let store = EvidenceStore::new(&path);
    assert!(store.append(&evidence).unwrap());
    assert!(!store.append(&evidence).unwrap());
    assert_eq!(EvidenceStore::new(&path).load().unwrap(), vec![evidence]);
    fs::remove_dir_all(path).unwrap();
}
