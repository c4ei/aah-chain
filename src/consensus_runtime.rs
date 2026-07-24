use crate::chain::Blockchain;
use crate::consensus::{BftConsensus, ConsensusMessage, ConsensusPhase, SignedProposal, Validator};
use crate::model::Block;
use crate::wallet::Wallet;
use std::time::{Duration, Instant};

/// P2P 어댑터와 독립적으로 테스트 가능한 실제 합의 실행 코어입니다.
/// 후보 블록은 메모리에만 보관하고 2/3 초과 precommit 뒤에만 체인에 저장합니다.
pub struct ConsensusRuntime {
    pub chain: Blockchain,
    consensus: BftConsensus,
    validator: Wallet,
    pending: Option<Block>,
    deadline: Instant,
    timeout: Duration,
}

impl ConsensusRuntime {
    pub fn new(
        chain: Blockchain,
        validators: Vec<Validator>,
        validator: Wallet,
        timeout: Duration,
    ) -> Result<Self, String> {
        let next_height = chain.blocks.last().map(|b| b.height + 1).unwrap_or(1);
        let mut consensus = BftConsensus::new(validators)?;
        consensus.start_round(next_height, 0)?;
        Ok(Self {
            chain,
            consensus,
            validator,
            pending: None,
            deadline: Instant::now() + timeout,
            timeout,
        })
    }

    pub fn make_proposal(&self, block: Block) -> Result<SignedProposal, String> {
        if self.validator.address() != self.consensus.expected_proposer() {
            return Err("이 노드는 현재 라운드 제안자가 아닙니다.".into());
        }
        Ok(SignedProposal::new(
            block.height,
            self.consensus.round(),
            &self.validator,
            block,
        ))
    }

    pub fn receive_proposal(
        &mut self,
        proposal: SignedProposal,
    ) -> Result<ConsensusMessage, String> {
        let previous = self
            .chain
            .blocks
            .last()
            .ok_or("제네시스 블록이 없습니다.")?;
        if proposal.block.previous_hash != previous.hash
            || proposal.block.height != previous.height + 1
        {
            return Err("제안 블록이 현재 체인의 다음 블록이 아닙니다.".into());
        }
        self.consensus.handle_proposal(&proposal)?;
        self.pending = Some(proposal.block.clone());
        self.deadline = Instant::now() + self.timeout;
        Ok(ConsensusMessage::prevote(
            proposal.height,
            proposal.round,
            &self.validator,
            proposal.block.hash,
        ))
    }

    pub fn receive_vote(
        &mut self,
        vote: ConsensusMessage,
    ) -> Result<Option<ConsensusMessage>, String> {
        self.consensus.handle(vote)?;
        if self.consensus.phase() == ConsensusPhase::Precommit {
            let block_hash = self
                .pending
                .as_ref()
                .ok_or("후보 블록이 없습니다.")?
                .hash
                .clone();
            return Ok(Some(ConsensusMessage::precommit(
                self.pending.as_ref().unwrap().height,
                self.consensus.round(),
                &self.validator,
                block_hash,
            )));
        }
        if self.consensus.phase() == ConsensusPhase::Finalized {
            let block = self.pending.take().ok_or("확정할 후보 블록이 없습니다.")?;
            if self.consensus.finalized_hash() != Some(block.hash.as_str()) {
                return Err("확정 해시와 후보 블록 해시가 다릅니다.".into());
            }
            self.chain.apply_block(block)?;
        }
        Ok(None)
    }

    pub fn timeout_if_due(&mut self, now: Instant) -> Result<bool, String> {
        if now < self.deadline || self.consensus.phase() == ConsensusPhase::Finalized {
            return Ok(false);
        }
        self.consensus.on_timeout()?;
        self.pending = None;
        self.deadline = now + self.timeout;
        Ok(true)
    }

    pub fn force_timeout_for_test(&mut self) -> Result<u32, String> {
        self.pending = None;
        self.consensus.on_timeout()
    }

    /// 현재 확정 높이 뒤의 블록만 제공하며 한 응답을 128개로 제한합니다.
    pub fn blocks_from(&self, from_height: u64) -> Vec<Block> {
        self.chain
            .blocks
            .iter()
            .filter(|b| b.height >= from_height)
            .take(128)
            .cloned()
            .collect()
    }

    /// 제네시스 commitment가 같은 체인의 연속 블록만 적용합니다.
    pub fn apply_sync_batch(&mut self, blocks: Vec<Block>) -> Result<usize, String> {
        let mut applied = 0;
        for block in blocks {
            let next = self.chain.blocks.last().unwrap().height + 1;
            if block.height < next {
                continue;
            }
            if block.height != next {
                return Err("동기화 응답에 블록 높이 공백이 있습니다.".into());
            }
            self.chain.apply_block(block)?;
            applied += 1;
        }
        Ok(applied)
    }

    pub fn phase(&self) -> ConsensusPhase {
        self.consensus.phase()
    }
}
