use libp2p::identity::Keypair;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// 영구 libp2p 키를 읽거나 처음 한 번만 생성합니다. 같은 파일을 사용하면 PeerId가 유지됩니다.
pub fn load_or_create_node_key(path: impl AsRef<Path>) -> Result<Keypair, String> {
    let path = path.as_ref();
    if path.exists() {
        let bytes = fs::read(path).map_err(|e| format!("node key 읽기 실패: {e}"))?;
        return Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| format!("node key 형식 오류: {e}"));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("node key 폴더 생성 실패: {e}"))?;
    }
    let key = Keypair::generate_ed25519();
    let bytes = key
        .to_protobuf_encoding()
        .map_err(|e| format!("node key 직렬화 실패: {e}"))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("node key 생성 실패: {e}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("node key 저장 실패: {e}"))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_key_keeps_peer_id() {
        let path = std::env::temp_dir().join(format!("aah-node-key-{}", std::process::id()));
        let first = load_or_create_node_key(&path).unwrap();
        let second = load_or_create_node_key(&path).unwrap();
        assert_eq!(
            libp2p::PeerId::from(first.public()),
            libp2p::PeerId::from(second.public())
        );
        let _ = std::fs::remove_file(path);
    }
}
