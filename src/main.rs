use clap::{Args as ClapArgs, Parser, Subcommand};
use ieum_chain::{
    NetworkConfig, P2pNode, RpcConfig, RpcServer, node_key::load_or_create_node_key,
};
use libp2p::Multiaddr;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "ieum-chain", version, about = "가벼운 IEUM 테스트넷 노드")]
struct Args {
    #[command(subcommand)]
    command: Command,
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

    /// 운영 서버 부트스트랩 주소. 여러 서버를 지정하려면 옵션을 반복합니다.
    #[arg(
        long,
        required = true,
        help = "/dns4/node.ieum.aah.name/udp/7001/quic-v1/p2p/PeerId"
    )]
    peer: Vec<Multiaddr>,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();
    let (mode, args, bootstrap_peers) = match args.command {
        Command::Server(args) => ("서버", args, Vec::new()),
        Command::Client(client) => ("일반 PC", client.node, client.peer),
    };
    let identity_key = load_or_create_node_key(&args.node_key)?;
    let config = NetworkConfig {
        listen_port: args.port,
        bootstrap_peers,
        identity_key: Some(identity_key),
        max_message_bytes: args.max_message_bytes,
        idle_timeout: Duration::from_secs(30),
        ban_duration: Duration::from_secs(10 * 60),
    };
    let (peer_id, _commands, mut events) = P2pNode::new(config).run().await?;

    let rpc_config = RpcConfig {
        listen_ip: args.rpc_host,
        port: args.rpc_port,
        data_dir: args.rpc_data_dir,
        ..RpcConfig::default()
    };
    let mut rpc_task = tokio::spawn(RpcServer::new(rpc_config).run());

    println!("IEUM {mode} 노드 시작: {peer_id}");
    println!("영구 노드 키: {}", args.node_key.display());
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
                    Some(event) => println!("네트워크 이벤트: {event:?}"),
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
