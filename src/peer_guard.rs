use std::collections::HashMap;
use std::time::{Duration, Instant};

const BAN_SCORE: i32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerDecision {
    Allow,
    TemporarilyBlocked,
}

#[derive(Clone, Debug)]
struct PeerRecord {
    score: i32,
    blocked_until: Option<Instant>,
}

/// 잘못된 메시지와 반복 실패를 점수화해 악성 피어를 임시 차단합니다.
/// 영구 차단보다 NAT 공유 환경에서 실수로 정상 사용자를 막을 가능성이 작습니다.
#[derive(Clone, Debug)]
pub struct PeerGuard {
    peers: HashMap<String, PeerRecord>,
    ban_duration: Duration,
}

impl PeerGuard {
    pub fn new(ban_duration: Duration) -> Self {
        Self {
            peers: HashMap::new(),
            ban_duration,
        }
    }

    pub fn check(&mut self, peer: &str) -> PeerDecision {
        let Some(record) = self.peers.get_mut(peer) else {
            return PeerDecision::Allow;
        };
        if let Some(until) = record.blocked_until {
            if Instant::now() < until {
                return PeerDecision::TemporarilyBlocked;
            }
            record.blocked_until = None;
            record.score = 0;
        }
        PeerDecision::Allow
    }

    pub fn penalize(&mut self, peer: &str, points: i32) -> PeerDecision {
        let record = self.peers.entry(peer.to_string()).or_insert(PeerRecord {
            score: 0,
            blocked_until: None,
        });
        record.score = record.score.saturating_add(points.max(0));
        if record.score >= BAN_SCORE {
            record.blocked_until = Some(Instant::now() + self.ban_duration);
            PeerDecision::TemporarilyBlocked
        } else {
            PeerDecision::Allow
        }
    }

    pub fn reward(&mut self, peer: &str) {
        if let Some(record) = self.peers.get_mut(peer) {
            record.score = (record.score - 1).max(0);
        }
    }
}
