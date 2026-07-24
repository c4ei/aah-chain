use crate::consensus::{ConsensusMessage, VoteType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 노드 재시작 후에도 자신이 어떤 투표를 했는지 복구하기 위한 WAL 한 줄입니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalVote {
    pub message: ConsensusMessage,
}

/// 합의 투표를 JSON Lines 형식으로 먼저 기록한 뒤 네트워크에 전파합니다.
/// 이렇게 해야 재시작 직후 같은 높이에서 다른 블록에 다시 서명하는 사고를 막을 수 있습니다.
#[derive(Debug)]
pub struct ConsensusWal {
    path: PathBuf,
    signed: HashMap<(u64, u32, VoteType), String>,
}

impl ConsensusWal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let mut wal = Self {
            path,
            signed: HashMap::new(),
        };
        wal.reload()?;
        Ok(wal)
    }

    /// 서명 메시지를 디스크에 동기화합니다.
    /// 동일 높이·라운드·종류의 다른 블록에는 절대 다시 서명하지 않습니다.
    /// 이 WAL 인스턴스는 검증자 개인키 하나에만 전용으로 사용해야 합니다.
    pub fn record_before_broadcast(&mut self, message: &ConsensusMessage) -> Result<(), String> {
        message.verify()?;
        let key = (message.height, message.round, message.vote_type);
        if let Some(previous) = self.signed.get(&key) {
            if previous != &message.block_hash {
                return Err("WAL이 이중투표 서명을 차단했습니다.".into());
            }
            return Ok(());
        }

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let line = serde_json::to_string(&WalVote {
            message: message.clone(),
        })
        .map_err(|error| error.to_string())?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        writeln!(file, "{line}").map_err(|error| error.to_string())?;
        file.sync_data().map_err(|error| error.to_string())?;
        self.signed.insert(key, message.block_hash.clone());
        Ok(())
    }

    fn reload(&mut self) -> Result<(), String> {
        let Ok(contents) = fs::read_to_string(&self.path) else {
            return Ok(());
        };
        for (index, line) in contents.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let record: WalVote = serde_json::from_str(line)
                .map_err(|error| format!("WAL {}번째 줄 복구 실패: {error}", index + 1))?;
            record.message.verify()?;
            let key = (
                record.message.height,
                record.message.round,
                record.message.vote_type,
            );
            if let Some(previous) = self.signed.insert(key, record.message.block_hash.clone()) {
                if previous != record.message.block_hash {
                    return Err("WAL에서 과거 이중투표 기록을 발견했습니다.".into());
                }
            }
        }
        Ok(())
    }
}
