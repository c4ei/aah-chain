use crate::model::Transaction;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct Mempool {
    transactions: Vec<Transaction>,
    ids: HashSet<String>,
}

impl Mempool {
    /// 거래 ID가 같은 거래를 두 번 넣지 않습니다.
    /// 잔액과 nonce의 최종 검증은 블록 실행 시 다시 수행합니다.
    pub fn add(&mut self, tx: Transaction) -> Result<(), String> {
        let id = tx.id();
        if !self.ids.insert(id) {
            return Err("이미 mempool에 있는 거래입니다.".into());
        }
        self.transactions.push(tx);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// 블록 최대 거래 수만큼 앞에서 꺼냅니다.
    /// 운영 버전에서는 수수료와 공정성을 함께 고려한 선택 정책이 필요합니다.
    pub fn drain(&mut self, max_count: usize) -> Vec<Transaction> {
        let count = max_count.min(self.transactions.len());
        let selected: Vec<_> = self.transactions.drain(..count).collect();
        for tx in &selected {
            self.ids.remove(&tx.id());
        }
        selected
    }
}
