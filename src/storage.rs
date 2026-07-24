use crate::chain::Blockchain;
use std::fs;
use std::path::Path;

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

pub fn load(path: impl AsRef<Path>) -> Result<Blockchain, String> {
    // 파일 내용을 그대로 신뢰하지 않고 전체 체인을 재검증합니다.
    let json = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut chain: Blockchain = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    chain.verify_and_rebuild()?;
    Ok(chain)
}
