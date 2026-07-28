use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolUpgrade {
    pub name: String,
    pub activation_height: u64,
    pub protocol_version: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpgradeSchedule {
    pub upgrades: Vec<ProtocolUpgrade>,
}

impl UpgradeSchedule {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        let mut schedule: Self =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        schedule.upgrades.sort_by_key(|upgrade| upgrade.activation_height);
        for pair in schedule.upgrades.windows(2) {
            if pair[0].activation_height == pair[1].activation_height {
                return Err("같은 높이에 두 프로토콜 업그레이드를 등록할 수 없습니다.".into());
            }
        }
        Ok(schedule)
    }

    pub fn version_at(&self, height: u64) -> u32 {
        self.upgrades
            .iter()
            .filter(|upgrade| upgrade.activation_height <= height)
            .map(|upgrade| upgrade.protocol_version)
            .next_back()
            .unwrap_or(1)
    }

    pub fn ensure_supported(&self, height: u64, supported: u32) -> Result<(), String> {
        let required = self.version_at(height);
        if required > supported {
            return Err(format!(
                "높이 {height}부터 프로토콜 v{required}이 필요합니다. 노드를 업그레이드하세요."
            ));
        }
        Ok(())
    }
}
