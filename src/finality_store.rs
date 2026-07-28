use crate::consensus::{FinalityCertificate, Validator};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// 재시작 뒤에도 신규 노드에 확정 증명을 제공하기 위한 append-only 저장소입니다.
pub struct FinalityStore {
    path: PathBuf,
}

impl FinalityStore {
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let path = data_dir.as_ref().join("finality-certificates.jsonl");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        Ok(Self { path })
    }

    pub fn append(&self, certificate: &FinalityCertificate) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        serde_json::to_writer(&mut file, certificate).map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())?;
        file.sync_data().map_err(|error| error.to_string())
    }

    pub fn load(
        &self,
        validators: &[Validator],
    ) -> Result<Vec<FinalityCertificate>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = OpenOptions::new()
            .read(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        let mut certificates = Vec::new();
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|error| error.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            let certificate: FinalityCertificate = serde_json::from_str(&line)
                .map_err(|error| format!("확정 인증서 {}행 오류: {error}", index + 1))?;
            certificate.verify(validators)?;
            certificates.push(certificate);
        }
        certificates.sort_by_key(|certificate| certificate.block.height);
        for pair in certificates.windows(2) {
            if pair[0].block.height == pair[1].block.height
                && pair[0].block.hash != pair[1].block.hash
            {
                return Err(format!(
                    "확정성 위반: 높이 {}의 인증서 해시가 서로 다릅니다.",
                    pair[0].block.height
                ));
            }
        }
        Ok(certificates)
    }
}
