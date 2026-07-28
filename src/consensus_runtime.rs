use crate::chain::Blockchain;
use crate::consensus::{
    BftConsensus, ConsensusMessage, ConsensusPhase, DoubleVoteEvidence, FinalityCertificate,
    SignedProposal, Validator, VoteType,
};
use crate::model::Block;
use crate::wallet::Wallet;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct ConsensusTimeouts {
    pub propose: Duration,
    pub prevote: Duration,
    pub precommit: Duration,
}

impl ConsensusTimeouts {
    pub fn uniform(timeout: Duration) -> Self {
        Self {
            propose: timeout,
            prevote: timeout,
            precommit: timeout,
        }
    }

    fn for_phase(self, phase: ConsensusPhase) -> Duration {
        match phase {
            ConsensusPhase::Waiting | ConsensusPhase::Propose => self.propose,
            ConsensusPhase::Prevote => self.prevote,
            ConsensusPhase::Precommit | ConsensusPhase::Finalized => self.precommit,
        }
    }
}

/// P2P 어댑터와 독립적으로 테스트 가능한 실제 합의 실행 코어입니다.
/// 후보 블록은 메모리에만 보관하고 2/3 초과 precommit 뒤에만 체인에 저장합니다.
pub struct ConsensusRuntime {
    pub chain: Blockchain,
    consensus: BftConsensus,
    validator: Wallet,
    pending: Option<Block>,
    valid_block: Option<Block>,
    deadline: Instant,
    timeouts: ConsensusTimeouts,
    validators: Vec<Validator>,
    precommits: Vec<ConsensusMessage>,
    finalized: Vec<FinalityCertificate>,
    pending_finalized: Vec<FinalityCertificate>,
}

impl ConsensusRuntime {
    pub fn new(
        chain: Blockchain,
        validators: Vec<Validator>,
        validator: Wallet,
        timeout: Duration,
    ) -> Result<Self, String> {
        Self::with_timeouts(
            chain,
            validators,
            validator,
            ConsensusTimeouts::uniform(timeout),
        )
    }

    pub fn with_timeouts(
        chain: Blockchain,
        validators: Vec<Validator>,
        validator: Wallet,
        timeouts: ConsensusTimeouts,
    ) -> Result<Self, String> {
        let next_height = chain.blocks.last().map(|b| b.height + 1).unwrap_or(1);
        let mut consensus = BftConsensus::new(validators.clone())?;
        consensus.start_round(next_height, 0)?;
        let deadline = Instant::now() + timeouts.propose;
        Ok(Self {
            chain,
            consensus,
            validator,
            pending: None,
            valid_block: None,
            deadline,
            timeouts,
            validators,
            precommits: Vec::new(),
            finalized: Vec::new(),
            pending_finalized: Vec::new(),
        })
    }

    pub fn make_proposal(&self, block: Block) -> Result<SignedProposal, String> {
        if self.validator.address() != self.consensus.expected_proposer() {
            return Err("이 노드는 현재 라운드 제안자가 아닙니다.".into());
        }
        let (block, valid_round) = match (
            self.valid_block.as_ref(),
            self.consensus.valid_value(),
        ) {
            (Some(valid_block), Some((valid_hash, valid_round)))
                if valid_block.hash == valid_hash =>
            {
                (valid_block.clone(), Some(valid_round))
            }
            _ => (block, None),
        };
        Ok(SignedProposal::with_valid_round(
            block.height,
            self.consensus.round(),
            &self.validator,
            block,
            valid_round,
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
        self.reset_deadline();
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
        let previous_phase = self.consensus.phase();
        self.consensus.handle(vote.clone())?;
        if vote.vote_type == VoteType::Precommit
            && !self.precommits.iter().any(|existing| {
                existing.height == vote.height
                    && existing.round == vote.round
                    && existing.validator_id == vote.validator_id
            })
        {
            self.precommits.push(vote);
        }
        if previous_phase != self.consensus.phase() {
            if previous_phase == ConsensusPhase::Prevote
                && self.consensus.phase() == ConsensusPhase::Precommit
            {
                self.valid_block = self.pending.clone();
            }
            self.reset_deadline();
        }
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
            // 네트워크에서 같은 precommit이 다시 도착해도 이미 적용한 블록을
            // 중복 저장하거나 정상 노드를 오류 상태로 만들지 않습니다.
            let Some(block) = self.pending.take() else {
                return Ok(None);
            };
            if self.consensus.finalized_hash() != Some(block.hash.as_str()) {
                return Err("확정 해시와 후보 블록 해시가 다릅니다.".into());
            }
            let certificate = FinalityCertificate {
                block: block.clone(),
                round: self.consensus.round(),
                precommits: self.precommits.clone(),
            };
            certificate.verify(&self.validators)?;
            self.chain.apply_block(block)?;
            self.finalized.push(certificate.clone());
            self.pending_finalized.push(certificate);
        }
        Ok(None)
    }

    pub fn timeout_if_due(&mut self, now: Instant) -> Result<bool, String> {
        if now < self.deadline || self.consensus.phase() == ConsensusPhase::Finalized {
            return Ok(false);
        }
        self.consensus.on_timeout()?;
        self.pending = None;
        self.precommits.clear();
        self.deadline = now + self.timeouts.propose;
        Ok(true)
    }

    pub fn pending_transactions(&self) -> Vec<crate::model::Transaction> {
        self.pending
            .as_ref()
            .map(|block| block.transactions.clone())
            .unwrap_or_default()
    }

    pub fn force_timeout_for_test(&mut self) -> Result<u32, String> {
        self.pending = None;
        self.consensus.on_timeout()
    }

    pub fn take_evidence(&mut self) -> Vec<DoubleVoteEvidence> {
        self.consensus.take_evidence()
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

    pub fn certificates_from(&self, from_height: u64) -> Vec<FinalityCertificate> {
        const MAX_SYNC_BYTES: usize = 1_500_000;
        let mut bytes: usize = 0;
        let mut selected = Vec::new();
        for certificate in self
            .finalized
            .iter()
            .filter(|certificate| certificate.block.height >= from_height)
            .take(128)
        {
            let size = serde_json::to_vec(certificate)
                .map(|value| value.len())
                .unwrap_or(MAX_SYNC_BYTES + 1);
            if !selected.is_empty() && bytes.saturating_add(size) > MAX_SYNC_BYTES {
                break;
            }
            bytes = bytes.saturating_add(size);
            selected.push(certificate.clone());
        }
        selected
    }

    pub fn import_certificate_history(
        &mut self,
        certificates: Vec<FinalityCertificate>,
    ) -> Result<usize, String> {
        let mut imported = 0;
        for certificate in certificates {
            certificate.verify(&self.validators)?;
            let Some(block) = self.chain.block_by_height(certificate.block.height) else {
                continue;
            };
            if block.hash != certificate.block.hash {
                return Err("원장 블록과 확정 인증서 해시가 다릅니다.".into());
            }
            if !self
                .finalized
                .iter()
                .any(|known| known.block.height == certificate.block.height)
            {
                self.finalized.push(certificate);
                imported += 1;
            }
        }
        self.finalized
            .sort_by_key(|certificate| certificate.block.height);
        Ok(imported)
    }

    pub fn take_finalized(&mut self) -> Vec<FinalityCertificate> {
        std::mem::take(&mut self.pending_finalized)
    }

    /// 확정 인증서를 검증하고 현재 tip 바로 다음 블록만 순서대로 적용합니다.
    pub fn apply_sync_certificates(
        &mut self,
        certificates: Vec<FinalityCertificate>,
    ) -> Result<usize, String> {
        let mut applied = 0;
        for certificate in certificates {
            certificate.verify(&self.validators)?;
            let next = self.chain.blocks.last().unwrap().height + 1;
            if certificate.block.height < next {
                let canonical = self
                    .chain
                    .block_by_height(certificate.block.height)
                    .ok_or("기존 canonical 블록을 찾을 수 없습니다.")?;
                if canonical.hash != certificate.block.hash {
                    return Err(format!(
                        "확정성 위반: 높이 {}에 서로 다른 2/3 인증서가 존재합니다.",
                        certificate.block.height
                    ));
                }
                continue;
            }
            if certificate.block.height != next {
                return Err("동기화 응답에 블록 높이 공백이 있습니다.".into());
            }
            self.chain.apply_block(certificate.block.clone())?;
            self.finalized.push(certificate);
            applied += 1;
        }
        if applied > 0 {
            let next_height = self.chain.blocks.last().unwrap().height + 1;
            self.consensus.start_round(next_height, 0)?;
            self.pending = None;
            self.valid_block = None;
            self.precommits.clear();
            self.reset_deadline();
        }
        Ok(applied)
    }

    /// 확정 블록 저장 후 다음 높이의 0라운드를 시작합니다.
    pub fn advance_after_finalization(&mut self) -> Result<(), String> {
        if self.consensus.phase() != ConsensusPhase::Finalized {
            return Err("확정되지 않은 상태에서는 다음 높이로 이동할 수 없습니다.".into());
        }
        let next_height = self.chain.blocks.last().unwrap().height + 1;
        self.consensus.start_round(next_height, 0)?;
        self.pending = None;
        self.valid_block = None;
        self.precommits.clear();
        self.reset_deadline();
        Ok(())
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

    pub fn round(&self) -> u32 {
        self.consensus.round()
    }

    pub fn locked_value(&self) -> Option<(&str, u32)> {
        self.consensus.locked_value()
    }

    pub fn valid_value(&self) -> Option<(&str, u32)> {
        self.consensus.valid_value()
    }

    fn reset_deadline(&mut self) {
        self.deadline = Instant::now() + self.timeouts.for_phase(self.consensus.phase());
    }
}
