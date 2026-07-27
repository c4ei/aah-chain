use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::model::Block;
use crate::wallet::{verify_signature, Wallet};

/// 검증자와 투표권입니다. 현재 예제에서는 stake를 투표 가중치로 사용합니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Validator {
    pub id: String,
    pub voting_power: u64,
}

impl Validator {
    pub fn new(id: impl Into<String>, voting_power: u64) -> Self {
        Self {
            id: id.into(),
            voting_power,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConsensusPhase {
    Waiting,
    Propose,
    Prevote,
    Precommit,
    Finalized,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VoteType {
    Prevote,
    Precommit,
}

/// 제안자가 서명한 후보 블록입니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedProposal {
    pub height: u64,
    pub round: u32,
    pub proposer_id: String,
    pub block: Block,
    pub signature: String,
}

impl SignedProposal {
    pub fn new(height: u64, round: u32, proposer: &Wallet, block: Block) -> Self {
        let proposer_id = proposer.address();
        let signature = proposer.sign_bytes(&Self::unsigned_bytes(
            height,
            round,
            &proposer_id,
            &block.hash,
        ));
        Self {
            height,
            round,
            proposer_id,
            block,
            signature,
        }
    }

    fn unsigned_bytes(
        height: u64,
        round: u32,
        proposer_id: &str,
        block_hash: &str,
    ) -> Vec<u8> {
        let mut bytes = b"IEUM-PROPOSAL-V1".to_vec();
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&round.to_be_bytes());
        push_text(&mut bytes, proposer_id);
        push_text(&mut bytes, block_hash);
        bytes
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.height != self.block.height {
            return Err("제안 높이와 블록 높이가 다릅니다.".into());
        }
        if self.block.hash != self.block.calculate_hash() {
            return Err("제안 블록 해시가 올바르지 않습니다.".into());
        }
        verify_signature(
            &self.proposer_id,
            &Self::unsigned_bytes(
                self.height,
                self.round,
                &self.proposer_id,
                &self.block.hash,
            ),
            &self.signature,
        )
    }
}

/// 네트워크로 전달되는 BFT 합의 메시지입니다.
/// 운영 버전에서는 validator_id가 아니라 검증자 키의 서명을 반드시 검증해야 합니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsensusMessage {
    pub height: u64,
    pub round: u32,
    pub validator_id: String,
    pub vote_type: VoteType,
    pub block_hash: String,
    /// validator_id에 해당하는 Ed25519 개인키로 만든 서명입니다.
    pub signature: String,
}

impl ConsensusMessage {
    fn unsigned_bytes(
        height: u64,
        round: u32,
        validator_id: &str,
        vote_type: VoteType,
        block_hash: &str,
    ) -> Vec<u8> {
        // 체인 간 재사용 공격을 막기 위한 도메인 구분 문자열입니다.
        let mut bytes = b"IEUM-CONSENSUS-V1".to_vec();
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&round.to_be_bytes());
        bytes.push(match vote_type {
            VoteType::Prevote => 1,
            VoteType::Precommit => 2,
        });
        push_text(&mut bytes, validator_id);
        push_text(&mut bytes, block_hash);
        bytes
    }

    /// 검증자 개인키로 서명된 prevote를 생성합니다.
    pub fn prevote(
        height: u64,
        round: u32,
        validator: &Wallet,
        block_hash: impl Into<String>,
    ) -> Self {
        Self::signed(height, round, validator, VoteType::Prevote, block_hash.into())
    }

    /// 검증자 개인키로 서명된 precommit을 생성합니다.
    pub fn precommit(
        height: u64,
        round: u32,
        validator: &Wallet,
        block_hash: impl Into<String>,
    ) -> Self {
        Self::signed(height, round, validator, VoteType::Precommit, block_hash.into())
    }

    fn signed(
        height: u64,
        round: u32,
        validator: &Wallet,
        vote_type: VoteType,
        block_hash: String,
    ) -> Self {
        let validator_id = validator.address();
        let signature = validator.sign_bytes(&Self::unsigned_bytes(
            height,
            round,
            &validator_id,
            vote_type,
            &block_hash,
        ));
        Self {
            height,
            round,
            validator_id,
            vote_type,
            block_hash,
            signature,
        }
    }

    /// 네트워크에서 받은 투표가 실제 등록 검증자의 서명인지 확인합니다.
    pub fn verify(&self) -> Result<(), String> {
        verify_signature(
            &self.validator_id,
            &Self::unsigned_bytes(
                self.height,
                self.round,
                &self.validator_id,
                self.vote_type,
                &self.block_hash,
            ),
            &self.signature,
        )
    }
}

fn push_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

/// Tendermint 계열의 propose → prevote → precommit 흐름을 작게 구현한 상태기계입니다.
///
/// 이 코드는 네트워크 학습 및 테스트넷용입니다. 투표 서명과 로컬 WAL은
/// 구현되어 있지만 잠금 규칙, 라운드 변경, 외부 이중서명 증거와 slashing은
/// 다음 단계에서 추가합니다.
#[derive(Clone, Debug)]
pub struct BftConsensus {
    validators: HashMap<String, u64>,
    total_power: u64,
    height: u64,
    round: u32,
    phase: ConsensusPhase,
    proposal: Option<String>,
    votes: HashMap<(VoteType, String), HashSet<String>>,
    /// (높이, 라운드, 투표 종류, 검증자)별 첫 투표를 보관해 이중투표를 거부합니다.
    vote_history: HashMap<(u64, u32, VoteType, String), String>,
    finalized: Option<String>,
}

impl BftConsensus {
    pub fn new(validators: Vec<Validator>) -> Result<Self, String> {
        if validators.len() < 4 {
            return Err("BFT 테스트넷은 최소 4개 검증자를 권장합니다.".into());
        }
        let mut map = HashMap::new();
        for validator in validators {
            if validator.voting_power == 0 || map.contains_key(&validator.id) {
                return Err("검증자 ID는 고유해야 하고 투표권은 1 이상이어야 합니다.".into());
            }
            map.insert(validator.id, validator.voting_power);
        }
        let total_power = map.values().sum();
        Ok(Self {
            validators: map,
            total_power,
            height: 0,
            round: 0,
            phase: ConsensusPhase::Waiting,
            proposal: None,
            votes: HashMap::new(),
            vote_history: HashMap::new(),
            finalized: None,
        })
    }

    pub fn start_round(&mut self, height: u64, round: u32) -> Result<(), String> {
        if height < self.height || (height == self.height && round < self.round) {
            return Err("과거 높이 또는 과거 라운드로 돌아갈 수 없습니다.".into());
        }
        self.height = height;
        self.round = round;
        self.phase = ConsensusPhase::Propose;
        self.proposal = None;
        // 이전 라운드의 투표는 quorum 계산에서는 제외하지만, 이중투표 증거용 기록은 유지합니다.
        self.votes.clear();
        self.finalized = None;
        Ok(())
    }

    pub fn expected_proposer(&self) -> &str {
        let mut ids: Vec<_> = self.validators.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids[((self.height.saturating_sub(1) + self.round as u64) as usize) % ids.len()]
    }

    pub fn propose(&mut self, proposer: &str, block_hash: &str) -> Result<(), String> {
        if self.phase != ConsensusPhase::Propose {
            return Err("현재 단계에서는 블록을 제안할 수 없습니다.".into());
        }
        if proposer != self.expected_proposer() {
            return Err(format!("이번 라운드의 제안자는 {}입니다.", self.expected_proposer()));
        }
        if block_hash.is_empty() {
            return Err("빈 블록 해시는 제안할 수 없습니다.".into());
        }
        self.proposal = Some(block_hash.to_string());
        self.phase = ConsensusPhase::Prevote;
        Ok(())
    }

    /// 네트워크에서 받은 제안자의 서명과 현재 높이/라운드를 확인합니다.
    pub fn handle_proposal(&mut self, proposal: &SignedProposal) -> Result<(), String> {
        if proposal.height != self.height || proposal.round != self.round {
            return Err("현재 높이/라운드와 다른 제안입니다.".into());
        }
        if !self.validators.contains_key(&proposal.proposer_id) {
            return Err("등록되지 않은 검증자의 제안입니다.".into());
        }
        proposal.verify()?;
        self.propose(&proposal.proposer_id, &proposal.block.hash)
    }

    pub fn handle(&mut self, message: ConsensusMessage) -> Result<(), String> {
        if message.height != self.height || message.round != self.round {
            return Err("현재 높이/라운드와 다른 투표입니다.".into());
        }
        if !self.validators.contains_key(&message.validator_id) {
            return Err("등록되지 않은 검증자의 투표입니다.".into());
        }
        message.verify()?;
        match (self.phase, message.vote_type) {
            (ConsensusPhase::Prevote, VoteType::Prevote)
            | (ConsensusPhase::Precommit, VoteType::Prevote)
            | (ConsensusPhase::Precommit, VoteType::Precommit)
            | (ConsensusPhase::Finalized, VoteType::Prevote)
            | (ConsensusPhase::Finalized, VoteType::Precommit) => {}
            _ => return Err("현재 합의 단계와 투표 종류가 일치하지 않습니다.".into()),
        }

        let history_key = (
            message.height,
            message.round,
            message.vote_type,
            message.validator_id.clone(),
        );
        if let Some(previous_hash) = self.vote_history.get(&history_key) {
            if previous_hash != &message.block_hash {
                return Err("동일 높이·라운드에서 서로 다른 블록에 이중투표했습니다.".into());
            }
        } else {
            self.vote_history
                .insert(history_key, message.block_hash.clone());
        }
        if self.proposal.as_deref() != Some(message.block_hash.as_str()) {
            return Err("현재 제안과 다른 블록에 대한 투표입니다.".into());
        }

        let key = (message.vote_type, message.block_hash.clone());
        self.votes.entry(key.clone()).or_default().insert(message.validator_id);
        if self.has_quorum(&key) && self.phase != ConsensusPhase::Finalized {
            match message.vote_type {
                VoteType::Prevote if self.phase == ConsensusPhase::Prevote => {
                    self.phase = ConsensusPhase::Precommit
                }
                VoteType::Precommit => {
                    self.phase = ConsensusPhase::Finalized;
                    self.finalized = Some(message.block_hash);
                }
                VoteType::Prevote => {}
            }
        }
        Ok(())
    }

    fn has_quorum(&self, key: &(VoteType, String)) -> bool {
        let power: u64 = self
            .votes
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(|id| self.validators.get(id))
            .sum();
        // 정확히 2/3는 부족하고, Byzantine fault tolerance에는 2/3 초과가 필요합니다.
        power.saturating_mul(3) > self.total_power.saturating_mul(2)
    }

    pub fn phase(&self) -> ConsensusPhase {
        self.phase
    }

    pub fn round(&self) -> u32 {
        self.round
    }

    /// 제한 시간 안에 합의하지 못하면 같은 높이의 다음 라운드를 시작합니다.
    pub fn on_timeout(&mut self) -> Result<u32, String> {
        let next_round = self
            .round
            .checked_add(1)
            .ok_or("합의 라운드 번호가 범위를 넘었습니다.")?;
        self.start_round(self.height, next_round)?;
        Ok(next_round)
    }

    pub fn finalized_hash(&self) -> Option<&str> {
        self.finalized.as_deref()
    }
}
