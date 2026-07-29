use crate::consensus::Validator;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const BPS_DENOMINATOR: u128 = 10_000;

/// `node.ieum.aah.name` 등록 창구에 제출되고 체인에 확정되는 검증자 후보 정보입니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorCandidate {
    pub validator_id: String,
    pub stake_account: String,
    /// 원장에 잠긴 위임 지분입니다. 단순 잔액 위임은 인정하지 않습니다.
    #[serde(default)]
    pub delegated_stake: u128,
    #[serde(default)]
    pub country_code: Option<String>,
    /// DID/VC 또는 거버넌스 확인으로 후보의 국가가 인증됐는지 여부입니다.
    #[serde(default)]
    pub identity_country_verified: bool,
    /// 지분 계좌가 validator_id와 node_endpoint 등록 내용에 서명했는지 여부입니다.
    #[serde(default)]
    pub ownership_proof_verified: bool,
    /// 등록 서버가 QUIC challenge로 외부 접속을 확인한 주소입니다.
    #[serde(default)]
    pub node_endpoint: Option<String>,
    #[serde(default)]
    pub observed_epochs: u64,
    /// 최근 관측 epoch의 정상 응답률. 10,000 = 100%.
    #[serde(default)]
    pub uptime_bps: u16,
    #[serde(default)]
    pub administrator_approved: bool,
    #[serde(default)]
    pub blocked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorPolicy {
    /// 유통량 대비 최소 보유·위임 비율. 100 = 1%.
    #[serde(default = "default_min_stake_bps")]
    pub min_stake_bps: u16,
    /// 국가가 인증된 후보 중 국가별 코인 보유 순위 제한.
    #[serde(default = "default_country_rank_limit")]
    pub country_rank_limit: u16,
    #[serde(default = "default_max_validators")]
    pub max_validators: usize,
    #[serde(default = "default_min_validators")]
    pub min_validators: usize,
    /// 기존 검증자의 최소 정상 응답률. 신규 후보는 첫 epoch 동안 유예합니다.
    #[serde(default = "default_min_uptime_bps")]
    pub min_uptime_bps: u16,
}

impl Default for ValidatorPolicy {
    fn default() -> Self {
        Self {
            min_stake_bps: 100,
            country_rank_limit: 50,
            max_validators: 50,
            min_validators: 4,
            min_uptime_bps: 9_500,
        }
    }
}

fn default_min_stake_bps() -> u16 {
    100
}
fn default_country_rank_limit() -> u16 {
    50
}
fn default_max_validators() -> usize {
    50
}
fn default_min_validators() -> usize {
    4
}
fn default_min_uptime_bps() -> u16 {
    9_500
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedValidator {
    pub validator: Validator,
    pub stake: u128,
}

impl ValidatorPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.min_stake_bps == 0 || u128::from(self.min_stake_bps) > BPS_DENOMINATOR {
            return Err("최소 지분 비율은 1~10,000 bps여야 합니다.".into());
        }
        if self.country_rank_limit == 0 {
            return Err("국가 순위 제한은 1 이상이어야 합니다.".into());
        }
        if self.min_validators < 4 || self.max_validators < self.min_validators {
            return Err("검증자는 최소 4명이고 최대 인원은 최소 인원 이상이어야 합니다.".into());
        }
        if self.min_uptime_bps > 10_000 {
            return Err("최소 정상 응답률은 0~10,000 bps여야 합니다.".into());
        }
        Ok(())
    }

    /// 같은 확정 잔액과 후보 집합을 받은 모든 노드가 같은 검증자 집합을 계산합니다.
    ///
    /// 자격 경로는 1% 지분, 국가별 보유량 상위 50위, 관리자 승인 중 하나입니다.
    /// 위임 지분도 포함하며, 소유권 서명·외부 QUIC 접속·가동률은 공통 필수 조건입니다.
    pub fn select(
        &self,
        candidates: &[ValidatorCandidate],
        balances: &HashMap<String, u128>,
        circulating_supply: u128,
    ) -> Result<Vec<SelectedValidator>, String> {
        self.validate()?;
        if circulating_supply == 0 {
            return Err("유통량은 0일 수 없습니다.".into());
        }

        let mut validator_ids = HashSet::new();
        let mut stake_accounts = HashSet::new();
        let mut eligible = Vec::new();
        for candidate in candidates {
            validate_candidate(candidate)?;
            if !validator_ids.insert(candidate.validator_id.clone()) {
                return Err("중복된 검증자 공개키가 있습니다.".into());
            }
            let account = normalize_account(&candidate.stake_account);
            if !stake_accounts.insert(account.clone()) {
                return Err("하나의 지분 계좌를 여러 검증자 후보가 사용할 수 없습니다.".into());
            }
            if candidate.blocked
                || !candidate.ownership_proof_verified
                || candidate.node_endpoint.is_none()
                || (candidate.observed_epochs > 0 && candidate.uptime_bps < self.min_uptime_bps)
            {
                continue;
            }
            let stake = balances
                .get(&account)
                .copied()
                .unwrap_or(0)
                .saturating_add(candidate.delegated_stake);
            let has_minimum_stake = stake.saturating_mul(BPS_DENOMINATOR)
                >= circulating_supply.saturating_mul(u128::from(self.min_stake_bps));
            eligible.push((candidate, stake, has_minimum_stake));
        }

        let mut country_ranked = HashSet::new();
        let mut countries: HashMap<&str, Vec<(&ValidatorCandidate, u128)>> = HashMap::new();
        for (candidate, stake, _) in &eligible {
            if candidate.identity_country_verified
                && let Some(country) = candidate.country_code.as_deref()
            {
                countries
                    .entry(country)
                    .or_default()
                    .push((candidate, *stake));
            }
        }
        for ranked in countries.values_mut() {
            ranked.sort_by(|(left, left_stake), (right, right_stake)| {
                right_stake
                    .cmp(left_stake)
                    .then_with(|| left.validator_id.cmp(&right.validator_id))
            });
            for (candidate, _) in ranked.iter().take(usize::from(self.country_rank_limit)) {
                country_ranked.insert(candidate.validator_id.as_str());
            }
        }

        let mut selected: Vec<_> = eligible
            .into_iter()
            .filter(|(candidate, _, has_minimum_stake)| {
                *has_minimum_stake
                    || country_ranked.contains(candidate.validator_id.as_str())
                    || candidate.administrator_approved
            })
            .map(|(candidate, stake, _)| SelectedValidator {
                validator: Validator {
                    id: candidate.validator_id.clone(),
                    voting_power: voting_power(stake, circulating_supply),
                },
                stake,
            })
            .collect();
        selected.sort_by(|left, right| {
            right
                .stake
                .cmp(&left.stake)
                .then_with(|| left.validator.id.cmp(&right.validator.id))
        });
        selected.truncate(self.max_validators);
        if selected.len() < self.min_validators {
            return Err(format!(
                "선발된 검증자가 {}명입니다. BFT 실행에는 최소 {}명이 필요합니다.",
                selected.len(),
                self.min_validators
            ));
        }
        Ok(selected)
    }
}

fn validate_candidate(candidate: &ValidatorCandidate) -> Result<(), String> {
    if hex::decode(&candidate.validator_id)
        .map(|bytes| bytes.len() != 32)
        .unwrap_or(true)
    {
        return Err("검증자 ID는 32바이트 Ed25519 공개키 hex여야 합니다.".into());
    }
    let account = candidate.stake_account.trim_start_matches("0x");
    if account.len() != 40 || hex::decode(account).is_err() {
        return Err("지분 계좌는 20바이트 Ethereum 주소여야 합니다.".into());
    }
    if candidate.identity_country_verified
        && candidate.country_code.as_deref().unwrap_or_default().len() != 2
    {
        return Err("국가 인증 후보에는 2자리 국가 코드가 필요합니다.".into());
    }
    if candidate
        .node_endpoint
        .as_deref()
        .is_some_and(str::is_empty)
    {
        return Err("노드 접속 주소는 비어 있을 수 없습니다.".into());
    }
    Ok(())
}

fn normalize_account(value: &str) -> String {
    format!("0x{}", value.trim_start_matches("0x").to_ascii_lowercase())
}

fn voting_power(stake: u128, circulating_supply: u128) -> u64 {
    let proportional = stake
        .saturating_mul(1_000_000)
        .checked_div(circulating_supply)
        .unwrap_or(0)
        .max(1);
    proportional.min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(index: u8, account: &str) -> ValidatorCandidate {
        ValidatorCandidate {
            validator_id: hex::encode([index; 32]),
            stake_account: account.into(),
            delegated_stake: 0,
            country_code: None,
            identity_country_verified: false,
            ownership_proof_verified: true,
            node_endpoint: Some("/dns4/node.example/udp/7001/quic-v1".into()),
            observed_epochs: 0,
            uptime_bps: 0,
            administrator_approved: false,
            blocked: false,
        }
    }

    #[test]
    fn one_percent_delegation_and_admin_routes_are_selected() {
        let accounts = [
            "0x1111111111111111111111111111111111111111",
            "0x2222222222222222222222222222222222222222",
            "0x3333333333333333333333333333333333333333",
            "0x4444444444444444444444444444444444444444",
        ];
        let mut candidates: Vec<_> = accounts
            .iter()
            .enumerate()
            .map(|(index, account)| candidate(index as u8 + 1, account))
            .collect();
        candidates[2].delegated_stake = 10;
        candidates[3].administrator_approved = true;
        let balances = HashMap::from([
            (accounts[0].into(), 50),
            (accounts[1].into(), 30),
            (accounts[2].into(), 0),
            (accounts[3].into(), 0),
        ]);
        let selected = ValidatorPolicy::default()
            .select(&candidates, &balances, 1_000)
            .unwrap();
        assert_eq!(selected.len(), 4);
        assert_eq!(selected[2].stake, 10);
    }

    #[test]
    fn country_top_fifty_is_computed_from_verified_accounts() {
        let mut candidates = Vec::new();
        let mut balances = HashMap::new();
        for index in 1..=55 {
            let account = format!("0x{index:040x}");
            let mut value = candidate(index, &account);
            value.country_code = Some("KR".into());
            value.identity_country_verified = true;
            candidates.push(value);
            balances.insert(account, u128::from(index));
        }
        let selected = ValidatorPolicy::default()
            .select(&candidates, &balances, 10_000_000)
            .unwrap();
        assert_eq!(selected.len(), 50);
        assert_eq!(selected[0].stake, 55);
        assert_eq!(selected[49].stake, 6);
    }

    #[test]
    fn low_uptime_or_blocked_candidate_is_never_selected() {
        let mut candidates = Vec::new();
        let mut balances = HashMap::new();
        for index in 1..=6 {
            let account = format!("0x{index:040x}");
            let mut value = candidate(index, &account);
            value.administrator_approved = true;
            value.observed_epochs = 1;
            value.uptime_bps = 9_900;
            value.blocked = index == 1;
            if index == 2 {
                value.uptime_bps = 9_000;
            }
            candidates.push(value);
            balances.insert(account, 100);
        }
        let selected = ValidatorPolicy::default()
            .select(&candidates, &balances, 1_000)
            .unwrap();
        assert_eq!(selected.len(), 4);
    }
}
