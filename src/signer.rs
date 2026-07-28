use crate::wallet::{verify_signature, Wallet};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SignRequest {
    version: u32,
    public_key: String,
    payload_hex: String,
}

#[derive(Debug)]
pub struct ExternalSigner {
    program: PathBuf,
    public_key: String,
}

impl ExternalSigner {
    pub fn new(program: impl AsRef<Path>, public_key: impl Into<String>) -> Result<Self, String> {
        let program = program.as_ref().to_path_buf();
        if !program.exists() {
            return Err(format!("외부 signer 프로그램이 없습니다: {}", program.display()));
        }
        let public_key = public_key.into();
        if hex::decode(&public_key).map(|bytes| bytes.len()).unwrap_or_default() != 32 {
            return Err("외부 signer 공개키는 32바이트 Ed25519 hex여야 합니다.".into());
        }
        Ok(Self { program, public_key })
    }

    pub fn address(&self) -> String {
        self.public_key.clone()
    }

    pub fn sign_bytes(&self, payload: &[u8]) -> Result<String, String> {
        let request = SignRequest {
            version: 1,
            public_key: self.public_key.clone(),
            payload_hex: hex::encode(payload),
        };
        let mut child = Command::new(&self.program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("외부 signer 실행 실패: {error}"))?;
        child
            .stdin
            .take()
            .ok_or("외부 signer stdin을 열 수 없습니다.")?
            .write_all(&serde_json::to_vec(&request).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        let output = child.wait_with_output().map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "외부 signer 거부: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let signature = String::from_utf8(output.stdout)
            .map_err(|_| "외부 signer 응답은 UTF-8 hex여야 합니다.")?
            .trim()
            .to_string();
        verify_signature(&self.public_key, payload, &signature)?;
        Ok(signature)
    }
}

#[derive(Debug)]
pub enum ValidatorSigner {
    Local(Wallet),
    External(ExternalSigner),
}

impl ValidatorSigner {
    pub fn address(&self) -> String {
        match self {
            Self::Local(wallet) => wallet.address(),
            Self::External(signer) => signer.address(),
        }
    }

    pub fn sign_bytes(&self, payload: &[u8]) -> Result<String, String> {
        match self {
            Self::Local(wallet) => Ok(wallet.sign_bytes(payload)),
            Self::External(signer) => signer.sign_bytes(payload),
        }
    }
}

impl From<Wallet> for ValidatorSigner {
    fn from(value: Wallet) -> Self {
        Self::Local(value)
    }
}

impl From<ExternalSigner> for ValidatorSigner {
    fn from(value: ExternalSigner) -> Self {
        Self::External(value)
    }
}
