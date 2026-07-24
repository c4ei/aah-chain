use crate::chain::Blockchain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub height: u64,
    pub block_hash: String,
    pub state_hash: String,
}

impl Checkpoint {
    /// 거래가 없는 동안에도 노드가 정상임을 증명할 최소 체크포인트 원문을 만듭니다.
    /// 실제 네트워크에서는 검증자 2/3 초과의 서명을 함께 저장해야 합니다.
    pub fn from_chain(chain: &Blockchain) -> Self {
        let last = chain.blocks.last().expect("블록이 필요합니다.");
        Self {
            height: last.height,
            block_hash: last.hash.clone(),
            state_hash: chain.state_hash(),
        }
    }
}
