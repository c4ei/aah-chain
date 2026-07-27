use ieum_chain::Blockchain;
use ieum_chain::consensus::{ConsensusPhase, Validator};
use ieum_chain::consensus_runtime::ConsensusRuntime;
use ieum_chain::model::Block;
use ieum_chain::wallet::Wallet;
use std::time::Duration;

fn setup() -> (Vec<ConsensusRuntime>, Vec<Wallet>) {
    let wallets: Vec<_> = (1_u8..=4).map(|n| Wallet::from_seed([n; 32])).collect();
    let validators = wallets
        .iter()
        .map(|w| Validator::new(w.address(), 100))
        .collect::<Vec<_>>();
    let nodes = wallets
        .iter()
        .cloned()
        .map(|wallet| {
            ConsensusRuntime::new(
                Blockchain::new(vec![]),
                validators.clone(),
                wallet,
                Duration::from_millis(50),
            )
            .unwrap()
        })
        .collect();
    (nodes, wallets)
}

#[test]
fn four_nodes_finalize_then_store_and_new_node_syncs() {
    let (mut nodes, _) = setup();
    let proposer_index = (0..nodes.len())
        .find(|&i| {
            nodes[i]
                .make_proposal(Block::new(
                    1,
                    nodes[i].chain.blocks[0].hash.clone(),
                    1,
                    "validator".into(),
                    vec![],
                ))
                .is_ok()
        })
        .unwrap();
    let block = Block::new(
        1,
        nodes[proposer_index].chain.blocks[0].hash.clone(),
        1,
        "validator".into(),
        vec![],
    );
    let proposal = nodes[proposer_index].make_proposal(block).unwrap();
    let prevotes = nodes
        .iter_mut()
        .map(|node| node.receive_proposal(proposal.clone()).unwrap())
        .collect::<Vec<_>>();
    assert!(nodes.iter().all(|node| node.chain.blocks.len() == 1));

    let mut precommits = Vec::new();
    for vote in prevotes {
        for node in &mut nodes {
            if let Some(precommit) = node.receive_vote(vote.clone()).unwrap() {
                precommits.push(precommit);
            }
        }
    }
    precommits.sort_by(|a, b| a.validator_id.cmp(&b.validator_id));
    precommits.dedup_by(|a, b| a.validator_id == b.validator_id);
    for vote in precommits.into_iter().take(3) {
        for node in &mut nodes {
            node.receive_vote(vote.clone()).unwrap();
        }
    }
    assert!(
        nodes
            .iter()
            .all(|node| node.phase() == ConsensusPhase::Finalized)
    );
    assert!(nodes.iter().all(|node| node.chain.blocks.len() == 2));
    assert!(
        nodes
            .windows(2)
            .all(|pair| pair[0].chain.blocks == pair[1].chain.blocks)
    );

    let (mut fresh, _) = setup();
    let blocks = nodes[0].blocks_from(1);
    assert_eq!(fresh[0].apply_sync_batch(blocks).unwrap(), 1);
    assert_eq!(fresh[0].chain.blocks, nodes[0].chain.blocks);
}

#[test]
fn timeout_changes_round_without_storing_candidate() {
    let (mut nodes, _) = setup();
    assert_eq!(nodes[0].force_timeout_for_test().unwrap(), 1);
    assert_eq!(nodes[0].chain.blocks.len(), 1);
}
