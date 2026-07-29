use crate::model::Address;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const MAX_CLOCK_DRIFT_SECONDS: u64 = 30;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduledEventAction {
    TreasuryDistribution {
        recipients: Vec<EventPayment>,
    },
    PeriodicProducerReward {
        producer: Address,
        amount: u128,
    },
    IncidentCompensation {
        incident_id: String,
        victim: Address,
        amount: u128,
    },
    ProtocolCheckpoint {
        protocol_version: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventPayment {
    pub address: Address,
    pub amount: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledEvent {
    pub id: String,
    pub execute_at: u64,
    pub action: ScheduledEventAction,
}

impl ScheduledEvent {
    pub fn consensus_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("ScheduledEvent 직렬화는 실패하지 않아야 합니다")
    }

    pub fn payload_hash(&self) -> String {
        hex::encode(Sha256::digest(self.consensus_bytes()))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_' | b'.'))
        {
            return Err("이벤트 ID는 1~128자의 영문, 숫자, -, _, .만 허용합니다.".into());
        }
        if self.execute_at == 0 {
            return Err(format!(
                "이벤트 {}의 execute_at은 0일 수 없습니다.",
                self.id
            ));
        }
        let payments = match &self.action {
            ScheduledEventAction::TreasuryDistribution { recipients } => recipients,
            ScheduledEventAction::PeriodicProducerReward { producer, amount } => {
                validate_address(producer)?;
                if *amount == 0 {
                    return Err("주간 생성자 보상액은 0보다 커야 합니다.".into());
                }
                return Ok(());
            }
            ScheduledEventAction::IncidentCompensation {
                incident_id,
                victim,
                amount,
            } => {
                if incident_id.trim().is_empty() {
                    return Err("사고 ID가 비어 있습니다.".into());
                }
                validate_address(victim)?;
                if *amount == 0 {
                    return Err("사고 보상액은 0보다 커야 합니다.".into());
                }
                return Ok(());
            }
            ScheduledEventAction::ProtocolCheckpoint { protocol_version } => {
                if *protocol_version == 0 {
                    return Err("프로토콜 버전은 1 이상이어야 합니다.".into());
                }
                return Ok(());
            }
        };
        if payments.is_empty() || payments.len() > 10_000 {
            return Err("재단 배분 대상은 1~10,000개여야 합니다.".into());
        }
        for payment in payments {
            validate_address(&payment.address)?;
            if payment.amount == 0 {
                return Err("재단 배분액은 0보다 커야 합니다.".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EventSchedule {
    #[serde(default)]
    pub events: Vec<ScheduledEvent>,
}

impl EventSchedule {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path).map_err(|error| {
            format!("이벤트 설정을 읽지 못했습니다({}): {error}", path.display())
        })?;
        let schedule: Self = serde_json::from_slice(&bytes)
            .map_err(|error| format!("이벤트 설정 JSON 오류({}): {error}", path.display()))?;
        schedule.validate()?;
        Ok(schedule)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        for event in &self.events {
            event.validate()?;
            if !ids.insert(&event.id) {
                return Err(format!("중복 이벤트 ID: {}", event.id));
            }
        }
        Ok(())
    }

    pub fn due(&self, timestamp: u64, executed: &HashSet<String>) -> Vec<ScheduledEvent> {
        let mut due: Vec<_> = self
            .events
            .iter()
            .filter(|event| event.execute_at <= timestamp && !executed.contains(&event.id))
            .cloned()
            .collect();
        due.sort_by(|a, b| (a.execute_at, &a.id).cmp(&(b.execute_at, &b.id)));
        due
    }

    pub fn validate_block_events(
        &self,
        timestamp: u64,
        events: &[ScheduledEvent],
        executed: &HashSet<String>,
    ) -> Result<(), String> {
        let expected = self.due(timestamp, executed);
        if events != expected {
            return Err("블록의 시스템 이벤트가 로컬 승인 일정과 일치하지 않습니다.".into());
        }
        Ok(())
    }

    pub fn next_pending_at(&self, executed: &HashSet<String>) -> Option<u64> {
        self.events
            .iter()
            .filter(|event| !executed.contains(&event.id))
            .map(|event| event.execute_at)
            .min()
    }
}

fn validate_address(address: &str) -> Result<(), String> {
    let value = address.strip_prefix("0x").unwrap_or(address);
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("잘못된 IEUM 주소: {address}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_events_are_ordered_and_exactly_once() {
        let schedule = EventSchedule {
            events: vec![
                ScheduledEvent {
                    id: "second".into(),
                    execute_at: 20,
                    action: ScheduledEventAction::ProtocolCheckpoint {
                        protocol_version: 2,
                    },
                },
                ScheduledEvent {
                    id: "first".into(),
                    execute_at: 10,
                    action: ScheduledEventAction::ProtocolCheckpoint {
                        protocol_version: 2,
                    },
                },
            ],
        };
        let mut executed = HashSet::new();
        assert_eq!(schedule.due(20, &executed)[0].id, "first");
        executed.insert("first".into());
        assert_eq!(schedule.due(20, &executed)[0].id, "second");
    }
}
