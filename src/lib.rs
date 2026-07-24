pub mod chain;
pub mod consensus;
pub mod consensus_wal;
pub mod checkpoint;
pub mod mempool;
pub mod model;
pub mod network;
pub mod peer_guard;
pub mod storage;
pub mod wallet;

pub use chain::Blockchain;
pub use checkpoint::Checkpoint;
pub use consensus::{BftConsensus, ConsensusMessage, ConsensusPhase, Validator};
pub use consensus_wal::ConsensusWal;
pub use mempool::Mempool;
pub use network::{NetworkCommand, NetworkConfig, P2pNode};
pub use peer_guard::{PeerDecision, PeerGuard};
pub use wallet::Wallet;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mempool_fee_save_and_load() {
        let alice = Wallet::from_seed([1; 32]);
        let bob = Wallet::from_seed([2; 32]);
        let producer = Wallet::from_seed([3; 32]);
        let mut chain = Blockchain::new(vec![(alice.address(), 1_000)]);
        let mut pool = Mempool::default();
        pool.add(alice.sign_transfer(bob.address(), 250, 3, 0)).unwrap();
        chain.add_block(pool.drain(100), producer.address()).unwrap();

        assert_eq!(chain.balance_of(&alice.address()), 747);
        assert_eq!(chain.balance_of(&bob.address()), 250);
        assert_eq!(chain.balance_of(&producer.address()), 3);
        assert!(chain.verify_and_rebuild().is_ok());
    }

    #[test]
    fn duplicate_transaction_is_rejected() {
        let alice = Wallet::from_seed([1; 32]);
        let bob = Wallet::from_seed([2; 32]);
        let tx = alice.sign_transfer(bob.address(), 10, 1, 0);
        let mut pool = Mempool::default();
        pool.add(tx.clone()).unwrap();
        assert!(pool.add(tx).is_err());
    }

    #[test]
    fn received_block_keeps_original_hash() {
        let alice = Wallet::from_seed([1; 32]);
        let bob = Wallet::from_seed([2; 32]);
        let validator = Wallet::from_seed([3; 32]);
        let initial = vec![(alice.address(), 1_000)];
        let mut sender = Blockchain::new(initial.clone());
        let tx = alice.sign_transfer(bob.address(), 100, 1, 0);
        let block = sender.add_block(vec![tx], validator.address()).unwrap().clone();

        let mut receiver = Blockchain::new(initial);
        receiver.apply_block(block.clone()).unwrap();
        assert_eq!(receiver.blocks.last().unwrap().hash, block.hash);
        assert_eq!(receiver.balance_of(&bob.address()), 100);
    }

    #[test]
    fn bft_finalizes_after_two_thirds_precommit() {
        let keys: Vec<_> = (1..=4).map(|n| Wallet::from_seed([n; 32])).collect();
        let validators = keys
            .iter()
            .map(|key| Validator::new(key.address(), 100))
            .collect();
        let mut bft = BftConsensus::new(validators).unwrap();
        bft.start_round(5, 0).unwrap();
        let proposer = bft.expected_proposer().to_string();
        bft.propose(&proposer, "block-hash").unwrap();

        for key in keys.iter().take(3) {
            bft.handle(ConsensusMessage::prevote(
                5,
                0,
                key,
                "block-hash",
            ))
            .unwrap();
        }
        for key in keys.iter().take(3) {
            bft.handle(ConsensusMessage::precommit(
                5,
                0,
                key,
                "block-hash",
            ))
            .unwrap();
        }

        assert_eq!(bft.finalized_hash(), Some("block-hash"));
    }

    #[test]
    fn forged_consensus_vote_is_rejected() {
        let validator = Wallet::from_seed([1; 32]);
        let attacker = Wallet::from_seed([9; 32]);
        let mut message = ConsensusMessage::prevote(1, 0, &validator, "block-a");
        // b"..." 바이트 문자열은 ASCII만 허용한다.
        // 한글처럼 UTF-8 문자는 일반 문자열을 바이트 슬라이스로 변환해 전달한다.
        message.signature = attacker.sign_bytes("위조 서명".as_bytes());
        assert!(message.verify().is_err());
    }

    #[test]
    fn validator_double_vote_is_rejected() {
        let keys: Vec<_> = (1..=4).map(|n| Wallet::from_seed([n; 32])).collect();
        let validators = keys
            .iter()
            .map(|key| Validator::new(key.address(), 100))
            .collect();
        let mut bft = BftConsensus::new(validators).unwrap();
        bft.start_round(1, 0).unwrap();
        let proposer = bft.expected_proposer().to_string();
        bft.propose(&proposer, "block-a").unwrap();

        bft.handle(ConsensusMessage::prevote(1, 0, &keys[0], "block-a"))
            .unwrap();
        assert!(
            bft.handle(ConsensusMessage::prevote(1, 0, &keys[0], "block-b"))
                .is_err()
        );
    }
}
