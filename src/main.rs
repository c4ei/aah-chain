use clap::{Args as ClapArgs, Parser, Subcommand};
use ieum_chain::{
    ConsensusRuntime, ConsensusTimeouts, EventSchedule, EvidenceStore, ExternalSigner,
    FinalityStore, NetworkCommand, NetworkConfig, NetworkEvent, P2pNode, RpcConfig, RpcServer,
    SyncTip, TipQuorum, UpgradeSchedule, Validator, ValidatorRegistration, ValidatorSigner, Wallet,
    log_error, log_info, logger::init_server_log, node_key::load_or_create_node_key,
};
use libp2p::{Multiaddr, multiaddr::Protocol};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_BOOTSTRAP_CONFIG: &str = "config/bootstrap.json";
const DEFAULT_BOOTSTRAP_PEER: &str = "/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/12D3KooWCngLfRL315jgBHezczSQtgsqjcVqvHMbhmUibkRT7Veb";
const SERVER_INSTANCE_PORT: u16 = 49_889;
const CLIENT_INSTANCE_PORT: u16 = 49_890;
const SUPPORTED_PROTOCOL_VERSION: u32 = 2;

mod installation;

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
    /// 서명 manifest를 검사하고 실행파일만 비대화형으로 교체합니다.
    /// 서버는 systemd가 중지한 상태에서 호출해야 합니다.
    Update {
        /// 서명된 업데이트 manifest URL
        #[arg(long)]
        manifest_url: String,
        /// manifest를 검증할 IEUM 릴리스 Ed25519 공개키(32바이트 hex)
        #[arg(long)]
        release_public_key: String,
    },
    /// 운영 검증자 Ed25519 키와 공개키 설정을 관리합니다.
    ValidatorKey {
        #[command(subcommand)]
        command: ValidatorKeyCommand,
    },
    /// 신규 노드 초기화와 기존 노드 상태 검증을 수행합니다.
    Node {
        #[command(subcommand)]
        command: NodeCommand,
    },
}

#[derive(Debug, Subcommand)]
enum NodeCommand {
    /// 기존 상태를 자동 백업하고 완전한 신규 서버 노드를 초기화합니다.
    Init {
        /// 기존 노드 복구가 아닌 신규 노드 생성을 명시적으로 확인합니다.
        #[arg(long, required = true)]
        new: bool,
    },
    /// 기존 서버 노드의 원장, validator.key, server.node.key 일치를 검사합니다.
    Verify,
}

#[derive(Debug, Subcommand)]
enum ValidatorKeyCommand {
    /// 운영체제 난수로 검증자 개인 seed 파일을 안전하게 생성합니다.
    Generate {
        /// 생성할 개인키 파일
        #[arg(long, default_value = "config/validator.key")]
        output: PathBuf,
    },
    /// 개인 seed 파일에서 validators.json에 넣을 공개키를 출력합니다.
    Public {
        /// 읽을 개인키 파일
        #[arg(long, default_value = "config/validator.key")]
        key: PathBuf,
    },
    /// 공개키 4개 이상으로 모든 노드가 공유할 운영 설정을 생성합니다.
    CreateConfig {
        /// 검증자 Ed25519 공개키 32바이트 hex. 검증자 순서대로 반복합니다.
        #[arg(long = "public-key", required = true, num_args = 4..)]
        public_keys: Vec<String>,

        /// 각 검증자의 투표권
        #[arg(long, default_value_t = 100)]
        voting_power: u64,

        /// 생성할 운영 검증자 설정
        #[arg(long, default_value = "config/validators.json")]
        output: PathBuf,
    },
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
    #[arg(long, default_value = "config/validators.json")]
    validators_config: PathBuf,

    /// 검증자 전원이 동일하게 배포하는 승인된 시간 기반 이벤트 설정
    #[arg(long, default_value = "config/events.json")]
    events_config: PathBuf,

    /// 서명된 업데이트 manifest URL. 지정한 경우 시작할 때 새 버전을 확인합니다.
    #[arg(long, requires = "release_public_key")]
    update_manifest_url: Option<String>,

    /// manifest를 검증할 IEUM 릴리스 Ed25519 공개키(32바이트 hex)
    #[arg(long, requires = "update_manifest_url")]
    release_public_key: Option<String>,

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
        Some(Command::ValidatorKey { command }) => return run_validator_key_command(command),
        Some(Command::Node { command }) => return run_node_command(command),
        Some(Command::Update {
            manifest_url,
            release_public_key,
        }) => {
            return match ieum_chain::updater::install_non_interactive(
                &manifest_url,
                &release_public_key,
            )? {
                ieum_chain::updater::UpdateResult::Current => {
                    println!("현재 버전이 최신입니다.");
                    Ok(())
                }
                ieum_chain::updater::UpdateResult::Installed => {
                    println!("서명된 업데이트를 설치했습니다.");
                    Ok(())
                }
            };
        }
        Some(Command::Server(mut args)) => {
            if args.node_key == Path::new("data/node.key") {
                args.node_key = PathBuf::from("data/server.node.key");
            }
            let mut peers = std::mem::take(&mut args.peer);
            if peers.is_empty() {
                peers.push(
                    DEFAULT_BOOTSTRAP_PEER
                        .parse()
                        .map_err(|error| format!("기본 부트스트랩 주소 오류: {error}"))?,
                );
            }
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
                validators_config: PathBuf::from("config/validators.json"),
                events_config: PathBuf::from("config/events.json"),
                update_manifest_url: None,
                release_public_key: None,
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
        installation::prepare_server_files(
            &args.validator_key,
            &args.node_key,
            &args.rpc_data_dir,
            &args.validators_config,
            &args.events_config,
            Path::new("config/upgrades.json"),
            args.allow_insecure_test_keys,
        )?;
    }
    prepare_ports(&mut args, is_client)?;
    if let (Some(url), Some(public_key)) = (
        args.update_manifest_url.as_deref(),
        args.release_public_key.as_deref(),
    ) && let Err(error) = ieum_chain::updater::check_and_prompt(url, public_key, is_client)
    {
        log_error!("[업데이트 확인 실패] {error}. 현재 버전으로 계속 실행합니다.");
    }

    let identity_key = load_or_create_node_key(&args.node_key)?;
    let local_peer_id = libp2p::PeerId::from(identity_key.public());
    let bootstrap_peers = bootstrap_peers
        .into_iter()
        .filter(|address| multiaddr_peer_id(address).as_ref() != Some(&local_peer_id))
        .collect();
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
    let mut validators = load_validators(&args.validators_config)?;
    if !is_client && validators.len() < 4 {
        log_info!(
            "[부트스트랩 합의] 현재 검증자 {}명입니다. 4명 이상 등록되기 전에는 \
             장애 허용 BFT가 아니라 개발·초기 구성 모드로 동작합니다.",
            validators.len()
        );
    }
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
    let local_validator_address = local_validator.address();
    let mut local_is_validator = !is_client
        && validators
            .iter()
            .any(|validator| validator.id == local_validator_address);
    if !is_client {
        if local_is_validator {
            log_info!(
                "[합의 참여] 로컬 검증자 {}가 현재 검증자 집합에 등록되어 있습니다.",
                local_validator_address
            );
        } else {
            log_info!(
                "[일반 노드 시작] 로컬 검증자 {}는 아직 등록되지 않았습니다. \
                 P2P 동기화와 서명 후보 등록은 계속하며 합의 투표에는 참여하지 않습니다.",
                local_validator_address
            );
        }
    }
    let local_registration = if is_client {
        None
    } else {
        Some(ValidatorRegistration {
            validator_id: local_validator_address.clone(),
            peer_id: peer_id.to_string(),
            signature_hex: local_validator.sign_bytes(&ValidatorRegistration::bytes_to_sign(
                &local_validator_address,
                &peer_id.to_string(),
            ))?,
        })
    };
    let mut registrations = BTreeMap::new();
    if let Some(registration) = &local_registration {
        registrations.insert(registration.validator_id.clone(), registration.clone());
    }
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
    let event_schedule = EventSchedule::load(&args.events_config)?;
    consensus.set_event_schedule(event_schedule.clone())?;
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
    let mut registration_tick = tokio::time::interval(Duration::from_secs(2));
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
            _ = registration_tick.tick(), if !is_client && !local_is_validator => {
                if let Some(registration) = &local_registration {
                    commands
                        .send(NetworkCommand::PublishValidatorRegistration(registration.clone()))
                        .await
                        .map_err(|error| error.to_string())?;
                }
            }
            _ = consensus_tick.tick() => {
                for envelope in rpc.drain_outbound_communication()? {
                    commands
                        .send(NetworkCommand::SendCommunication(envelope))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                let timed_out_transactions = consensus.pending_transactions();
                let consensus_is_active = local_is_validator
                    && (consensus.phase() != ieum_chain::ConsensusPhase::Propose
                        || rpc.has_pending_transactions()?);
                if consensus_is_active
                    && consensus.timeout_if_due(std::time::Instant::now())?
                {
                    rpc.restore_transactions(timed_out_transactions)?;
                    print!(
                        "\r\x1b[2K[BFT 라운드 변경] 단계별 제한 시간 초과, 새 라운드 {}",
                        consensus.round()
                    );
                    io::stdout().flush().map_err(|error| error.to_string())?;
                }
                if !is_client && local_is_validator {
                    upgrades.ensure_supported(
                        consensus.chain.tip_height().saturating_add(1),
                        SUPPORTED_PROTOCOL_VERSION,
                    )?;
                    let pending = rpc.drain_transactions(1_000)?;
                    let timestamp = unix_timestamp();
                    let due_events =
                        event_schedule.due(timestamp, consensus.chain.executed_events());
                    if !pending.is_empty() || !due_events.is_empty() {
                        let previous = consensus.chain.blocks.last().unwrap();
                        let block = ieum_chain::Block::new(
                            previous.height + 1,
                            previous.hash.clone(),
                            timestamp,
                            local_validator_address.clone(),
                            pending.clone(),
                        )
                        .with_system_events(due_events);
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
                        if let Some(registration) = &local_registration {
                            commands.send(NetworkCommand::PublishValidatorRegistration(registration.clone()))
                                .await.map_err(|e| e.to_string())?;
                        }
                    }
                    Some(NetworkEvent::ValidatorRegistrationReceived { source, registration }) if !is_client => {
                        ieum_chain::logger::write_repeated_info(&format!(
                            "[검증자 등록 수신] PeerId: {source}, 검증자: {}",
                            registration.validator_id
                        ));
                        if let Err(error) = verify_validator_registration(&registration) {
                            log_error!("[검증자 자동 등록 거부] {error}");
                            continue;
                        }
                        let registration_id = registration.validator_id.clone();
                        let is_new = registrations
                            .insert(registration_id.clone(), registration)
                            .is_none();
                        if is_new {
                            log_info!(
                                "[검증자 자동 등록] 확인 {}/4명",
                                registrations.len().min(4)
                            );
                        }
                        if registrations.len() >= 4 && consensus.chain.tip_height() == 0
                            && validators.len() < 4
                        {
                            let selected: Vec<_> = registrations
                                .keys()
                                .take(4)
                                .map(|id| Validator::new(id.clone(), 100))
                                .collect();
                            let mut current_ids: Vec<_> =
                                validators.iter().map(|validator| validator.id.clone()).collect();
                            current_ids.sort();
                            let selected_ids: Vec<_> =
                                selected.iter().map(|validator| validator.id.clone()).collect();
                            if current_ids != selected_ids {
                                consensus.replace_bootstrap_validators(selected.clone())?;
                                save_validators(&args.validators_config, &selected)?;
                                validators = selected;
                                local_is_validator = validators
                                    .iter()
                                    .any(|validator| validator.id == local_validator_address);
                                println!();
                                log_info!(
                                    "[BFT 합의 시작] 검증자 4명 자동 등록 완료. 공통 검증자 집합으로 전환했습니다."
                                );
                            }
                        } else if !validators
                            .iter()
                            .any(|validator| validator.id == registration_id)
                        {
                            log_info!(
                                "[검증자 후보 대기] {} · P2P 접속과 키 소유권 확인 완료. \
                                 현재 검증자 승인 및 다음 epoch 적용 전까지 합의권을 부여하지 않습니다.",
                                registration_id
                            );
                        }
                    }
                    Some(NetworkEvent::TransactionReceived { transaction, .. }) => {
                        rpc.restore_transactions(vec![transaction])?;
                    }
                    Some(NetworkEvent::CommunicationReceived { envelope, .. }) => {
                        rpc.receive_communication(envelope, unix_timestamp())?;
                    }
                    Some(NetworkEvent::ProposalReceived { proposal, .. }) if !is_client && local_is_validator => {
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
                    Some(NetworkEvent::ConsensusReceived { message, .. }) if !is_client && local_is_validator => {
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
                    Some(NetworkEvent::OutgoingConnectionFailed { peer_id, error, .. }) => {
                        let peer = peer_id
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "확인 불가".into());
                        let guidance = if error.contains("Unexpected peer ID") {
                            " · 주소의 실제 PeerId가 설정과 다릅니다. bootstrap.json과 운영 server.node.key를 확인하세요."
                        } else {
                            ""
                        };
                        ieum_chain::logger::write_repeated_error(
                            &format!("[P2P 접속 실패] PeerId: {peer} · 오류: {error}{guidance}"),
                        );
                    }
                    Some(event) => log_info!("{event}"),
                    None => {
                        rpc_task.abort();
                        return Err("P2P 네트워크 이벤트 채널이 종료되었습니다.".into());
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!();
                log_info!("노드를 안전하게 종료합니다.");
                rpc_task.abort();
                break;
            }
        }
    }
    Ok(())
}

fn run_validator_key_command(command: ValidatorKeyCommand) -> Result<(), String> {
    match command {
        ValidatorKeyCommand::Generate { output } => {
            let public_key = ieum_chain::validator_key::generate_key_file(&output)?;
            println!("검증자 개인키 생성 완료: {}", output.display());
            println!("공개키: {public_key}");
            println!("주의: 개인키 파일을 Git에 커밋하거나 다른 서버와 공유하지 마세요.");
        }
        ValidatorKeyCommand::Public { key } => {
            println!("{}", ieum_chain::validator_key::public_key_from_file(&key)?);
        }
        ValidatorKeyCommand::CreateConfig {
            public_keys,
            voting_power,
            output,
        } => {
            ieum_chain::validator_key::create_validators_config(
                &output,
                &public_keys,
                voting_power,
            )?;
            println!("운영 검증자 설정 생성 완료: {}", output.display());
            println!("검증자 수: {}", public_keys.len());
        }
    }
    Ok(())
}

fn run_node_command(command: NodeCommand) -> Result<(), String> {
    let validator_key = Path::new("config/validator.key");
    let node_key = Path::new("data/server.node.key");
    let ledger_dir = Path::new("data/ledger");
    match command {
        NodeCommand::Init { new: true } => {
            let backup = installation::initialize_new_server_node(
                validator_key,
                node_key,
                ledger_dir,
                Path::new("config/validators.json"),
                Path::new("config/events.json"),
                Path::new("config/upgrades.json"),
            )?;
            if let Some(path) = backup {
                println!("[기존 노드 자동 백업] {}", path.display());
            }
            println!("신규 노드 초기화가 완료되었습니다. 다음 명령으로 실행하세요:");
            println!("  ieum-chain server");
            Ok(())
        }
        NodeCommand::Init { new: false } => unreachable!("--new는 필수 옵션입니다."),
        NodeCommand::Verify => {
            let (validator_public_key, peer_id) =
                installation::verify_server_node(validator_key, node_key, ledger_dir)?;
            println!("[노드 검증 완료] validator 공개키: {validator_public_key}");
            println!("[노드 검증 완료] PeerId: {peer_id}");
            println!("원장과 서버 키가 최초 초기화 기록과 일치합니다.");
            Ok(())
        }
    }
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

#[derive(Debug, Deserialize, Serialize)]
struct ValidatorConfig {
    chain_id: String,
    validators: Vec<Validator>,
}

fn verify_validator_registration(registration: &ValidatorRegistration) -> Result<(), String> {
    registration
        .peer_id
        .parse::<libp2p::PeerId>()
        .map_err(|_| "등록 PeerId 형식이 올바르지 않습니다.".to_string())?;
    ieum_chain::wallet::verify_signature(
        &registration.validator_id,
        &ValidatorRegistration::bytes_to_sign(&registration.validator_id, &registration.peer_id),
        &registration.signature_hex,
    )
    .map_err(|error| format!("검증자 소유권 서명 오류: {error}"))
}

fn save_validators(path: &Path, validators: &[Validator]) -> Result<(), String> {
    let config = ValidatorConfig {
        chain_id: "21004".into(),
        validators: validators.to_vec(),
    };
    let mut contents = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("검증자 설정 직렬화 실패: {error}"))?;
    contents.push('\n');
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, contents)
        .map_err(|error| format!("검증자 임시 설정 저장 실패: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("검증자 설정 교체 실패({}): {error}", path.display()))
}

fn multiaddr_peer_id(address: &Multiaddr) -> Option<libp2p::PeerId> {
    address.iter().find_map(|protocol| match protocol {
        Protocol::P2p(peer_id) => Some(peer_id),
        _ => None,
    })
}

fn load_validators(path: &Path) -> Result<Vec<Validator>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("검증자 설정 읽기 실패({}): {error}", path.display()))?;
    let config: ValidatorConfig = serde_json::from_str(&text)
        .map_err(|error| format!("검증자 설정 형식 오류({}): {error}", path.display()))?;
    if config.chain_id != "21004" {
        return Err("검증자 설정 chain_id는 21004여야 합니다.".into());
    }
    if config.validators.is_empty() {
        return Err("검증자는 최소 1개가 필요합니다.".into());
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
        let json = format!(r#"["{DEFAULT_BOOTSTRAP_PEER}"]"#);
        let config: BootstrapConfig = serde_json::from_str(&json).unwrap();
        match config {
            BootstrapConfig::Addresses(peers) => {
                assert_eq!(peers.len(), 1);
                assert_eq!(peers[0], DEFAULT_BOOTSTRAP_PEER);
            }
            BootstrapConfig::Object { .. } => panic!("배열 형식이어야 합니다."),
        }
    }

    #[test]
    fn validator_registration_requires_key_ownership_signature() {
        let signer: ValidatorSigner = Wallet::from_seed([91; 32]).into();
        let validator_id = signer.address();
        let peer_id = libp2p::PeerId::random().to_string();
        let registration = ValidatorRegistration {
            validator_id: validator_id.clone(),
            peer_id: peer_id.clone(),
            signature_hex: signer
                .sign_bytes(&ValidatorRegistration::bytes_to_sign(
                    &validator_id,
                    &peer_id,
                ))
                .unwrap(),
        };
        assert!(verify_validator_registration(&registration).is_ok());

        let mut forged = registration;
        forged.peer_id = libp2p::PeerId::random().to_string();
        assert!(verify_validator_registration(&forged).is_err());
    }

    #[test]
    fn bootstrap_node_does_not_dial_itself() {
        let address: Multiaddr = DEFAULT_BOOTSTRAP_PEER.parse().unwrap();
        let peer_id = multiaddr_peer_id(&address).unwrap();
        assert_eq!(
            peer_id.to_string(),
            DEFAULT_BOOTSTRAP_PEER.rsplit('/').next().unwrap()
        );
    }
}
