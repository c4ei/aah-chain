use crate::genesis::GenesisConfig;
use crate::model::{Address, Block, Transaction};
use crate::wallet::verify_transaction_for_chain;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_MAX_BLOCK_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Blockchain {
    pub chain_id: u64,
    pub genesis_commitment: String,
    /// 확정된 블록만 저장합니다. 합의 중인 후보 블록은 여기에 넣으면 안 됩니다.
    pub blocks: Vec<Block>,
    /// 체크포인트로 시작할 때 활성 블록 앞에 있는 확정 기준점입니다.
    #[serde(default)]
    pub base_height: u64,
    #[serde(default)]
    pub base_hash: String,
    pub initial_balances: HashMap<Address, u128>,
    balances: HashMap<Address, u128>,
    next_nonces: HashMap<Address, u64>,
}

impl Blockchain {
    /// 제네시스 잔액과 0번 블록으로 새 체인을 시작합니다.
    pub fn new(initial_balances: Vec<(Address, u128)>) -> Self {
        Self::with_chain_id(21004, initial_balances)
    }

    pub fn with_chain_id(chain_id: u64, initial_balances: Vec<(Address, u128)>) -> Self {
        let mut entries = initial_balances.clone();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let commitment_bytes =
            serde_json::to_vec(&(chain_id, &entries)).expect("개발 제네시스 직렬화");
        let commitment = hex::encode(Sha256::digest(commitment_bytes));
        Self::build(chain_id, initial_balances, commitment)
    }

    pub fn from_genesis(genesis: &GenesisConfig) -> Result<Self, String> {
        genesis.validate()?;
        Ok(Self::build(
            genesis.chain_id,
            genesis.initial_balances.clone(),
            genesis.genesis_hash()?,
        ))
    }

    fn build(
        chain_id: u64,
        initial_balances: Vec<(Address, u128)>,
        genesis_commitment: String,
    ) -> Self {
        let initial_balances: HashMap<_, _> = initial_balances.into_iter().collect();
        Self {
            chain_id,
            blocks: vec![Block::genesis_with_commitment(&genesis_commitment)],
            base_height: 0,
            base_hash: Block::genesis_with_commitment(&genesis_commitment).hash,
            genesis_commitment,
            balances: initial_balances.clone(),
            initial_balances,
            next_nonces: HashMap::new(),
        }
    }

    pub fn from_snapshot(
        chain_id: u64,
        genesis_commitment: String,
        height: u64,
        block_hash: String,
        balances: HashMap<Address, u128>,
        next_nonces: HashMap<Address, u64>,
    ) -> Result<Self, String> {
        let chain = Self {
            chain_id,
            genesis_commitment,
            blocks: Vec::new(),
            base_height: height,
            base_hash: block_hash,
            initial_balances: balances.clone(),
            balances,
            next_nonces,
        };
        Ok(chain)
    }

    pub fn tip_height(&self) -> u64 {
        self.blocks
            .last()
            .map(|block| block.height)
            .unwrap_or(self.base_height)
    }

    pub fn tip_hash(&self) -> &str {
        self.blocks
            .last()
            .map(|block| block.hash.as_str())
            .unwrap_or(&self.base_hash)
    }

    pub fn balance_of(&self, address: &str) -> u128 {
        self.balances.get(address).copied().unwrap_or(0)
    }

    pub fn next_nonce(&self, address: &str) -> u64 {
        self.next_nonces.get(address).copied().unwrap_or(0)
    }

    pub fn balances_snapshot(&self) -> HashMap<Address, u128> {
        self.balances.clone()
    }

    pub fn nonces_snapshot(&self) -> HashMap<Address, u64> {
        self.next_nonces.clone()
    }

    pub fn block_by_height(&self, height: u64) -> Option<&Block> {
        self.blocks.iter().find(|block| block.height == height)
    }

    pub fn block_by_hash(&self, hash: &str) -> Option<&Block> {
        let hash = hash.trim_start_matches("0x");
        self.blocks.iter().find(|block| block.hash == hash)
    }

    pub fn transaction_by_hash(&self, hash: &str) -> Option<(&Block, usize, &Transaction)> {
        let hash = hash.trim_start_matches("0x");
        self.blocks.iter().find_map(|block| {
            block
                .transactions
                .iter()
                .enumerate()
                .find(|(_, transaction)| transaction.id() == hash)
                .map(|(index, transaction)| (block, index, transaction))
        })
    }

    /// 거래가 있을 때만 후보 블록을 만들고 즉시 적용하는 학습용 경로입니다.
    /// BFT 노드에서는 후보 생성과 확정을 분리해 확정 후 apply_block을 호출해야 합니다.
    pub fn add_block(
        &mut self,
        transactions: Vec<Transaction>,
        producer: Address,
    ) -> Result<&Block, String> {
        if transactions.is_empty() {
            return Err("거래가 없으므로 빈 블록을 건너뜁니다.".into());
        }
        let previous_height = self.tip_height();
        let previous_hash = self.tip_hash().to_string();
        let block = Block::new(
            previous_height + 1,
            previous_hash,
            now(),
            producer,
            transactions,
        );
        self.apply_block(block)?;
        Ok(self.blocks.last().unwrap())
    }

    /// 다른 노드에서 합의로 확정된 원본 블록을 검증하고 상태에 반영합니다.
    pub fn apply_block(&mut self, block: Block) -> Result<&Block, String> {
        let encoded_size = serde_json::to_vec(&block)
            .map_err(|error| error.to_string())?
            .len();
        if encoded_size > DEFAULT_MAX_BLOCK_BYTES {
            return Err(format!(
                "블록 크기 {encoded_size}바이트가 모바일 기본 제한 {DEFAULT_MAX_BLOCK_BYTES}바이트를 넘습니다."
            ));
        }
        if block.height != self.tip_height() + 1 || block.previous_hash != self.tip_hash() {
            return Err("새 블록이 현재 체인의 다음 블록이 아닙니다.".into());
        }
        if block.hash != block.calculate_hash() {
            return Err("새 블록 해시가 올바르지 않습니다.".into());
        }
        let mut balances = self.balances.clone();
        let mut nonces = self.next_nonces.clone();
        apply_transactions(
            self.chain_id,
            &block.transactions,
            &block.producer,
            &mut balances,
            &mut nonces,
        )?;
        self.blocks.push(block);
        self.balances = balances;
        self.next_nonces = nonces;
        Ok(self.blocks.last().unwrap())
    }

    /// 저장 파일을 읽은 뒤 제네시스부터 모든 잔액과 nonce를 다시 계산합니다.
    pub fn verify_and_rebuild(&mut self) -> Result<(), String> {
        if self.base_height > 0 {
            return Err("체크포인트 기반 체인은 전체 제네시스 재검증 대신 체크포인트 검증을 사용해야 합니다.".into());
        }
        let mut balances = self.initial_balances.clone();
        let mut nonces = HashMap::new();
        if self.blocks.first() != Some(&Block::genesis_with_commitment(&self.genesis_commitment)) {
            return Err("제네시스 블록이 다릅니다.".into());
        }
        for (index, block) in self.blocks.iter().enumerate() {
            if block.hash != block.calculate_hash() {
                return Err(format!("{index}번 블록 해시가 변조되었습니다."));
            }
            if index > 0 {
                let previous = &self.blocks[index - 1];
                if block.height != previous.height + 1 || block.previous_hash != previous.hash {
                    return Err(format!("{index}번 블록 연결이 끊어졌습니다."));
                }
                apply_transactions(
                    self.chain_id,
                    &block.transactions,
                    &block.producer,
                    &mut balances,
                    &mut nonces,
                )?;
            }
        }
        self.balances = balances;
        self.next_nonces = nonces;
        Ok(())
    }

    /// 체크포인트와 향후 상태 증명에서 사용할 결정론적 상태 해시입니다.
    pub fn state_hash(&self) -> String {
        let mut entries: Vec<_> = self.balances.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let mut hasher = Sha256::new();
        for (address, balance) in entries {
            hasher.update(address.as_bytes());
            hasher.update(balance.to_be_bytes());
            hasher.update(self.next_nonce(address).to_be_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

fn apply_transactions(
    chain_id: u64,
    transactions: &[Transaction],
    producer: &str,
    balances: &mut HashMap<Address, u128>,
    nonces: &mut HashMap<Address, u64>,
) -> Result<(), String> {
    // 블록 하나를 원자적으로 처리하기 위해 복제된 상태에 먼저 적용합니다.
    // 하나라도 실패하면 호출자가 원래 balances/nonces를 그대로 유지합니다.
    for tx in transactions {
        verify_transaction_for_chain(tx, chain_id)?;
        if tx.amount == 0 {
            return Err("송금액은 0보다 커야 합니다.".into());
        }
        let expected_nonce = nonces.get(&tx.from).copied().unwrap_or(0);
        if tx.nonce != expected_nonce {
            return Err(format!(
                "nonce 오류: 기대 {expected_nonce}, 입력 {}",
                tx.nonce
            ));
        }
        let total = tx
            .amount
            .checked_add(tx.fee)
            .ok_or("송금액과 수수료 합계가 너무 큽니다.")?;
        let total = u128::from(total);
        let sender = balances.get(&tx.from).copied().unwrap_or(0);
        if sender < total {
            return Err("수수료를 포함한 잔액이 부족합니다.".into());
        }
        balances.insert(tx.from.clone(), sender - total);
        *balances.entry(tx.to.clone()).or_default() += u128::from(tx.amount);
        *balances.entry(producer.to_string()).or_default() += u128::from(tx.fee);
        nonces.insert(tx.from.clone(), expected_nonce + 1);
    }
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("시스템 시간이 잘못되었습니다.")
        .as_secs()
}
