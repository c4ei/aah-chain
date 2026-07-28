use crate::consensus::{ConsensusMessage, DoubleVoteEvidence, FinalityCertificate, SignedProposal};
use crate::model::{Block, Transaction};
use crate::peer_guard::{PeerDecision, PeerGuard};
use crate::snapshot_sync::SyncTip;
use futures::StreamExt;
use libp2p::core::ConnectedPoint;
use libp2p::{
    Multiaddr, PeerId, SwarmBuilder, gossipsub, identify,
    identity::Keypair,
    kad, mdns,
    multiaddr::Protocol,
    swarm::{ConnectionId, NetworkBehaviour, SwarmEvent},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tokio::net::lookup_host;
use tokio::sync::mpsc;

pub const BLOCK_TOPIC: &str = "ieum-chain/blocks/1";
pub const CONSENSUS_TOPIC: &str = "ieum-chain/consensus/1";
pub const SYNC_TOPIC: &str = "ieum-chain/sync/2";

/// P2P 실행 시 바꿀 수 있는 네트워크·방어 설정입니다.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub listen_port: u16,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub identity_key: Option<Keypair>,
    pub max_message_bytes: usize,
    pub idle_timeout: Duration,
    pub ban_duration: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_port: 7001,
            bootstrap_peers: Vec::new(),
            identity_key: None,
            max_message_bytes: 2 * 1024 * 1024,
            idle_timeout: Duration::from_secs(30),
            ban_duration: Duration::from_secs(10 * 60),
        }
    }
}

/// Gossipsub으로 전파하는 메시지의 허용 종류입니다.
/// 합의 메시지는 내부 Ed25519 서명까지 검증한 뒤 상태기계에 전달해야 합니다.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WireMessage {
    Block(Block),
    Transaction(Transaction),
    Proposal(SignedProposal),
    Consensus(ConsensusMessage),
    Evidence(DoubleVoteEvidence),
    SyncRequest {
        requester: String,
        from_height: u64,
    },
    SyncResponse {
        requester: String,
        tip: SyncTip,
        certificates: Vec<FinalityCertificate>,
    },
}

/// 노드 코어가 비동기 P2P 작업에 보내는 명령입니다.
#[derive(Clone, Debug)]
pub enum NetworkCommand {
    PublishBlock(Block),
    PublishTransaction(Transaction),
    PublishConsensus(ConsensusMessage),
    PublishProposal(SignedProposal),
    PublishEvidence(DoubleVoteEvidence),
    RequestSync {
        from_height: u64,
    },
    RespondSync {
        requester: String,
        tip: SyncTip,
        certificates: Vec<FinalityCertificate>,
    },
    Dial(Multiaddr),
    Shutdown,
}

/// P2P 작업이 노드 코어에 전달하는 검증 전 이벤트입니다.
#[derive(Debug)]
pub enum NetworkEvent {
    PeerDiscovered(PeerId),
    OutgoingConnectionFailed {
        peer_id: Option<PeerId>,
        connection_id: String,
        error: String,
    },
    PeerConnected {
        peer_id: PeerId,
        remote_address: Multiaddr,
        remote_ip: Option<String>,
        direction: &'static str,
        connection_id: String,
        current_connections: usize,
    },
    PeerDisconnected {
        peer_id: PeerId,
        remote_address: Multiaddr,
        remote_ip: Option<String>,
        direction: &'static str,
        connection_id: String,
        connected_for: Option<Duration>,
        current_connections: usize,
        cause: Option<String>,
    },
    BlockReceived {
        source: PeerId,
        block: Block,
    },
    TransactionReceived {
        source: PeerId,
        transaction: Transaction,
    },
    ConsensusReceived {
        source: PeerId,
        message: ConsensusMessage,
    },
    ProposalReceived {
        source: PeerId,
        proposal: SignedProposal,
    },
    EvidenceReceived {
        source: PeerId,
        evidence: DoubleVoteEvidence,
    },
    SyncRequested {
        source: PeerId,
        requester: String,
        from_height: u64,
    },
    SyncReceived {
        source: PeerId,
        tip: SyncTip,
        certificates: Vec<FinalityCertificate>,
    },
}

impl fmt::Display for NetworkEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeerConnected {
                peer_id,
                remote_address,
                remote_ip,
                direction,
                connection_id,
                current_connections,
            } => write!(
                formatter,
                "[P2P 연결]\n  방향: {direction}\n  PeerId: {peer_id}\n  원격 주소: {remote_address}\n  원격 IP: {}\n  연결 ID: {connection_id}\n  현재 연결: {current_connections}",
                remote_ip.as_deref().unwrap_or("확인 불가")
            ),
            Self::PeerDisconnected {
                peer_id,
                remote_address,
                remote_ip,
                direction,
                connection_id,
                connected_for,
                current_connections,
                cause,
            } => write!(
                formatter,
                "[P2P 종료]\n  방향: {direction}\n  PeerId: {peer_id}\n  원격 주소: {remote_address}\n  원격 IP: {}\n  연결 ID: {connection_id}\n  연결 시간: {}\n  종료 원인: {}\n  현재 연결: {current_connections}",
                remote_ip.as_deref().unwrap_or("확인 불가"),
                connected_for
                    .map(format_duration)
                    .unwrap_or_else(|| "확인 불가".into()),
                cause.as_deref().unwrap_or("정상 종료")
            ),
            Self::PeerDiscovered(peer_id) => write!(formatter, "[P2P 발견] PeerId: {peer_id}"),
            Self::OutgoingConnectionFailed {
                peer_id,
                connection_id,
                error,
            } => write!(
                formatter,
                "[P2P 접속 실패]\n  PeerId: {}\n  연결 ID: {connection_id}\n  오류: {error}",
                peer_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "확인 불가".into())
            ),
            Self::BlockReceived { source, block } => {
                write!(
                    formatter,
                    "[P2P 블록 수신] PeerId: {source}, 블록: {block:?}"
                )
            }
            Self::TransactionReceived {
                source,
                transaction,
            } => write!(
                formatter,
                "[P2P 거래 수신] PeerId: {source}, 거래: {}",
                transaction.id()
            ),
            Self::ConsensusReceived { source, message } => {
                write!(
                    formatter,
                    "[P2P 합의 수신] PeerId: {source}, 메시지: {message:?}"
                )
            }
            Self::ProposalReceived { source, proposal } => write!(
                formatter,
                "[P2P 제안 수신] PeerId: {source}, 높이: {}, 해시: {}",
                proposal.height, proposal.block.hash
            ),
            Self::EvidenceReceived { source, evidence } => write!(
                formatter,
                "[P2P 이중투표 증거] PeerId: {source}, 증거: {}",
                evidence.id()
            ),
            Self::SyncRequested {
                source,
                from_height,
                ..
            } => write!(
                formatter,
                "[P2P 동기화 요청] PeerId: {source}, 시작 높이: {from_height}"
            ),
            Self::SyncReceived {
                source,
                certificates,
                ..
            } => write!(
                formatter,
                "[P2P 동기화 응답] PeerId: {source}, 확정 블록: {}개",
                certificates.len()
            ),
        }
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "IeumBehaviourEvent")]
struct IeumBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
}

#[derive(Debug)]
enum IeumBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
    Kademlia(kad::Event),
    Identify(Box<identify::Event>),
}

impl From<gossipsub::Event> for IeumBehaviourEvent {
    fn from(value: gossipsub::Event) -> Self {
        Self::Gossipsub(value)
    }
}
impl From<mdns::Event> for IeumBehaviourEvent {
    fn from(value: mdns::Event) -> Self {
        Self::Mdns(value)
    }
}
impl From<kad::Event> for IeumBehaviourEvent {
    fn from(value: kad::Event) -> Self {
        Self::Kademlia(value)
    }
}
impl From<identify::Event> for IeumBehaviourEvent {
    fn from(value: identify::Event) -> Self {
        Self::Identify(Box::new(value))
    }
}

pub struct P2pNode {
    config: NetworkConfig,
}

impl P2pNode {
    pub fn new(config: NetworkConfig) -> Self {
        Self { config }
    }

    /// QUIC 리스너와 피어 검색을 시작하고, 명령/이벤트 채널을 돌려줍니다.
    pub async fn run(
        self,
    ) -> Result<
        (
            PeerId,
            mpsc::Sender<NetworkCommand>,
            mpsc::Receiver<NetworkEvent>,
        ),
        String,
    > {
        let config = self.config;
        let max_message_bytes = config.max_message_bytes;
        let idle_timeout = config.idle_timeout;

        let identity_key = config
            .identity_key
            .unwrap_or_else(Keypair::generate_ed25519);
        let mut swarm = SwarmBuilder::with_existing_identity(identity_key)
            .with_tokio()
            .with_quic()
            .with_behaviour(
                move |key| -> Result<IeumBehaviour, Box<dyn std::error::Error + Send + Sync>> {
                    let peer_id = PeerId::from(key.public());
                    let gossip_config = gossipsub::ConfigBuilder::default()
                        .max_transmit_size(max_message_bytes)
                        .validation_mode(gossipsub::ValidationMode::Strict)
                        .heartbeat_interval(Duration::from_secs(1))
                        .build()?;
                    let mut gossipsub = gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossip_config,
                    )?;
                    gossipsub.subscribe(&gossipsub::IdentTopic::new(BLOCK_TOPIC))?;
                    gossipsub.subscribe(&gossipsub::IdentTopic::new(CONSENSUS_TOPIC))?;
                    gossipsub.subscribe(&gossipsub::IdentTopic::new(SYNC_TOPIC))?;

                    let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
                    let store = kad::store::MemoryStore::new(peer_id);
                    let mut kademlia = kad::Behaviour::new(peer_id, store);
                    kademlia.set_mode(Some(kad::Mode::Server));
                    let identify = identify::Behaviour::new(identify::Config::new(
                        "/ieum-chain/1.1.0".into(),
                        key.public(),
                    ));
                    Ok(IeumBehaviour {
                        gossipsub,
                        mdns,
                        kademlia,
                        identify,
                    })
                },
            )
            .map_err(|error| error.to_string())?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(idle_timeout))
            .build();

        let local_peer_id = *swarm.local_peer_id();
        let listen: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", config.listen_port)
            .parse()
            .map_err(|error| format!("리스닝 주소 오류: {error}"))?;
        swarm.listen_on(listen).map_err(|error| error.to_string())?;

        // 설정에는 /dns4/도메인 주소를 유지하되 QUIC dial 직전에 IPv4로 변환합니다.
        for address in config.bootstrap_peers {
            add_bootstrap_address(&mut swarm, address).await?;
        }
        let _ = swarm.behaviour_mut().kademlia.bootstrap();

        let (command_tx, mut command_rx) = mpsc::channel(128);
        let (event_tx, event_rx) = mpsc::channel(256);
        let mut guard = PeerGuard::new(config.ban_duration);
        let mut connected_at: HashMap<ConnectionId, Instant> = HashMap::new();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    command = command_rx.recv() => {
                        match command {
                            Some(NetworkCommand::PublishBlock(block)) => {
                                publish(&mut swarm, BLOCK_TOPIC, &WireMessage::Block(block));
                            }
                            Some(NetworkCommand::PublishTransaction(transaction)) => {
                                publish(
                                    &mut swarm,
                                    BLOCK_TOPIC,
                                    &WireMessage::Transaction(transaction),
                                );
                            }
                            Some(NetworkCommand::PublishConsensus(message)) => {
                                publish(&mut swarm, CONSENSUS_TOPIC, &WireMessage::Consensus(message));
                            }
                            Some(NetworkCommand::PublishProposal(proposal)) => {
                                publish(&mut swarm, CONSENSUS_TOPIC, &WireMessage::Proposal(proposal));
                            }
                            Some(NetworkCommand::PublishEvidence(evidence)) => {
                                publish(&mut swarm, CONSENSUS_TOPIC, &WireMessage::Evidence(evidence));
                            }
                            Some(NetworkCommand::RequestSync { from_height }) => {
                                publish(
                                    &mut swarm,
                                    SYNC_TOPIC,
                                    &WireMessage::SyncRequest {
                                        requester: local_peer_id.to_string(),
                                        from_height,
                                    },
                                );
                            }
                            Some(NetworkCommand::RespondSync { requester, tip, certificates }) => {
                                publish(
                                    &mut swarm,
                                    SYNC_TOPIC,
                                    &WireMessage::SyncResponse {
                                        requester,
                                        tip,
                                        certificates,
                                    },
                                );
                            }
                            Some(NetworkCommand::Dial(address)) => {
                                if let Err(error) = dial_address(&mut swarm, address).await {
                                    crate::log_error!("{error}");
                                }
                            }
                            Some(NetworkCommand::Shutdown) | None => break,
                        }
                    }
                    event = swarm.select_next_some() => {
                        if let Err(error) = handle_swarm_event(
                            &mut swarm,
                            event,
                            &event_tx,
                            &mut guard,
                            &mut connected_at,
                            max_message_bytes,
                        ).await {
                            crate::log_error!("P2P 이벤트 처리 오류: {error}");
                        }
                    }
                }
            }
        });

        Ok((local_peer_id, command_tx, event_rx))
    }
}

async fn add_bootstrap_address(
    swarm: &mut libp2p::Swarm<IeumBehaviour>,
    address: Multiaddr,
) -> Result<(), String> {
    let original_address = address.clone();
    let resolved_addresses = resolve_dns4_addresses(&address).await?;
    for mut resolved_address in resolved_addresses {
        let peer_id = match resolved_address.pop() {
            Some(Protocol::P2p(peer_id)) => peer_id,
            _ => return Err("부트스트랩 주소 끝에는 /p2p/PeerId가 필요합니다.".into()),
        };
        swarm
            .behaviour_mut()
            .kademlia
            .add_address(&peer_id, resolved_address.clone());
        resolved_address.push(Protocol::P2p(peer_id));
        crate::log_info!("[P2P 접속 시도] {original_address} -> {resolved_address}");
        swarm.dial(resolved_address).map_err(|error| {
            format!("[P2P 접속 시작 실패] 주소: {original_address}, 오류: {error}")
        })?;
    }
    Ok(())
}

async fn dial_address(
    swarm: &mut libp2p::Swarm<IeumBehaviour>,
    address: Multiaddr,
) -> Result<(), String> {
    let resolved_addresses = resolve_dns4_addresses(&address).await?;
    for resolved_address in resolved_addresses {
        crate::log_info!("[P2P 접속 시도] {address} -> {resolved_address}");
        swarm
            .dial(resolved_address)
            .map_err(|error| format!("[P2P 접속 시작 실패] 주소: {address}, 오류: {error}"))?;
    }
    Ok(())
}

/// QUIC transport가 직접 처리하지 못하는 `/dns4/...`를 `/ip4/...`로 변환합니다.
/// 설정에는 도메인을 그대로 두므로 노드를 다시 시작하거나 재접속할 때 최신 IP를 조회합니다.
async fn resolve_dns4_addresses(address: &Multiaddr) -> Result<Vec<Multiaddr>, String> {
    let domain = address.iter().find_map(|protocol| match protocol {
        Protocol::Dns4(domain) => Some(domain.into_owned()),
        _ => None,
    });
    let Some(domain) = domain else {
        return Ok(vec![address.clone()]);
    };

    let resolved = lookup_host((domain.as_str(), 0))
        .await
        .map_err(|error| format!("[P2P DNS 조회 실패] 도메인: {domain}, 오류: {error}"))?;
    let ipv4_addresses: HashSet<_> = resolved
        .filter_map(|socket_address| match socket_address.ip() {
            IpAddr::V4(ip) => Some(ip),
            IpAddr::V6(_) => None,
        })
        .collect();
    if ipv4_addresses.is_empty() {
        return Err(format!(
            "[P2P DNS 조회 실패] 도메인 {domain}에서 IPv4 주소를 찾지 못했습니다."
        ));
    }

    let mut resolved_addresses = Vec::with_capacity(ipv4_addresses.len());
    for ip in ipv4_addresses {
        let mut resolved_address = Multiaddr::empty();
        for protocol in address.iter() {
            match protocol {
                Protocol::Dns4(_) => resolved_address.push(Protocol::Ip4(ip)),
                other => resolved_address.push(other),
            }
        }
        crate::log_info!("[P2P DNS 변환] {domain} -> {ip}");
        resolved_addresses.push(resolved_address);
    }
    resolved_addresses.sort();
    Ok(resolved_addresses)
}

fn publish(swarm: &mut libp2p::Swarm<IeumBehaviour>, topic: &str, message: &WireMessage) {
    let Ok(bytes) = serde_json::to_vec(message) else {
        crate::log_error!("P2P 메시지 직렬화 실패");
        return;
    };
    if let Err(error) = swarm
        .behaviour_mut()
        .gossipsub
        .publish(gossipsub::IdentTopic::new(topic), bytes)
    {
        crate::log_error!("P2P 메시지 전파 실패: {error}");
    }
}

async fn handle_swarm_event(
    swarm: &mut libp2p::Swarm<IeumBehaviour>,
    event: SwarmEvent<IeumBehaviourEvent>,
    event_tx: &mpsc::Sender<NetworkEvent>,
    guard: &mut PeerGuard,
    connected_at: &mut HashMap<ConnectionId, Instant>,
    max_message_bytes: usize,
) -> Result<(), String> {
    match event {
        SwarmEvent::Behaviour(IeumBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
            for (peer, address) in peers {
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
                swarm.behaviour_mut().kademlia.add_address(&peer, address);
                let _ = event_tx.send(NetworkEvent::PeerDiscovered(peer)).await;
            }
        }
        SwarmEvent::Behaviour(IeumBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer, _) in peers {
                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer);
            }
        }
        SwarmEvent::Behaviour(IeumBehaviourEvent::Identify(event)) => {
            if let identify::Event::Received { peer_id, info, .. } = *event {
                for address in info.listen_addrs {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, address);
                }
            }
        }
        SwarmEvent::Behaviour(IeumBehaviourEvent::Gossipsub(gossipsub::Event::Message {
            propagation_source,
            message,
            ..
        })) => {
            let peer_key = propagation_source.to_string();
            if guard.check(&peer_key) == PeerDecision::TemporarilyBlocked {
                return Ok(());
            }
            if message.data.len() > max_message_bytes {
                guard.penalize(&peer_key, 100);
                let _ = swarm.disconnect_peer_id(propagation_source);
                return Err("최대 크기를 넘는 메시지를 보낸 피어를 차단했습니다.".into());
            }
            let decoded: WireMessage = match serde_json::from_slice(&message.data) {
                Ok(value) => value,
                Err(_) => {
                    guard.penalize(&peer_key, 25);
                    return Err("해석할 수 없는 메시지입니다.".into());
                }
            };
            // 여기서는 외부 포맷과 크기까지만 검사합니다.
            // 거래·블록·합의 서명의 의미 검증은 노드 코어에서 수행합니다.
            guard.reward(&peer_key);
            let network_event = match decoded {
                WireMessage::Block(block) => NetworkEvent::BlockReceived {
                    source: propagation_source,
                    block,
                },
                WireMessage::Transaction(transaction) => NetworkEvent::TransactionReceived {
                    source: propagation_source,
                    transaction,
                },
                WireMessage::Consensus(message) => NetworkEvent::ConsensusReceived {
                    source: propagation_source,
                    message,
                },
                WireMessage::Proposal(proposal) => NetworkEvent::ProposalReceived {
                    source: propagation_source,
                    proposal,
                },
                WireMessage::Evidence(evidence) => NetworkEvent::EvidenceReceived {
                    source: propagation_source,
                    evidence,
                },
                WireMessage::SyncRequest {
                    requester,
                    from_height,
                } => NetworkEvent::SyncRequested {
                    source: propagation_source,
                    requester,
                    from_height,
                },
                WireMessage::SyncResponse {
                    requester,
                    tip,
                    certificates,
                } => {
                    if requester != swarm.local_peer_id().to_string() {
                        return Ok(());
                    }
                    NetworkEvent::SyncReceived {
                        source: propagation_source,
                        tip,
                        certificates,
                    }
                }
            };
            event_tx
                .send(network_event)
                .await
                .map_err(|e| e.to_string())?;
        }
        SwarmEvent::ConnectionEstablished {
            peer_id,
            connection_id,
            endpoint,
            num_established,
            ..
        } => {
            connected_at.insert(connection_id, Instant::now());
            let remote_address = endpoint.get_remote_address().clone();
            let _ = event_tx
                .send(NetworkEvent::PeerConnected {
                    peer_id,
                    remote_ip: multiaddr_ip(&remote_address),
                    remote_address,
                    direction: connection_direction(&endpoint),
                    connection_id: format!("{connection_id:?}"),
                    current_connections: num_established.get() as usize,
                })
                .await;
        }
        SwarmEvent::ConnectionClosed {
            peer_id,
            connection_id,
            endpoint,
            num_established,
            cause,
            ..
        } => {
            let remote_address = endpoint.get_remote_address().clone();
            let connected_for = connected_at
                .remove(&connection_id)
                .map(|started| started.elapsed());
            let _ = event_tx
                .send(NetworkEvent::PeerDisconnected {
                    peer_id,
                    remote_ip: multiaddr_ip(&remote_address),
                    remote_address,
                    direction: connection_direction(&endpoint),
                    connection_id: format!("{connection_id:?}"),
                    connected_for,
                    current_connections: num_established as usize,
                    cause: cause.map(|error| error.to_string()),
                })
                .await;
        }
        SwarmEvent::OutgoingConnectionError {
            connection_id,
            peer_id,
            error,
        } => {
            let _ = event_tx
                .send(NetworkEvent::OutgoingConnectionFailed {
                    peer_id,
                    connection_id: format!("{connection_id:?}"),
                    error: error.to_string(),
                })
                .await;
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            crate::log_info!("QUIC P2P 대기: {address}/p2p/{}", swarm.local_peer_id());
        }
        // Kademlia 이벤트는 Behaviour 내부에서 이미 상태에 반영됩니다.
        // 값을 명시적으로 소비해 이벤트 필드가 사용되지 않는다는 경고를 피합니다.
        SwarmEvent::Behaviour(IeumBehaviourEvent::Kademlia(_event)) => {}
        _ => {}
    }
    Ok(())
}

fn connection_direction(endpoint: &ConnectedPoint) -> &'static str {
    match endpoint {
        ConnectedPoint::Dialer { .. } => "발신",
        ConnectedPoint::Listener { .. } => "수신",
    }
}

fn multiaddr_ip(address: &Multiaddr) -> Option<String> {
    address.iter().find_map(|protocol| match protocol {
        Protocol::Ip4(ip) => Some(ip.to_string()),
        Protocol::Ip6(ip) => Some(ip.to_string()),
        _ => None,
    })
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = total_seconds % 3_600 / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}시간 {minutes}분 {seconds}초")
    } else if minutes > 0 {
        format!("{minutes}분 {seconds}초")
    } else {
        format!("{seconds}초")
    }
}

#[cfg(test)]
mod connection_log_tests {
    use super::*;

    #[test]
    fn extracts_ipv4_from_multiaddr() {
        let address: Multiaddr = "/ip4/192.168.1.193/udp/7001/quic-v1".parse().unwrap();
        assert_eq!(multiaddr_ip(&address).as_deref(), Some("192.168.1.193"));
    }

    #[test]
    fn formats_connection_duration() {
        assert_eq!(format_duration(Duration::from_secs(3_725)), "1시간 2분 5초");
    }

    #[tokio::test]
    async fn keeps_ipv4_multiaddr_unchanged() {
        let address: Multiaddr =
            "/ip4/122.35.243.20/udp/7001/quic-v1/p2p/12D3KooWAVRZjnbP8nXp8vD6irYFAXdLJVyczEFWdKLFzKnKDATx"
                .parse()
                .unwrap();
        assert_eq!(
            resolve_dns4_addresses(&address).await.unwrap(),
            vec![address]
        );
    }
}
