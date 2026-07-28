use clap::{Args as ClapArgs, Parser, Subcommand};
use ieum_chain::{
    NetworkConfig, P2pNode, RpcConfig, RpcServer, node_key::load_or_create_node_key,
};
use libp2p::Multiaddr;
use serde::Deserialize;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, TcpListener, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_BOOTSTRAP_CONFIG: &str = "config/bootstrap.json";
const DEFAULT_BOOTSTRAP_PEER: &str = "/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/12D3KooWAVRZjnbP8nXp8vD6irYFAXdLJVyczEFWdKLFzKnKDATx";
const SERVER_INSTANCE_PORT: u16 = 49_889;
const CLIENT_INSTANCE_PORT: u16 = 49_890;

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
    #[arg(long, default_value_t = 524_288)]
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
}

#[derive(Debug, ClapArgs)]
struct ClientArgs {
    #[command(flatten)]
    node: NodeArgs,

    /// 추가 운영 서버 주소. 지정하지 않으면 config/bootstrap.json을 자동으로 읽습니다.
    #[arg(long, help = "/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/PeerId")]
    peer: Vec<Multiaddr>,

    /// 자동으로 읽을 부트스트랩 설정 파일
    #[arg(long, default_value = DEFAULT_BOOTSTRAP_CONFIG)]
    bootstrap_config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();
    let (mode, mut args, bootstrap_peers, is_client) = match args.command {
        Some(Command::Server(mut args)) => {
            if args.node_key == PathBuf::from("data/node.key") {
                args.node_key = PathBuf::from("data/server.node.key");
            }
            ("서버", args, Vec::new(), false)
        }
        Some(Command::Client(mut client)) => {
            if client.node.node_key == PathBuf::from("data/node.key") {
                client.node.node_key = PathBuf::from("data/client.node.key");
            }
            if client.node.rpc_data_dir == PathBuf::from("data/ledger") {
                client.node.rpc_data_dir = PathBuf::from("data/client-ledger");
            }
            let peers = load_bootstrap_peers(&client.bootstrap_config, client.peer)?;
            ("일반 PC", client.node, peers, true)
        }
        None => {
            let node = NodeArgs {
                port: 7001,
                max_message_bytes: 524_288,
                rpc_port: 8989,
                rpc_host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                rpc_data_dir: PathBuf::from("data/client-ledger"),
                node_key: PathBuf::from("data/client.node.key"),
            };
            let peers =
                load_bootstrap_peers(Path::new(DEFAULT_BOOTSTRAP_CONFIG), Vec::new())?;
            ("일반 PC", node, peers, true)
        }
    };
    let _instance_guard = acquire_instance_guard(is_client)?;
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
    let (peer_id, _commands, mut events) = P2pNode::new(config).run().await?;

    let rpc_config = RpcConfig {
        listen_ip: args.rpc_host,
        port: args.rpc_port,
        data_dir: args.rpc_data_dir.clone(),
        ..RpcConfig::default()
    };
    let mut rpc_task = tokio::spawn(RpcServer::new(rpc_config).run());

    println!("IEUM {mode} 노드 시작: {peer_id}");
    println!("영구 노드 키: {}", args.node_key.display());
    println!("P2P 포트: {}/UDP", args.port);
    println!("RPC 주소: {}:{}", args.rpc_host, args.rpc_port);
    println!("원장 경로: {}", args.rpc_data_dir.display());
    if is_client {
        println!("운영 서버 자동 연결 대상:");
        for peer in &startup_peers {
            println!("  - {peer}");
        }
    }
    println!("같은 LAN의 노드는 mDNS로 자동 검색합니다. 종료: Ctrl+C");

    loop {
        tokio::select! {
            result = &mut rpc_task => {
                return match result {
                    Ok(Ok(())) => Err("JSON-RPC 서버가 예기치 않게 종료되었습니다.".into()),
                    Ok(Err(message)) => Err(message),
                    Err(error) => Err(format!("JSON-RPC 작업 실행 실패: {error}")),
                };
            }
            event = events.recv() => {
                match event {
                    Some(event) => println!("{event}"),
                    None => {
                        rpc_task.abort();
                        return Err("P2P 네트워크 이벤트 채널이 종료되었습니다.".into());
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("노드를 안전하게 종료합니다.");
                rpc_task.abort();
                break;
            }
        }
    }
    Ok(())
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
            println!(
                "UDP {original_p2p} 포트가 사용 중이므로 클라이언트 P2P 포트를 {}로 자동 변경합니다.",
                args.port
            );
        }

        let original_rpc = args.rpc_port;
        args.rpc_port = first_available_tcp_port(args.rpc_host, args.rpc_port, 100)?;
        if args.rpc_port != original_rpc {
            println!(
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
        let configured: BootstrapConfig = serde_json::from_str(&text).map_err(|error| {
            format!("부트스트랩 설정 형식 오류({}): {error}", path.display())
        })?;
        let addresses = match configured {
            BootstrapConfig::Addresses(peers) | BootstrapConfig::Object { peers } => peers,
        };
        addresses
            .into_iter()
            .map(|address| {
                address.parse().map_err(|error| {
                    format!("부트스트랩 주소 형식 오류({address}): {error}")
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    } else if path == Path::new(DEFAULT_BOOTSTRAP_CONFIG) {
        vec![DEFAULT_BOOTSTRAP_PEER
            .parse()
            .map_err(|error| format!("내장 부트스트랩 주소 오류: {error}"))?]
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
