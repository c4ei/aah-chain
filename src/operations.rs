use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStorageMode {
    Pruned,
    Archive,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PruningPolicy {
    pub mode: NodeStorageMode,
    pub keep_recent_heights: u64,
    pub keep_checkpoint_interval: u64,
}

impl PruningPolicy {
    pub fn validate(&self) -> Result<(), String> {
        match self.mode {
            NodeStorageMode::Archive => Ok(()),
            NodeStorageMode::Pruned
                if self.keep_recent_heights > 0 && self.keep_checkpoint_interval > 0 =>
            {
                Ok(())
            }
            NodeStorageMode::Pruned => {
                Err("pruned 모드는 최근 높이와 체크포인트 간격이 필요합니다.".into())
            }
        }
    }

    pub fn should_keep(&self, current: u64, candidate: u64) -> bool {
        self.mode == NodeStorageMode::Archive
            || candidate.saturating_add(self.keep_recent_heights) >= current
            || candidate.is_multiple_of(self.keep_checkpoint_interval.max(1))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageManifest {
    pub schema_version: u32,
    pub backend: String,
    pub chain_id: u64,
    pub height: u64,
    pub state_root: String,
}

impl StorageManifest {
    pub fn backup(
        &self,
        data_dir: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<PathBuf, String> {
        let destination = destination.as_ref();
        fs::create_dir_all(destination).map_err(|error| error.to_string())?;
        let manifest_path = destination.join("storage-manifest.json");
        let temporary = manifest_path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        fs::rename(&temporary, &manifest_path).map_err(|error| error.to_string())?;
        let source = data_dir.as_ref().join("db/ieum-state.db");
        if source.exists() {
            fs::copy(source, destination.join("ieum-state.db"))
                .map_err(|error| error.to_string())?;
        }
        Ok(manifest_path)
    }

    pub fn verify_restore(
        source: impl AsRef<Path>,
        expected_chain_id: u64,
    ) -> Result<Self, String> {
        let bytes = fs::read(source.as_ref().join("storage-manifest.json"))
            .map_err(|error| error.to_string())?;
        let manifest: Self = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        if manifest.schema_version == 0 || manifest.chain_id != expected_chain_id {
            return Err("복원 manifest의 schema 또는 chain id가 올바르지 않습니다.".into());
        }
        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pruning_keeps_recent_and_checkpoint_heights() {
        let policy = PruningPolicy {
            mode: NodeStorageMode::Pruned,
            keep_recent_heights: 100,
            keep_checkpoint_interval: 1_000,
        };
        assert!(policy.should_keep(10_000, 9_950));
        assert!(policy.should_keep(10_000, 8_000));
        assert!(!policy.should_keep(10_000, 8_001));
    }
}
