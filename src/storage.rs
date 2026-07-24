use crate::chain::Blockchain;
use crate::model::Block;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MAX_SEGMENT_BYTES: u64 = 100_000_000;

pub fn save(chain: &Blockchain, path: impl AsRef<Path>) -> Result<(), String> {
    // 학습 단계에서는 사람이 읽기 쉬운 JSON을 사용합니다.
    // 데이터가 커지면 RocksDB 계열의 상태 저장소와 원자적 batch write로 교체합니다.
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(chain).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

/// 확정 블록을 JSONL 세그먼트에 추가하고, 다음 기록이 100MB를 넘기 전에 새 파일로 회전합니다.
/// 모바일 클라이언트는 이 파일을 받지 않고 체크포인트/헤더만 동기화합니다.
pub fn append_block_segmented(
    directory: impl AsRef<Path>,
    block: &Block,
    max_segment_bytes: u64,
) -> Result<PathBuf, String> {
    if max_segment_bytes == 0 || max_segment_bytes > MAX_SEGMENT_BYTES {
        return Err("세그먼트 제한은 1바이트 이상 100MB 이하여야 합니다.".into());
    }
    let directory = directory.as_ref();
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut record = serde_json::to_vec(block).map_err(|error| error.to_string())?;
    record.push(b'\n');
    if record.len() as u64 > max_segment_bytes {
        return Err("블록 하나가 세그먼트 최대 크기보다 큽니다.".into());
    }

    let mut index = 0u64;
    loop {
        let path = directory.join(format!("blocks-{index:06}.jsonl"));
        let current = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        if current + record.len() as u64 <= max_segment_bytes {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|error| error.to_string())?;
            file.write_all(&record).map_err(|error| error.to_string())?;
            file.sync_data().map_err(|error| error.to_string())?;
            return Ok(path);
        }
        index = index.checked_add(1).ok_or("세그먼트 번호가 넘쳤습니다.")?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_rotates_before_limit() {
        let dir = std::env::temp_dir().join(format!("aah-segment-test-{}", std::process::id()));
        let block = Block::genesis();
        let one = serde_json::to_vec(&block).unwrap().len() as u64 + 1;
        let first = append_block_segmented(&dir, &block, one).unwrap();
        let second = append_block_segmented(&dir, &block, one).unwrap();
        assert_ne!(first, second);
        let _ = fs::remove_dir_all(dir);
    }
}

pub fn load(path: impl AsRef<Path>) -> Result<Blockchain, String> {
    // 파일 내용을 그대로 신뢰하지 않고 전체 체인을 재검증합니다.
    let json = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut chain: Blockchain = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    chain.verify_and_rebuild()?;
    Ok(chain)
}
