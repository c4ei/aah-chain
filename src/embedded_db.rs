use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DatabaseImage {
    schema_version: u32,
    generation: u64,
    values: BTreeMap<String, Vec<u8>>,
}

/// 별도 서버 없이 노드 데이터 디렉터리에 함께 저장되는 작은 key-value DB입니다.
///
/// 현재 구현은 단일 writer를 전제로 하며, 전체 image를 임시 파일에 기록하고
/// fsync한 뒤 rename합니다. backend 경계가 분리되어 있어 데이터가 커지면
/// RocksDB/SQLite 구현으로 교체할 수 있습니다.
#[derive(Clone, Debug)]
pub struct EmbeddedDb {
    path: PathBuf,
    wal_path: PathBuf,
    image: DatabaseImage,
}

impl EmbeddedDb {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let directory = data_dir.as_ref().join("db");
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let path = directory.join("ieum-state.db");
        let wal_path = directory.join("ieum-state.wal");
        let source = if wal_path.exists() { &wal_path } else { &path };
        let image = match fs::read(source) {
            Ok(bytes) => {
                let image: DatabaseImage =
                    serde_json::from_slice(&bytes).map_err(|error| format!("embedded DB 손상: {error}"))?;
                if image.schema_version != SCHEMA_VERSION {
                    return Err("지원하지 않는 embedded DB schema입니다.".into());
                }
                image
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => DatabaseImage {
                schema_version: SCHEMA_VERSION,
                generation: 0,
                values: BTreeMap::new(),
            },
            Err(error) => return Err(error.to_string()),
        };
        let mut db = Self { path, wal_path, image };
        if db.wal_path.exists() {
            db.checkpoint()?;
        }
        Ok(db)
    }

    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.image.values.get(key).map(Vec::as_slice)
    }

    pub fn put(&mut self, key: impl Into<String>, value: Vec<u8>) {
        self.image.values.insert(key.into(), value);
    }

    pub fn remove(&mut self, key: &str) {
        self.image.values.remove(key);
    }

    pub fn scan_prefix(&self, prefix: &str) -> Vec<(String, Vec<u8>)> {
        self.image
            .values
            .range(prefix.to_owned()..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    pub fn commit(&mut self) -> Result<u64, String> {
        self.image.generation = self.image.generation.saturating_add(1);
        let bytes = serde_json::to_vec(&self.image).map_err(|error| error.to_string())?;
        let temporary = self.wal_path.with_extension("tmp");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, &self.wal_path).map_err(|error| error.to_string())?;
        self.checkpoint()?;
        Ok(self.image.generation)
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        if self.wal_path.exists() {
            fs::rename(&self.wal_path, &self.path).map_err(|error| error.to_string())?;
        }
        if let Some(parent) = self.path.parent() {
            OpenOptions::new()
                .read(true)
                .open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn generation(&self) -> u64 {
        self.image.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_atomic_key_value_image() {
        let root = std::env::temp_dir().join(format!("ieum-embedded-db-{}", std::process::id()));
        let mut db = EmbeddedDb::open(&root).unwrap();
        db.put("state/root", b"abc".to_vec());
        assert_eq!(db.commit().unwrap(), 1);
        let restored = EmbeddedDb::open(&root).unwrap();
        assert_eq!(restored.get("state/root"), Some(b"abc".as_slice()));
        assert_eq!(restored.generation(), 1);
        let _ = fs::remove_dir_all(root);
    }
}
