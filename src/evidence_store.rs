use crate::consensus::DoubleVoteEvidence;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 검증자 이중투표 증거를 재시작 뒤에도 유지하는 append-only 저장소입니다.
pub struct EvidenceStore {
    path: PathBuf,
}

impl EvidenceStore {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            path: data_dir.as_ref().join("double-vote-evidence.jsonl"),
        }
    }

    pub fn append(&self, evidence: &DoubleVoteEvidence) -> Result<bool, String> {
        evidence.verify()?;
        let known = self.load_ids()?;
        if known.contains(&evidence.id()) {
            return Ok(false);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let line = serde_json::to_string(evidence).map_err(|error| error.to_string())?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        writeln!(file, "{line}").map_err(|error| error.to_string())?;
        file.sync_data().map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub fn load(&self) -> Result<Vec<DoubleVoteEvidence>, String> {
        let Ok(contents) = fs::read_to_string(&self.path) else {
            return Ok(Vec::new());
        };
        let mut evidence = Vec::new();
        let mut ids = HashSet::new();
        for (index, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let item: DoubleVoteEvidence = serde_json::from_str(line)
                .map_err(|error| format!("이중투표 증거 {}번째 줄 오류: {error}", index + 1))?;
            item.verify()?;
            if ids.insert(item.id()) {
                evidence.push(item);
            }
        }
        Ok(evidence)
    }

    fn load_ids(&self) -> Result<HashSet<String>, String> {
        Ok(self.load()?.into_iter().map(|item| item.id()).collect())
    }
}
