use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    Transfer,
    HighValueTransfer,
    Validator,
    AccountRecovery,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountKey {
    pub public_key: String,
    pub weight: u16,
    pub permissions: HashSet<Permission>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountPolicy {
    pub keys: Vec<AccountKey>,
    pub thresholds: HashMap<Permission, u16>,
    pub high_value_amount: u128,
    pub policy_nonce: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Authorization {
    pub policy_nonce: u64,
    pub signer_keys: Vec<String>,
}

impl AccountPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.keys.is_empty() {
            return Err("계정에는 하나 이상의 키가 필요합니다.".into());
        }
        let mut unique = HashSet::new();
        if self
            .keys
            .iter()
            .any(|key| key.weight == 0 || !unique.insert(key.public_key.as_str()))
        {
            return Err("계정 키는 고유해야 하고 가중치는 1 이상이어야 합니다.".into());
        }
        for (permission, threshold) in &self.thresholds {
            let available: u32 = self
                .keys
                .iter()
                .filter(|key| key.permissions.contains(permission))
                .map(|key| key.weight as u32)
                .sum();
            if *threshold == 0 || *threshold as u32 > available {
                return Err("권한 임계값이 사용 가능한 키 가중치를 넘었습니다.".into());
            }
        }
        Ok(())
    }

    pub fn authorize(
        &self,
        permission: Permission,
        authorization: &Authorization,
    ) -> Result<(), String> {
        self.validate()?;
        if authorization.policy_nonce != self.policy_nonce {
            return Err("오래된 계정 정책에 대한 서명입니다.".into());
        }
        let threshold = *self
            .thresholds
            .get(&permission)
            .ok_or("해당 작업의 권한 임계값이 없습니다.")? as u32;
        let unique: HashSet<_> = authorization
            .signer_keys
            .iter()
            .map(String::as_str)
            .collect();
        let weight: u32 = self
            .keys
            .iter()
            .filter(|key| {
                unique.contains(key.public_key.as_str()) && key.permissions.contains(&permission)
            })
            .map(|key| key.weight as u32)
            .sum();
        if weight < threshold {
            return Err("작업에 필요한 서명 가중치가 부족합니다.".into());
        }
        Ok(())
    }

    pub fn transfer_permission(&self, amount: u128) -> Permission {
        if amount >= self.high_value_amount {
            Permission::HighValueTransfer
        } else {
            Permission::Transfer
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryRequest {
    pub account: String,
    pub new_keys: Vec<AccountKey>,
    pub execute_after_height: u64,
    pub authorization: Authorization,
}

impl RecoveryRequest {
    pub fn verify(&self, policy: &AccountPolicy, current_height: u64) -> Result<(), String> {
        if current_height < self.execute_after_height {
            return Err("계정 복구 대기 높이가 지나지 않았습니다.".into());
        }
        if self.new_keys.is_empty() {
            return Err("복구 후 계정 키가 비어 있을 수 없습니다.".into());
        }
        policy.authorize(Permission::AccountRecovery, &self.authorization)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AccountPolicy {
        let all: HashSet<_> = [
            Permission::Transfer,
            Permission::HighValueTransfer,
            Permission::AccountRecovery,
        ]
        .into_iter()
        .collect();
        AccountPolicy {
            keys: (1..=3)
                .map(|index| AccountKey {
                    public_key: format!("key-{index}"),
                    weight: 1,
                    permissions: all.clone(),
                })
                .collect(),
            thresholds: [
                (Permission::Transfer, 1),
                (Permission::HighValueTransfer, 2),
                (Permission::AccountRecovery, 2),
            ]
            .into_iter()
            .collect(),
            high_value_amount: 1_000,
            policy_nonce: 7,
        }
    }

    #[test]
    fn high_value_transfer_requires_two_keys() {
        let policy = policy();
        let one = Authorization {
            policy_nonce: 7,
            signer_keys: vec!["key-1".into()],
        };
        assert!(
            policy
                .authorize(policy.transfer_permission(1_000), &one)
                .is_err()
        );
        let two = Authorization {
            policy_nonce: 7,
            signer_keys: vec!["key-1".into(), "key-2".into()],
        };
        assert!(
            policy
                .authorize(policy.transfer_permission(1_000), &two)
                .is_ok()
        );
    }
}
