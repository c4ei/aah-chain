use clap::{Args as ClapArgs, Parser, Subcommand};
use ieum_chain::{
    ConsensusRuntime, ConsensusTimeouts, EvidenceStore, ExternalSigner, FinalityStore,
    NetworkCommand, NetworkConfig, NetworkEvent, P2pNode, RpcConfig, RpcServer, SyncTip, TipQuorum,
    UpgradeSchedule, Validator, ValidatorSigner, Wallet, log_error, log_info,
    logger::init_server_log, node_key::load_or_create_node_key,
};
use libp2p::Multiaddr;
use serde::Deserialize;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, TcpListener, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_BOOTSTRAP_CONFIG: &str = "config/bootstrap.json";
const DEFAULT_BOOTSTRAP_PEER: &str = "/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/12D3KooWAVRZjnbP8nXp8vD6irYFAXdLJVyczEFWdKLFzKnKDATx";
const SERVER_INSTANCE_PORT: u16 = 49_889;
const CLIENT_INSTANCE_PORT: u16 = 49_890;
const SUPPORTED_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Parser)]
#[command(name = "ieum-chain", version, about = "가벼운 IEUM 테스트넷 노드")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 외부 노드가 접속하는 운영 부트스트랩 서버로 실행합니다.
    Server(NodeArgs),
    /// 일반 PC 노드로 실행하고 운영 서버에 연결합니다.
    Client(ClientArgs),
}

#[derive(Debug, ClapArgs)]
struct NodeArgs {
    /// QUIC UDP 리스닝 포트
    #[arg(long, default_value_t = 7001)]
    port: u16,

    /// 단일 P2P 메시지 최대 크기
    #[arg(long, default_value_t = 2_097_152)]
    max_message_bytes: usize,

    /// 지갑/geth 호환 JSON-RPC TCP 포트
    #[arg(long, default_value_t = 8989)]
    rpc_port: u16,

    /// JSON-RPC 리스닝 IP. 기본값은 외부에 노출되지 않는 localhost입니다.
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    rpc_host: IpAddr,

    /// RPC 원장·체크포인트 저장 경로
    #[arg(long, default_value = "data/ledger")]
    rpc_data_dir: PathBuf,

    /// 재시작 후에도 PeerId를 유지할 영구 노드 키 파일
    #[arg(long, default_value = "data/node.key")]
    node_key: PathBuf,

    /// 4노드 테스트넷 검증자 번호(1~4). server에서만 사용합니다.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=4))]
    validator_index: u8,

    /// 운영 검증자 Ed25519 seed 32바이트 hex 파일
    #[arg(long, default_value = "config/validator.key")]
    validator_key: PathBuf,

    /// 검증자 공개키와 투표권 설정
    #[arg(long, default_value = "config/validators.example.json")]
    validators_config: PathBuf,

    /// 폐쇄형 개발망에서만 고정 검증자 키를 허용합니다.
    #[arg(long, default_value_t = false)]
    allow_insecure_test_keys: bool,

    /// 개인키를 노드 밖에 두는 signer 실행 파일(HSM/Vault adapter)
    #[arg(long, requires = "validator_public_key")]
    validator_signer_command: Option<PathBuf>,

    /// 외부 signer의 Ed25519 공개키 32바이트 hex
    #[arg(long, requires = "validator_signer_command")]
    validator_public_key: Option<String>,

    /// proposal 단계 제한 시간(ms)
    #[arg(long, default_value_t = 3_000)]
    propose_timeout_ms: u64,

    /// prevote 단계 제한 시간(ms)
    #[arg(long, default_value_t = 2_000)]
    prevote_timeout_ms: u64,

    /// precommit 단계 제한 시간(ms)
    #[arg(long, default_value_t = 2_000)]
    precommit_timeout_ms: u64,

    /// 동일 tip/state root 확인에 필요한 독립 피어 수(2~3)
    #[arg(long, default_value_t = 2, value_parser = parse_sync_quorum_peers)]
    sync_quorum_peers: usize,

    /// 서버/클라이언트가 시작할 때 접속할 추가 P2P 주소
    #[arg(long, help = "/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/PeerId")]
    peer: Vec<Multiaddr>,
}

#[derive(Debug, ClapArgs)]
struct ClientArgs {
    #[command(flatten)]
    node: NodeArgs,

    /// 자동으로 읽을 부트스트랩 설정 파일
    #[arg(long, default_value = DEFAULT_BOOTSTRAP_CONFIG)]
    bootstrap_config: PathBuf,
}

fn parse_sync_quorum_peers(value: &str) -> Result<usize, String> {
    let peers = value
        .parse::<usize>()
        .map_err(|_| "sync quorum peers는 2 또는 3이어야 합니다".to_string())?;

    if (2..=3).contains(&peers) {
        Ok(peers)
    } else {
        Err("sync quorum peers는 2 또는 3이어야 합니다".to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();
    let (mode, mut args, bootstrap_peers, is_client) = match args.command {
        Some(Command::Server(mut args)) => {
            if args.node_key == Path::new("data/node.key") {
                args.node_key = PathBuf::from("data/server.node.key");
            }
            let peers = std::mem::take(&mut args.peer);
            ("서버", args, peers, false)
        }
        Some(Command::Client(mut client)) => {
            if client.node.node_key == Path::new("data/node.key") {
                client.node.node_key = PathBuf::from("data/client.node.key");
            }
            if client.node.rpc_data_dir == Path::new("data/ledger") {
                client.node.rpc_data_dir = PathBuf::from("data/client-ledger");
            }
            let peers = load_bootstrap_peers(
                &client.bootstrap_config,
                std::mem::take(&mut client.node.peer),
            )?;
            ("일반 PC", client.node, peers, true)
        }
        None => {
            let node = NodeArgs {
                port: 7001,
                max_message_bytes: 2_097_152,
                rpc_port: 8989,
                rpc_host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                rpc_data_dir: PathBuf::from("data/client-ledger"),
                node_key: PathBuf::from("data/client.node.key"),
                validator_index: 1,
                validator_key: PathBuf::from("config/validator.key"),
                validators_config: PathBuf::from("config/validators.example.json"),
                allow_insecure_test_keys: false,
                validator_signer_command: None,
                validator_public_key: None,
                propose_timeout_ms: 3_000,
                prevote_timeout_ms: 2_000,
                precommit_timeout_ms: 2_000,
                sync_quorum_peers: 2,
                peer: Vec::new(),
            };
            let peers = load_bootstrap_peers(Path::new(DEFAULT_BOOTSTRAP_CONFIG), Vec::new())?;
            ("일반 PC", node, peers, true)
        }
    };
    // 서버는 P2P/RPC 포트 자체가 인스턴스 경계이므로 같은 장비에서 여러 검증자를
    // 실행할 수 있습니다. 일반 PC 클라이언트만 중복 실행을 차단합니다.
    let _instance_guard = if is_client {
        Some(acquire_instance_guard(true)?)
    } else {
        None
    };
    if !is_client {
        init_server_log("data/logs/ieum-chain.log")?;
    }
    prepare_ports(&mut args, is_client)?;

    let identity_key = load_or_create_node_key(&args.node_key)?;
    let config = NetworkConfig {
        listen_port: args.port,
        bootstrap_peers,
        identity_key: Some(identity_key),
        max_message_bytes: args.max_message_bytes,
        idle_timeout: Duration::from_secs(30),
        ban_duration: Duration::from_secs(10 * 60),
    };
    let startup_peers = config.bootstrap_peers.clone();
    let (peer_id, commands, mut events) = P2pNode::new(config).run().await?;

    let rpc_config = RpcConfig {
        listen_ip: args.rpc_host,
        port: args.rpc_port,
        data_dir: args.rpc_data_dir.clone(),
        ..RpcConfig::default()
    };
    let rpc_server = RpcServer::new(rpc_config);
    let rpc = rpc_server.node_handle();
    let mut rpc_task = tokio::spawn(rpc_server.run());
    let validators = load_validators(&args.validators_config)?;
    let local_validator: ValidatorSigner = if is_client {
        Wallet::new().into()
    } else if let (Some(command), Some(public_key)) = (
        args.validator_signer_command.as_ref(),
        args.validator_public_key.as_ref(),
    ) {
        ExternalSigner::new(command, public_key.clone())?.into()
    } else {
        load_validator_wallet(
            &args.validator_key,
            args.validator_index,
            args.allow_insecure_test_keys,
        )?
        .into()
    };
    if !is_client
        && !validators
            .iter()
            .any(|validator| validator.id == local_validator.address())
    {
        return Err("검증자 개인키가 config/validators 목록에 없습니다.".into());
    }
    let local_validator_address = local_validator.address();
    let upgrades = UpgradeSchedule::load("config/upgrades.json")?;
    upgrades.ensure_supported(
        rpc.chain()?.tip_height().saturating_add(1),
        SUPPORTED_PROTOCOL_VERSION,
    )?;
    let mut consensus = ConsensusRuntime::with_signer(
        rpc.chain()?,
        validators.clone(),
        local_validator,
        ConsensusTimeouts {
            propose: Duration::from_millis(args.propose_timeout_ms),
            prevote: Duration::from_millis(args.prevote_timeout_ms),
            precommit: Duration::from_millis(args.precommit_timeout_ms),
        },
    )?;
    let finality_store = FinalityStore::new(&args.rpc_data_dir)?;
    let evidence_store = EvidenceStore::new(&args.rpc_data_dir);
    let evidence_count = evidence_store.load()?.len();
    if evidence_count > 0 {
        log_info!("[BFT 이중투표 증거 복원] {evidence_count}개");
    }
    let imported = consensus.import_certificate_history(finality_store.load(&validators)?)?;
    if imported > 0 {
        log_info!("[BFT 인증서 복원] {imported}개");
    }
    let mut consensus_tick = tokio::time::interval(Duration::from_millis(500));
    let mut sync_quorum = TipQuorum::new(args.sync_quorum_peers)?;

    log_info!("IEUM {mode} 노드 시작: {peer_id}");
    log_info!("영구 노드 키: {}", args.node_key.display());
    log_info!("P2P 포트: {}/UDP", args.port);
    log_info!("RPC 주소: {}:{}", args.rpc_host, args.rpc_port);
    log_info!("원장 경로: {}", args.rpc_data_dir.display());
    if is_client {
        log_info!("운영 서버 자동 연결 대상:");
        for peer in &startup_peers {
            log_info!("  - {peer}");
        }
    }
    log_info!("같은 LAN의 노드는 mDNS로 자동 검색합니다. 종료: Ctrl+C");

    loop {
        tokio::select! {
            _ = consensus_tick.tick() => {
                let timed_out_transactions = consensus.pending_transactions();
                if consensus.timeout_if_due(std::time::Instant::now())? {
                    rpc.restore_transactions(timed_out_transactions)?;
                    log_info!("[BFT 라운드 변경] 단계별 제한 시간 초과, 새 라운드 {}", consensus.round());
                }
                if !is_client {
                    upgrades.ensure_supported(
                        consensus.chain.tip_height().saturating_add(1),
                        SUPPORTED_PROTOCOL_VERSION,
                    )?;
                    let pending = rpc.drain_transactions(1_000)?;
                    if !pending.is_empty() {
                        let previous = consensus.chain.blocks.last().unwrap();
                        let block = ieum_chain::Block::new(
                            previous.height + 1,
                            previous.hash.clone(),
                            unix_timestamp(),
                            local_validator_address.clone(),
                            pending.clone(),
                        );
                        match consensus.make_proposal(block) {
                            Ok(proposal) => {
                                let prevote = consensus.receive_proposal(proposal.clone())?;
                                commands.send(NetworkCommand::PublishProposal(proposal)).await.map_err(|e| e.to_string())?;
                                commands.send(NetworkCommand::PublishConsensus(prevote.clone())).await.map_err(|e| e.to_string())?;
                                if let Some(precommit) = consensus.receive_vote(prevote)? {
                                    commands.send(NetworkCommand::PublishConsensus(precommit.clone())).await.map_err(|e| e.to_string())?;
                                    consensus.receive_vote(precommit)?;
                                }
                                finalize_if_ready(&mut consensus, &rpc, &commands, &finality_store).await?;
                            }
                            Err(_) => {
                                for transaction in &pending {
                                    commands.send(NetworkCommand::PublishTransaction(transaction.clone())).await.map_err(|e| e.to_string())?;
                                }
                                rpc.restore_transactions(pending)?;
                            }
                        }
                    }
                }
            }
            result = &mut rpc_task => {
                return match result {
                    Ok(Ok(())) => Err("JSON-RPC 서버가 예기치 않게 종료되었습니다.".into()),
                    Ok(Err(message)) => Err(message),
                    Err(error) => Err(format!("JSON-RPC 작업 실행 실패: {error}")),
                };
            }
            event = events.recv() => {
                match event {
                    Some(NetworkEvent::PeerConnected { peer_id: connected, remote_address, remote_ip, direction, connection_id, current_connections }) => {
                        rpc.set_peer_count(current_connections)?;
                        log_info!("{}", NetworkEvent::PeerConnected { peer_id: connected, remote_address, remote_ip, direction, connection_id, current_connections });
                        commands.send(NetworkCommand::RequestSync {
                            from_height: consensus.chain.tip_height() + 1,
                        }).await.map_err(|e| e.to_string())?;
                    }
                    Some(NetworkEvent::TransactionReceived { transaction, .. }) => {
                        rpc.restore_transactions(vec![transaction])?;
                    }
                    Some(NetworkEvent::ProposalReceived { proposal, .. }) if !is_client => {
                        match consensus.receive_proposal(proposal) {
                            Ok(prevote) => {
                                commands.send(NetworkCommand::PublishConsensus(prevote.clone())).await.map_err(|e| e.to_string())?;
                                if let Some(precommit) = consensus.receive_vote(prevote)? {
                                    commands.send(NetworkCommand::PublishConsensus(precommit.clone())).await.map_err(|e| e.to_string())?;
                                    consensus.receive_vote(precommit)?;
                                }
                                finalize_if_ready(&mut consensus, &rpc, &commands, &finality_store).await?;
                            }
                            Err(error) => log_error!("[BFT 제안 거부] {error}"),
                        }
                    }
                    Some(NetworkEvent::ConsensusReceived { message, .. }) if !is_client => {
                        match consensus.receive_vote(message) {
                            Ok(Some(precommit)) => {
                                commands.send(NetworkCommand::PublishConsensus(precommit.clone())).await.map_err(|e| e.to_string())?;
                                consensus.receive_vote(precommit)?;
                            }
                            Ok(None) => {}
                            Err(error) => log_error!("[BFT 투표 거부] {error}"),
                        }
                        persist_and_publish_evidence(
                            &mut consensus,
                            &evidence_store,
                            &commands,
                        ).await?;
                        finalize_if_ready(&mut consensus, &rpc, &commands, &finality_store).await?;
                    }
                    Some(NetworkEvent::EvidenceReceived { evidence, .. }) => {
                        let registered = validators
                            .iter()
                            .any(|validator| validator.id == evidence.first.validator_id);
                        if !registered {
                            log_error!("[BFT 이중투표 증거 거부] 등록되지 않은 검증자");
                        } else if evidence_store.append(&evidence)? {
                            log_error!("[BFT 이중투표 증거 저장] {}", evidence.id());
                        }
                    }
                    Some(NetworkEvent::SyncRequested { requester, from_height, .. }) if !is_client => {
                        commands.send(NetworkCommand::RespondSync {
                            requester,
                            tip: SyncTip {
                                height: consensus.chain.tip_height(),
                                block_hash: consensus.chain.tip_hash().to_string(),
                                state_root: consensus.chain.state_hash(),
                            },
                            certificates: consensus.certificates_from(from_height),
                        }).await.map_err(|e| e.to_string())?;
                    }
                    Some(NetworkEvent::SyncReceived { source, tip, certificates }) => {
                        let Some(agreed_tip) = sync_quorum.observe(source.to_string(), tip) else {
                            log_info!("[동기화 교차검증] 두 번째 독립 피어 응답을 기다립니다.");
                            continue;
                        };
                        rpc.begin_sync(agreed_tip.height)?;
                        let mut applied = 0;
                        for certificate in certificates {
                            let chain_before = consensus.chain.clone();
                            let block = certificate.block.clone();
                            if consensus.apply_sync_certificates(vec![certificate.clone()])? == 1 {
                                rpc.install_finalized(
                                    &chain_before,
                                    consensus.chain.clone(),
                                    &block,
                                )?;
                                finality_store.append(&certificate)?;
                                applied += 1;
                            }
                        }
                        if applied > 0 {
                            if consensus.chain.tip_height() == agreed_tip.height
                                && (consensus.chain.tip_hash() != agreed_tip.block_hash
                                    || consensus.chain.state_hash() != agreed_tip.state_root)
                            {
                                return Err("동기화 완료 상태가 피어 quorum의 tip/state root와 다릅니다.".into());
                            }
                            log_info!("[동기화 완료] 확정 블록 {applied}개 적용, 높이 {}", consensus.chain.tip_height());
                            commands.send(NetworkCommand::RequestSync {
                                from_height: consensus.chain.tip_height() + 1,
                            }).await.map_err(|e| e.to_string())?;
                        } else if consensus.chain.tip_height() < agreed_tip.height {
                            commands.send(NetworkCommand::RequestSync {
                                from_height: consensus.chain.tip_height() + 1,
                            }).await.map_err(|e| e.to_string())?;
                        }
                    }
                    Some(NetworkEvent::PeerDisconnected { peer_id, remote_address, remote_ip, direction, connection_id, connected_for, current_connections, cause }) => {
                        rpc.set_peer_count(current_connections)?;
                        log_info!("{}", NetworkEvent::PeerDisconnected {
                            peer_id,
                            remote_address,
                            remote_ip,
                            direction,
                            connection_id,
                            connected_for,
                            current_connections,
                            cause,
                        });
                    }
                    Some(event) => log_info!("{event}"),
                    None => {
                        rpc_task.abort();
                        return Err("P2P 네트워크 이벤트 채널이 종료되었습니다.".into());
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                log_info!("노드를 안전하게 종료합니다.");
                rpc_task.abort();
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod cli_tests {
    use super::parse_sync_quorum_peers;

    #[test]
    fn sync_quorum_peers_accepts_two_or_three() {
        assert_eq!(parse_sync_quorum_peers("2"), Ok(2));
        assert_eq!(parse_sync_quorum_peers("3"), Ok(3));
    }

    #[test]
    fn sync_quorum_peers_rejects_values_outside_range() {
        assert!(parse_sync_quorum_peers("1").is_err());
        assert!(parse_sync_quorum_peers("4").is_err());
        assert!(parse_sync_quorum_peers("invalid").is_err());
    }
}

async fn persist_and_publish_evidence(
    consensus: &mut ConsensusRuntime,
    evidence_store: &EvidenceStore,
    commands: &tokio::sync::mpsc::Sender<NetworkCommand>,
) -> Result<(), String> {
    for evidence in consensus.take_evidence() {
        if evidence_store.append(&evidence)? {
            log_error!("[BFT 이중투표 증거 생성] {}", evidence.id());
            commands
                .send(NetworkCommand::PublishEvidence(evidence))
                .await
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

async fn finalize_if_ready(
    consensus: &mut ConsensusRuntime,
    rpc: &ieum_chain::rpc::RpcNodeHandle,
    commands: &tokio::sync::mpsc::Sender<NetworkCommand>,
    finality_store: &FinalityStore,
) -> Result<(), String> {
    let certificates = consensus.take_finalized();
    if certificates.is_empty() {
        return Ok(());
    }
    for certificate in certificates {
        let chain_before = rpc.chain()?;
        let chain_after = consensus.chain.clone();
        rpc.install_finalized(&chain_before, chain_after, &certificate.block)?;
        finality_store.append(&certificate)?;
        commands
            .send(NetworkCommand::PublishBlock(certificate.block.clone()))
            .await
            .map_err(|error| error.to_string())?;
        log_info!(
            "[BFT 확정] 높이 {}, 해시 {}, precommit {}개",
            certificate.block.height,
            certificate.block.hash,
            certificate.precommits.len()
        );
    }
    consensus.advance_after_finalization()
}

fn testnet_validator_seed(index: u8) -> [u8; 32] {
    [index; 32]
}

fn load_validator_wallet(
    path: &Path,
    index: u8,
    allow_insecure_test_keys: bool,
) -> Result<Wallet, String> {
    if allow_insecure_test_keys {
        log_info!(
            "[경고] 고정 개발 검증자 키 {}번을 사용합니다. 실제 자산을 넣지 마세요.",
            index
        );
        return Ok(Wallet::from_seed(testnet_validator_seed(index)));
    }
    let value = fs::read_to_string(path).map_err(|error| {
        format!(
            "검증자 키 파일을 읽지 못했습니다({}): {error}. 개발망이면 --allow-insecure-test-keys를 명시하세요.",
            path.display()
        )
    })?;
    let seed: [u8; 32] = hex::decode(value.trim().trim_start_matches("0x"))
        .map_err(|_| "검증자 키 파일은 32바이트 hex여야 합니다.")?
        .try_into()
        .map_err(|_| "검증자 키 파일은 정확히 32바이트여야 합니다.")?;
    Ok(Wallet::from_seed(seed))
}

#[derive(Debug, Deserialize)]
struct ValidatorConfig {
    chain_id: String,
    validators: Vec<Validator>,
}

fn load_validators(path: &Path) -> Result<Vec<Validator>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("검증자 설정 읽기 실패({}): {error}", path.display()))?;
    let config: ValidatorConfig = serde_json::from_str(&text)
        .map_err(|error| format!("검증자 설정 형식 오류({}): {error}", path.display()))?;
    if config.chain_id != "21004" {
        return Err("검증자 설정 chain_id는 21004여야 합니다.".into());
    }
    if config.validators.len() < 4 {
        return Err("BFT 검증자는 최소 4개가 필요합니다.".into());
    }
    Ok(config.validators)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

struct InstanceGuard {
    _listener: TcpListener,
}

fn acquire_instance_guard(is_client: bool) -> Result<InstanceGuard, String> {
    let (port, mode) = if is_client {
        (CLIENT_INSTANCE_PORT, "클라이언트")
    } else {
        (SERVER_INSTANCE_PORT, "서버")
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|_| {
        format!(
            "이미 IEUM {mode} 인스턴스가 실행 중입니다. 기존 프로세스를 종료한 뒤 다시 실행하세요."
        )
    })?;
    Ok(InstanceGuard {
        _listener: listener,
    })
}

fn prepare_ports(args: &mut NodeArgs, is_client: bool) -> Result<(), String> {
    if is_client {
        let original_p2p = args.port;
        args.port = first_available_udp_port(args.port, 100)?;
        if args.port != original_p2p {
            log_info!(
                "UDP {original_p2p} 포트가 사용 중이므로 클라이언트 P2P 포트를 {}로 자동 변경합니다.",
                args.port
            );
        }

        let original_rpc = args.rpc_port;
        args.rpc_port = first_available_tcp_port(args.rpc_host, args.rpc_port, 100)?;
        if args.rpc_port != original_rpc {
            log_info!(
                "TCP {original_rpc} 포트가 사용 중이므로 클라이언트 RPC 포트를 {}로 자동 변경합니다.",
                args.rpc_port
            );
        }
    } else {
        ensure_udp_port_available(args.port)?;
        ensure_tcp_port_available(args.rpc_host, args.rpc_port)?;
    }
    Ok(())
}

fn ensure_udp_port_available(port: u16) -> Result<(), String> {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))
        .map(|_| ())
        .map_err(|error| {
            format!(
                "P2P UDP {port} 포트를 사용할 수 없습니다: {error}. 이미 IEUM 노드나 다른 프로그램이 실행 중인지 확인하세요."
            )
        })
}

fn ensure_tcp_port_available(host: IpAddr, port: u16) -> Result<(), String> {
    TcpListener::bind((host, port)).map(|_| ()).map_err(|error| {
        format!(
            "JSON-RPC TCP {host}:{port} 포트를 사용할 수 없습니다: {error}. 이미 IEUM 노드나 다른 프로그램이 실행 중인지 확인하세요."
        )
    })
}

fn first_available_udp_port(start: u16, attempts: u16) -> Result<u16, String> {
    (start..=start.saturating_add(attempts))
        .find(|port| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, *port)).is_ok())
        .ok_or_else(|| {
            format!(
                "사용 가능한 클라이언트 P2P UDP 포트를 찾지 못했습니다({start}~{}).",
                start.saturating_add(attempts)
            )
        })
}

fn first_available_tcp_port(host: IpAddr, start: u16, attempts: u16) -> Result<u16, String> {
    (start..=start.saturating_add(attempts))
        .find(|port| TcpListener::bind((host, *port)).is_ok())
        .ok_or_else(|| {
            format!(
                "사용 가능한 클라이언트 RPC TCP 포트를 찾지 못했습니다({start}~{}).",
                start.saturating_add(attempts)
            )
        })
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BootstrapConfig {
    Addresses(Vec<String>),
    Object { peers: Vec<String> },
}

fn load_bootstrap_peers(
    path: &Path,
    mut command_line_peers: Vec<Multiaddr>,
) -> Result<Vec<Multiaddr>, String> {
    let mut peers = if path.exists() {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("부트스트랩 설정 읽기 실패({}): {error}", path.display()))?;
        let configured: BootstrapConfig = serde_json::from_str(&text)
            .map_err(|error| format!("부트스트랩 설정 형식 오류({}): {error}", path.display()))?;
        let addresses = match configured {
            BootstrapConfig::Addresses(peers) | BootstrapConfig::Object { peers } => peers,
        };
        addresses
            .into_iter()
            .map(|address| {
                address
                    .parse()
                    .map_err(|error| format!("부트스트랩 주소 형식 오류({address}): {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else if path == Path::new(DEFAULT_BOOTSTRAP_CONFIG) {
        vec![
            DEFAULT_BOOTSTRAP_PEER
                .parse()
                .map_err(|error| format!("내장 부트스트랩 주소 오류: {error}"))?,
        ]
    } else if command_line_peers.is_empty() {
        return Err(format!(
            "부트스트랩 설정 파일이 없습니다: {}",
            path.display()
        ));
    } else {
        Vec::new()
    };
    peers.append(&mut command_line_peers);
    peers.sort();
    peers.dedup();
    if peers.is_empty() {
        return Err(format!(
            "부트스트랩 피어가 없습니다. {}에 운영 서버 주소를 등록하세요.",
            path.display()
        ));
    }
    Ok(peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_config_accepts_address_array() {
        let config: BootstrapConfig = serde_json::from_str(
            r#"["/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/12D3KooWAVRZjnbP8nXp8vD6irYFAXdLJVyczEFWdKLFzKnKDATx"]"#,
        )
        .unwrap();
        match config {
            BootstrapConfig::Addresses(peers) => {
                assert_eq!(peers.len(), 1);
                assert_eq!(peers[0], DEFAULT_BOOTSTRAP_PEER);
            }
            BootstrapConfig::Object { .. } => panic!("배열 형식이어야 합니다."),
        }
    }
}
