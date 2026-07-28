use ieum_chain::{PeerDecision, PeerGuard};
use std::time::Duration;

#[test]
fn repeated_bad_messages_temporarily_block_peer() {
    let mut guard = PeerGuard::new(Duration::from_secs(60));
    assert_eq!(guard.penalize("peer-a", 25), PeerDecision::Allow);
    assert_eq!(
        guard.penalize("peer-a", 75),
        PeerDecision::TemporarilyBlocked
    );
    assert_eq!(guard.check("peer-a"), PeerDecision::TemporarilyBlocked);
}
