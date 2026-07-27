use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub type Address = String;

/// 사용자가 서명해 네트워크에 제출하는 가장 기본적인 코인 송금 거래입니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transaction {
    pub from: Address,
    pub to: Address,
    /// 1 IEUM = 10^18 최소 단위이므로 전체 발행량을 안전하게 담도록 u128을 사용합니다.
    pub amount: u128,
    pub fee: u128,
    pub nonce: u64,
    pub signature: String,
}

impl Transaction {
    /// 직렬화 구현이 바뀌어도 같은 서명 결과가 나오도록 필드를 고정 순서로 조합합니다.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_text(&mut bytes, &self.from);
        push_text(&mut bytes, &self.to);
        bytes.extend_from_slice(&self.amount.to_be_bytes());
        bytes.extend_from_slice(&self.fee.to_be_bytes());
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        bytes
    }

    /// mempool 중복 검사와 블록 해시에 사용할 거래 식별자입니다.
    pub fn id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.signing_bytes());
        hasher.update(self.signature.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// 확정된 거래 묶음입니다. 빈 거래 블록은 만들지 않는 것이 이 체인의 기본 정책입니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    pub height: u64,
    pub previous_hash: String,
    pub timestamp: u64,
    pub producer: Address,
    pub transactions: Vec<Transaction>,
    pub hash: String,
}

impl Block {
    /// 모든 노드가 동일하게 시작하는 0번 블록입니다.
    pub fn genesis() -> Self {
        let mut block = Self {
            height: 0,
            previous_hash: "0".repeat(64),
            timestamp: 0,
            producer: "genesis".into(),
            transactions: vec![],
            hash: String::new(),
        };
        block.hash = block.calculate_hash();
        block
    }

    pub fn new(
        height: u64,
        previous_hash: String,
        timestamp: u64,
        producer: Address,
        transactions: Vec<Transaction>,
    ) -> Self {
        let mut block = Self {
            height,
            previous_hash,
            timestamp,
            producer,
            transactions,
            hash: String::new(),
        };
        block.hash = block.calculate_hash();
        block
    }

    /// 블록의 모든 합의 대상 필드를 고정 순서로 해시합니다.
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.height.to_be_bytes());
        hasher.update(self.previous_hash.as_bytes());
        hasher.update(self.timestamp.to_be_bytes());
        hasher.update(self.producer.as_bytes());
        for tx in &self.transactions {
            hasher.update(tx.id().as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

fn push_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}
