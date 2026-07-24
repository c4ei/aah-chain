use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MONTH_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardSplit {
    pub validator_bps: u16,
    pub participant_bps: u16,
    pub treasury_bps: u16,
}

impl RewardSplit {
    pub fn validate(&self) -> Result<(), String> {
        if u32::from(self.validator_bps)
            + u32::from(self.participant_bps)
            + u32::from(self.treasury_bps)
            != 10_000
        {
            return Err("보상 배분 합계는 10,000bp(100%)여야 합니다.".into());
        }
        Ok(())
    }
}

/// PoW 해시 난이도 대신 네트워크 참여 목표와 보상 곡선을 정의합니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParticipationPolicy {
    pub target_active_nodes: u32,
    pub minimum_uptime_bps: u16,
    pub reward_per_block: u64,
    pub reward_split: RewardSplit,
}

impl ParticipationPolicy {
    /// 활성 노드가 목표보다 적으면 보상을 높이고, 많으면 완만히 낮춥니다.
    /// 한 달에 최대 ±12.5%만 변해 급격한 발행량 변화를 막습니다.
    pub fn adjusted_reward(&self, active_nodes: u32) -> u64 {
        let target = self.target_active_nodes.max(1) as u128;
        let active = active_nodes.max(1) as u128;
        let ratio_bps = (target * 10_000 / active).clamp(8_750, 11_250);
        (u128::from(self.reward_per_block) * ratio_bps / 10_000) as u64
    }
}

#[derive(Clone, Debug)]
pub struct Governance {
    epoch_started_at: u64,
    voting_power: HashMap<String, u64>,
    votes: HashMap<String, (ParticipationPolicy, u64)>,
}

impl Governance {
    pub fn new(epoch_started_at: u64, validators: impl IntoIterator<Item = (String, u64)>) -> Self {
        Self {
            epoch_started_at,
            voting_power: validators.into_iter().collect(),
            votes: HashMap::new(),
        }
    }

    pub fn vote(
        &mut self,
        now: u64,
        validator: &str,
        proposal: ParticipationPolicy,
    ) -> Result<(), String> {
        if now < self.epoch_started_at + MONTH_SECONDS {
            return Err("정책 투표는 30일 epoch가 끝난 뒤 진행합니다.".into());
        }
        proposal.reward_split.validate()?;
        let power = self
            .voting_power
            .get(validator)
            .copied()
            .ok_or("등록 검증자가 아닙니다.")?;
        self.votes.insert(validator.to_string(), (proposal, power));
        Ok(())
    }

    /// 동일 정책에 전체 투표권의 2/3 초과가 모여야 다음 달 정책으로 확정합니다.
    pub fn finalize(&self) -> Option<ParticipationPolicy> {
        let total: u64 = self.voting_power.values().sum();
        let mut candidates: Vec<(ParticipationPolicy, u64)> = Vec::new();
        for (policy, power) in self.votes.values() {
            if let Some((_, sum)) = candidates.iter_mut().find(|(item, _)| item == policy) {
                *sum += *power;
            } else {
                candidates.push((policy.clone(), *power));
            }
        }
        candidates
            .into_iter()
            .find(|(_, power)| power.saturating_mul(3) > total.saturating_mul(2))
            .map(|(policy, _)| policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ParticipationPolicy {
        ParticipationPolicy {
            target_active_nodes: 100,
            minimum_uptime_bps: 9_000,
            reward_per_block: 1_000,
            reward_split: RewardSplit {
                validator_bps: 5_000,
                participant_bps: 4_000,
                treasury_bps: 1_000,
            },
        }
    }

    #[test]
    fn reward_curve_is_bounded() {
        assert_eq!(policy().adjusted_reward(1), 1_125);
        assert_eq!(policy().adjusted_reward(1_000), 875);
    }

    #[test]
    fn monthly_supermajority_finalizes_policy() {
        let mut governance =
            Governance::new(0, [("a".into(), 40), ("b".into(), 35), ("c".into(), 25)]);
        governance.vote(MONTH_SECONDS, "a", policy()).unwrap();
        governance.vote(MONTH_SECONDS, "b", policy()).unwrap();
        assert_eq!(governance.finalize(), Some(policy()));
    }
}
