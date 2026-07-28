use crate::wallet::{verify_signature, Wallet};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SignRequest {
    version: u32,
    public_key: String,
    payload_hex: String,
}

#[derive(Debug)]
pub struct ExternalSigner {
    programs: Vec<PathBuf>,
    public_key: String,
    timeout: Duration,
    minimum_interval: Duration,
    last_request: std::sync::Mutex<Option<Instant>>,
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
        Ok(Self {
            programs: vec![program],
            public_key,
            timeout: Duration::from_secs(3),
            minimum_interval: Duration::from_millis(10),
            last_request: std::sync::Mutex::new(None),
        })
    }

    pub fn with_failover(
        programs: Vec<PathBuf>,
        public_key: impl Into<String>,
        timeout: Duration,
        minimum_interval: Duration,
    ) -> Result<Self, String> {
        if programs.is_empty() || programs.iter().any(|program| !program.exists()) {
            return Err("외부 signer 프로그램을 하나 이상 지정해야 하며 모두 존재해야 합니다.".into());
        }
        let public_key = public_key.into();
        if hex::decode(&public_key).map(|bytes| bytes.len()).unwrap_or_default() != 32 {
            return Err("외부 signer 공개키는 32바이트 Ed25519 hex여야 합니다.".into());
        }
        Ok(Self {
            programs,
            public_key,
            timeout,
            minimum_interval,
            last_request: std::sync::Mutex::new(None),
        })
    }

    pub fn address(&self) -> String {
        self.public_key.clone()
    }

    pub fn sign_bytes(&self, payload: &[u8]) -> Result<String, String> {
        let mut last_request = self
            .last_request
            .lock()
            .map_err(|_| "외부 signer rate limit 잠금이 손상되었습니다.")?;
        if let Some(last) = *last_request {
            let elapsed = last.elapsed();
            if elapsed < self.minimum_interval {
                thread::sleep(self.minimum_interval - elapsed);
            }
        }
        *last_request = Some(Instant::now());
        drop(last_request);

        let request = SignRequest {
            version: 1,
            public_key: self.public_key.clone(),
            payload_hex: hex::encode(payload),
        };
        let request_bytes = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
        let mut failures = Vec::new();
        for program in &self.programs {
            match self.try_program(program, &request_bytes, payload) {
                Ok(signature) => return Ok(signature),
                Err(error) => failures.push(format!("{}: {error}", program.display())),
            }
        }
        Err(format!("외부 signer 전체 실패: {}", failures.join("; ")))
    }

    fn try_program(&self, program: &Path, request: &[u8], payload: &[u8]) -> Result<String, String> {
        let mut child = Command::new(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("외부 signer 실행 실패: {error}"))?;
        child
            .stdin
            .take()
            .ok_or("외부 signer stdin을 열 수 없습니다.")?
            .write_all(request)
            .map_err(|error| error.to_string())?;
        let started = Instant::now();
        loop {
            if child.try_wait().map_err(|error| error.to_string())?.is_some() {
                break;
            }
            if started.elapsed() >= self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("응답 제한시간 {}ms 초과", self.timeout.as_millis()));
            }
            thread::sleep(Duration::from_millis(5));
        }
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
