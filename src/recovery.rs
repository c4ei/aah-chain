use crate::Validator;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 사고 복구안은 검증자 수 또는 투표권 중 어느 한쪽에서 3/4 이상 동의를 받아야 합니다.
pub const RECOVERY_QUORUM_NUMERATOR: u128 = 3;
pub const RECOVERY_QUORUM_DENOMINATOR: u128 = 4;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryApprovalBasis {
    ValidatorCount,
    VotingPower,
    Both,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryApprovalResult {
    pub approved: bool,
    pub basis: Option<RecoveryApprovalBasis>,
    pub approved_validators: usize,
    pub total_validators: usize,
    pub approved_voting_power: u64,
    pub total_voting_power: u64,
}

/// `approval_ids`는 복구 계획의 정확한 해시에 대한 서명이 이미 검증된 검증자 ID입니다.
/// 동일 ID는 한 번만 계산하며 미등록 검증자 ID가 있으면 전체 검사를 거부합니다.
pub fn evaluate_recovery_approvals(
    validators: &[Validator],
    approval_ids: impl IntoIterator<Item = String>,
) -> Result<RecoveryApprovalResult, String> {
    if validators.is_empty() {
        return Err("복구 승인에 사용할 등록 검증자가 없습니다.".into());
    }

    let mut powers = HashMap::new();
    let mut total_power = 0u64;
    for validator in validators {
        if validator.voting_power == 0 {
            return Err(format!("검증자 {}의 투표권이 0입니다.", validator.id));
        }
        if powers
            .insert(validator.id.clone(), validator.voting_power)
            .is_some()
        {
            return Err(format!("중복 등록 검증자입니다: {}", validator.id));
        }
        total_power = total_power
            .checked_add(validator.voting_power)
            .ok_or("전체 검증자 투표권 합계가 u64 범위를 넘습니다.")?;
    }

    let approvals: HashSet<String> = approval_ids.into_iter().collect();
    let mut approved_power = 0u64;
    for id in &approvals {
        let power = powers
            .get(id)
            .ok_or_else(|| format!("미등록 검증자의 복구 승인이 포함됐습니다: {id}"))?;
        approved_power = approved_power
            .checked_add(*power)
            .ok_or("승인 투표권 합계가 u64 범위를 넘습니다.")?;
    }

    let count_passed = reaches_three_quarters(approvals.len() as u128, validators.len() as u128);
    let power_passed = reaches_three_quarters(approved_power as u128, total_power as u128);
    let basis = if count_passed {
        Some(RecoveryApprovalBasis::ValidatorCount)
    } else if power_passed {
        Some(RecoveryApprovalBasis::VotingPower)
    } else {
        None
    };

    Ok(RecoveryApprovalResult {
        approved: basis.is_some(),
        basis,
        approved_validators: approvals.len(),
        total_validators: validators.len(),
        approved_voting_power: approved_power,
        total_voting_power: total_power,
    })
}

/// 체크포인트 롤백과 총공급량 변경은 검증자 수와 투표권 모두 3/4 이상이어야 합니다.
/// 한 검증자가 투표권 75%를 보유해도 단독으로 전체 체인을 되돌릴 수 없습니다.
pub fn evaluate_checkpoint_recovery_approvals(
    validators: &[Validator],
    approval_ids: impl IntoIterator<Item = String>,
) -> Result<RecoveryApprovalResult, String> {
    let mut result = evaluate_recovery_approvals(validators, approval_ids)?;
    let count_passed = reaches_three_quarters(
        result.approved_validators as u128,
        result.total_validators as u128,
    );
    let power_passed = reaches_three_quarters(
        result.approved_voting_power as u128,
        result.total_voting_power as u128,
    );
    result.approved = count_passed && power_passed;
    result.basis = result.approved.then_some(RecoveryApprovalBasis::Both);
    Ok(result)
}

fn reaches_three_quarters(approved: u128, total: u128) -> bool {
    total > 0
        && approved.saturating_mul(RECOVERY_QUORUM_DENOMINATOR)
            >= total.saturating_mul(RECOVERY_QUORUM_NUMERATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validators(powers: &[u64]) -> Vec<Validator> {
        powers
            .iter()
            .enumerate()
            .map(|(index, power)| Validator {
                id: format!("validator-{index}"),
                voting_power: *power,
            })
            .collect()
    }

    #[test]
    fn three_of_four_validators_approve() {
        let result = evaluate_recovery_approvals(
            &validators(&[25, 25, 25, 25]),
            [0, 1, 2].map(|index| format!("validator-{index}")),
        )
        .unwrap();
        assert!(result.approved);
        assert_eq!(result.basis, Some(RecoveryApprovalBasis::ValidatorCount));
    }

    #[test]
    fn three_quarters_voting_power_can_approve() {
        let result =
            evaluate_recovery_approvals(&validators(&[80, 10, 5, 5]), ["validator-0".to_string()])
                .unwrap();
        assert!(result.approved);
        assert_eq!(result.basis, Some(RecoveryApprovalBasis::VotingPower));
    }

    #[test]
    fn duplicate_approval_is_counted_once() {
        let result = evaluate_recovery_approvals(
            &validators(&[25, 25, 25, 25]),
            ["validator-0".to_string(), "validator-0".to_string()],
        )
        .unwrap();
        assert!(!result.approved);
        assert_eq!(result.approved_validators, 1);
    }

    #[test]
    fn unknown_validator_is_rejected() {
        assert!(
            evaluate_recovery_approvals(&validators(&[25, 25, 25, 25]), ["attacker".to_string()],)
                .is_err()
        );
    }

    #[test]
    fn checkpoint_requires_count_and_voting_power_quorum() {
        let result = evaluate_checkpoint_recovery_approvals(
            &validators(&[80, 10, 5, 5]),
            ["validator-0".to_string()],
        )
        .unwrap();
        assert!(!result.approved);

        let result = evaluate_checkpoint_recovery_approvals(
            &validators(&[80, 10, 5, 5]),
            [0, 1, 2].map(|index| format!("validator-{index}")),
        )
        .unwrap();
        assert!(result.approved);
        assert_eq!(result.basis, Some(RecoveryApprovalBasis::Both));
    }
}
