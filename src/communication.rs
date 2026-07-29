use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

pub const MAX_ENCRYPTED_SIGNAL_BYTES: usize = 64 * 1024;
pub const MAX_SIGNAL_TTL_SECONDS: u64 = 120;
pub const MAX_PENDING_SIGNALS: usize = 256;
const MAX_SEEN_SIGNAL_IDS: usize = 1_024;

/// 지갑 WebRTC 계층이 종단간 암호화한 연결 협상 메시지입니다.
///
/// 노드는 암호문을 복호화하지 않고 대상 peer에게 전달만 합니다. SDP, ICE,
/// 채팅 평문, 음성·영상은 블록과 원장에 기록하지 않습니다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationKind {
    CallInvite,
    CallAccept,
    CallReject,
    WebRtcOffer,
    WebRtcAnswer,
    IceCandidate,
    Hangup,
    EncryptedChat,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommunicationEnvelope {
    pub id: String,
    pub sender_peer_id: String,
    pub target_peer_id: String,
    pub kind: CommunicationKind,
    pub created_at: u64,
    pub expires_at: u64,
    /// X25519/Signal 계열 세션 키로 암호화한 payload의 hex 문자열입니다.
    pub encrypted_payload_hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicationAck {
    pub message_id: String,
    pub accepted: bool,
}

impl CommunicationEnvelope {
    pub fn validate(&self, now: u64) -> Result<(), String> {
        if self.id.len() < 16 || self.id.len() > 128 || !is_safe_id(&self.id) {
            return Err("통신 메시지 id 형식이 올바르지 않습니다.".into());
        }
        if self.target_peer_id.is_empty() || self.target_peer_id.len() > 128 {
            return Err("대상 PeerId가 올바르지 않습니다.".into());
        }
        if self.expires_at <= self.created_at
            || self.expires_at.saturating_sub(self.created_at) > MAX_SIGNAL_TTL_SECONDS
            || self.expires_at < now
        {
            return Err("통신 메시지가 만료되었거나 허용 수명을 초과했습니다.".into());
        }
        if !self.encrypted_payload_hex.len().is_multiple_of(2)
            || self.encrypted_payload_hex.len() / 2 > MAX_ENCRYPTED_SIGNAL_BYTES
            || hex::decode(&self.encrypted_payload_hex).is_err()
        {
            return Err("암호화 payload 형식 또는 크기가 올바르지 않습니다.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct CommunicationInbox {
    items: VecDeque<CommunicationEnvelope>,
    seen: HashSet<String>,
    seen_order: VecDeque<String>,
}

impl CommunicationInbox {
    pub fn push(&mut self, envelope: CommunicationEnvelope, now: u64) -> Result<(), String> {
        envelope.validate(now)?;
        if !self.seen.insert(envelope.id.clone()) {
            return Err("이미 처리한 통신 메시지입니다.".into());
        }
        self.seen_order.push_back(envelope.id.clone());
        while self.seen_order.len() > MAX_SEEN_SIGNAL_IDS {
            if let Some(old_id) = self.seen_order.pop_front() {
                self.seen.remove(&old_id);
            }
        }
        while self.items.len() >= MAX_PENDING_SIGNALS {
            self.items.pop_front();
        }
        self.items.push_back(envelope);
        Ok(())
    }

    pub fn drain(&mut self) -> Vec<CommunicationEnvelope> {
        self.items.drain(..).collect()
    }
}

fn is_safe_id(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> CommunicationEnvelope {
        CommunicationEnvelope {
            id: "call_0123456789abcdef".into(),
            sender_peer_id: String::new(),
            target_peer_id: "12D3KooWTarget".into(),
            kind: CommunicationKind::CallInvite,
            created_at: 100,
            expires_at: 160,
            encrypted_payload_hex: "aabbcc".into(),
        }
    }

    #[test]
    fn accepts_only_short_lived_encrypted_signals_once() {
        let mut inbox = CommunicationInbox::default();
        inbox.push(envelope(), 110).unwrap();
        assert!(inbox.push(envelope(), 110).is_err());
        assert_eq!(inbox.drain().len(), 1);
    }

    #[test]
    fn rejects_plaintext_and_long_lived_payloads() {
        let mut plain = envelope();
        plain.encrypted_payload_hex = "영상 연결해 주세요".into();
        assert!(plain.validate(110).is_err());

        let mut long = envelope();
        long.expires_at = 500;
        assert!(long.validate(110).is_err());
    }
}
