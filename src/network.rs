use crate::consensus::ConsensusMessage;
use crate::model::Block;
use crate::peer_guard::{PeerDecision, PeerGuard};
use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, SwarmBuilder, gossipsub, identify, kad, mdns,
    multiaddr::Protocol,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

pub const BLOCK_TOPIC: &str = "ieum-chain/blocks/1";
pub const CONSENSUS_TOPIC: &str = "ieum-chain/consensus/1";

/// P2P 실행 시 바꿀 수 있는 네트워크·방어 설정입니다.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub listen_port: u16,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub max_message_bytes: usize,
    pub idle_timeout: Duration,
    pub ban_duration: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_port: 7001,
            bootstrap_peers: Vec::new(),
            max_message_bytes: 512 * 1024,
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
    Consensus(ConsensusMessage),
}

/// 노드 코어가 비동기 P2P 작업에 보내는 명령입니다.
#[derive(Clone, Debug)]
pub enum NetworkCommand {
    PublishBlock(Block),
    PublishConsensus(ConsensusMessage),
    Dial(Multiaddr),
    Shutdown,
}

/// P2P 작업이 노드 코어에 전달하는 검증 전 이벤트입니다.
#[derive(Debug)]
pub enum NetworkEvent {
    PeerDiscovered(PeerId),
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    BlockReceived {
        source: PeerId,
        block: Block,
    },
    ConsensusReceived {
        source: PeerId,
        message: ConsensusMessage,
    },
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
    Identify(identify::Event),
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
        Self::Identify(value)
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

        let mut swarm = SwarmBuilder::with_new_identity()
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

                    let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
                    let store = kad::store::MemoryStore::new(peer_id);
                    let mut kademlia = kad::Behaviour::new(peer_id, store);
                    kademlia.set_mode(Some(kad::Mode::Server));
                    let identify = identify::Behaviour::new(identify::Config::new(
                        "/ieum-chain/1.0.0".into(),
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

        // 외부망에서는 /ip4/주소/udp/포트/quic-v1/p2p/PeerId 형식으로 지정합니다.
        for address in config.bootstrap_peers {
            add_bootstrap_address(&mut swarm, address)?;
        }
        let _ = swarm.behaviour_mut().kademlia.bootstrap();

        let (command_tx, mut command_rx) = mpsc::channel(128);
        let (event_tx, event_rx) = mpsc::channel(256);
        let mut guard = PeerGuard::new(config.ban_duration);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    command = command_rx.recv() => {
                        match command {
                            Some(NetworkCommand::PublishBlock(block)) => {
                                publish(&mut swarm, BLOCK_TOPIC, &WireMessage::Block(block));
                            }
                            Some(NetworkCommand::PublishConsensus(message)) => {
                                publish(&mut swarm, CONSENSUS_TOPIC, &WireMessage::Consensus(message));
                            }
                            Some(NetworkCommand::Dial(address)) => {
                                if let Err(error) = swarm.dial(address) {
                                    eprintln!("피어 연결 시작 실패: {error}");
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
                            max_message_bytes,
                        ).await {
                            eprintln!("P2P 이벤트 처리 오류: {error}");
                        }
                    }
                }
            }
        });

        Ok((local_peer_id, command_tx, event_rx))
    }
}

fn add_bootstrap_address(
    swarm: &mut libp2p::Swarm<IeumBehaviour>,
    mut address: Multiaddr,
) -> Result<(), String> {
    let peer_id = match address.pop() {
        Some(Protocol::P2p(peer_id)) => peer_id,
        _ => return Err("부트스트랩 주소 끝에는 /p2p/PeerId가 필요합니다.".into()),
    };
    swarm
        .behaviour_mut()
        .kademlia
        .add_address(&peer_id, address.clone());
    address.push(Protocol::P2p(peer_id));
    swarm.dial(address).map_err(|error| error.to_string())
}

fn publish(swarm: &mut libp2p::Swarm<IeumBehaviour>, topic: &str, message: &WireMessage) {
    let Ok(bytes) = serde_json::to_vec(message) else {
        eprintln!("P2P 메시지 직렬화 실패");
        return;
    };
    if let Err(error) = swarm
        .behaviour_mut()
        .gossipsub
        .publish(gossipsub::IdentTopic::new(topic), bytes)
    {
        eprintln!("P2P 메시지 전파 실패: {error}");
    }
}

async fn handle_swarm_event(
    swarm: &mut libp2p::Swarm<IeumBehaviour>,
    event: SwarmEvent<IeumBehaviourEvent>,
    event_tx: &mpsc::Sender<NetworkEvent>,
    guard: &mut PeerGuard,
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
        SwarmEvent::Behaviour(IeumBehaviourEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
        })) => {
            for address in info.listen_addrs {
                swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, address);
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
                WireMessage::Consensus(message) => NetworkEvent::ConsensusReceived {
                    source: propagation_source,
                    message,
                },
            };
            event_tx
                .send(network_event)
                .await
                .map_err(|e| e.to_string())?;
        }
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            let _ = event_tx.send(NetworkEvent::PeerConnected(peer_id)).await;
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            let _ = event_tx.send(NetworkEvent::PeerDisconnected(peer_id)).await;
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            println!("QUIC P2P 대기: {address}/p2p/{}", swarm.local_peer_id());
        }
        // Kademlia 이벤트는 Behaviour 내부에서 이미 상태에 반영됩니다.
        // 값을 명시적으로 소비해 이벤트 필드가 사용되지 않는다는 경고를 피합니다.
        SwarmEvent::Behaviour(IeumBehaviourEvent::Kademlia(_event)) => {}
        SwarmEvent::Behaviour(IeumBehaviourEvent::Identify(_)) => {}
        _ => {}
    }
    Ok(())
}
