use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkAssignment {
    pub peer_id: String,
    pub chunk_index: u32,
}

#[derive(Debug)]
struct InFlight {
    peer_id: String,
    started: Instant,
}

/// 누락 snapshot chunk를 여러 피어에 분산하고 timeout·실패 시 다른 피어로 재배정합니다.
#[derive(Debug)]
pub struct SnapshotScheduler {
    pending: VecDeque<u32>,
    peers: Vec<String>,
    in_flight: HashMap<u32, InFlight>,
    failures: HashMap<(String, u32), u32>,
    cursor: usize,
    timeout: Duration,
    max_in_flight: usize,
    max_peer_retries: u32,
}

impl SnapshotScheduler {
    pub fn new(
        missing: Vec<u32>,
        peers: Vec<String>,
        timeout: Duration,
        max_in_flight: usize,
        max_peer_retries: u32,
    ) -> Result<Self, String> {
        if peers.len() < 2 || max_in_flight == 0 || max_peer_retries == 0 {
            return Err(
                "snapshot 병렬 다운로드는 피어 2개 이상과 양수 제한값이 필요합니다.".into(),
            );
        }
        Ok(Self {
            pending: missing.into(),
            peers,
            in_flight: HashMap::new(),
            failures: HashMap::new(),
            cursor: 0,
            timeout,
            max_in_flight,
            max_peer_retries,
        })
    }

    pub fn assign(&mut self, now: Instant) -> Vec<ChunkAssignment> {
        self.requeue_timed_out(now);
        let mut assigned = Vec::new();
        while self.in_flight.len() < self.max_in_flight {
            let Some(chunk_index) = self.pending.pop_front() else {
                break;
            };
            let Some(peer_id) = self.next_peer(chunk_index) else {
                self.pending.push_back(chunk_index);
                break;
            };
            self.in_flight.insert(
                chunk_index,
                InFlight {
                    peer_id: peer_id.clone(),
                    started: now,
                },
            );
            assigned.push(ChunkAssignment {
                peer_id,
                chunk_index,
            });
        }
        assigned
    }

    pub fn complete(&mut self, peer_id: &str, chunk_index: u32) -> Result<(), String> {
        match self.in_flight.get(&chunk_index) {
            Some(in_flight) if in_flight.peer_id == peer_id => {
                self.in_flight.remove(&chunk_index);
                Ok(())
            }
            _ => Err("현재 할당과 다른 snapshot chunk 응답입니다.".into()),
        }
    }

    pub fn fail(&mut self, peer_id: &str, chunk_index: u32) {
        if self
            .in_flight
            .get(&chunk_index)
            .is_some_and(|in_flight| in_flight.peer_id == peer_id)
        {
            self.in_flight.remove(&chunk_index);
            *self
                .failures
                .entry((peer_id.to_string(), chunk_index))
                .or_default() += 1;
            self.pending.push_back(chunk_index);
        }
    }

    pub fn is_done(&self) -> bool {
        self.pending.is_empty() && self.in_flight.is_empty()
    }

    fn requeue_timed_out(&mut self, now: Instant) {
        let timed_out = self
            .in_flight
            .iter()
            .filter(|(_, value)| now.duration_since(value.started) >= self.timeout)
            .map(|(chunk, value)| (*chunk, value.peer_id.clone()))
            .collect::<Vec<_>>();
        for (chunk, peer) in timed_out {
            self.fail(&peer, chunk);
        }
    }

    fn next_peer(&mut self, chunk: u32) -> Option<String> {
        for _ in 0..self.peers.len() {
            let peer = self.peers[self.cursor % self.peers.len()].clone();
            self.cursor = self.cursor.wrapping_add(1);
            if self
                .failures
                .get(&(peer.clone(), chunk))
                .copied()
                .unwrap_or_default()
                < self.max_peer_retries
            {
                return Some(peer);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_out_chunk_moves_to_another_peer() {
        let now = Instant::now();
        let mut scheduler = SnapshotScheduler::new(
            vec![0],
            vec!["peer-a".into(), "peer-b".into()],
            Duration::from_secs(1),
            1,
            2,
        )
        .unwrap();
        assert_eq!(scheduler.assign(now)[0].peer_id, "peer-a");
        let retried = scheduler.assign(now + Duration::from_secs(2));
        assert_eq!(retried[0].peer_id, "peer-b");
        scheduler.complete("peer-b", 0).unwrap();
        assert!(scheduler.is_done());
    }
}
