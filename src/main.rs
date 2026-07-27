use clap::Parser;
use ieum_chain::{NetworkConfig, P2pNode, RpcConfig, RpcServer};
use libp2p::Multiaddr;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "ieum-chain", version, about = "가벼운 IEUM 테스트넷 노드")]
struct Args {
    /// QUIC UDP 리스닝 포트
    #[arg(long, default_value_t = 7001)]
    port: u16,

    /// /ip4/.../udp/.../quic-v1/p2p/PeerId 형식의 부트스트랩 피어
    #[arg(long)]
    peer: Vec<Multiaddr>,

    /// 단일 P2P 메시지 최대 크기
    #[arg(long, default_value_t = 524_288)]
    max_message_bytes: usize,

    /// 지갑/geth 호환 JSON-RPC TCP 포트
    #[arg(long, default_value_t = 8545)]
    rpc_port: u16,

    /// JSON-RPC 리스닝 IP. 기본값은 외부에 노출되지 않는 localhost입니다.
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    rpc_host: IpAddr,

    /// RPC 원장·체크포인트 저장 경로
    #[arg(long, default_value = "data/ledger")]
    rpc_data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();
    let config = NetworkConfig {
        listen_port: args.port,
        bootstrap_peers: args.peer,
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

    println!("IEUM 노드 시작: {peer_id}");
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
