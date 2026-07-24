use aah_chain::{NetworkConfig, P2pNode, RpcConfig, RpcServer};
use clap::Parser;
use libp2p::Multiaddr;
use std::time::Duration;
use std::{net::IpAddr, str::FromStr};

#[derive(Debug, Parser)]
#[command(name = "aah-chain", version, about = "가벼운 AAH 테스트넷 노드")]
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

    /// geth 호환 HTTP JSON-RPC 포트
    #[arg(long, default_value_t = 8545)]
    rpc_port: u16,

    /// JSON-RPC 리스닝 IP. 기본값은 안전한 localhost입니다.
    #[arg(long, default_value = "127.0.0.1")]
    rpc_addr: String,

    /// eth_chainId와 net_version에서 반환할 체인 ID
    #[arg(long, default_value_t = 31337)]
    chain_id: u64,
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
    let rpc_ip =
        IpAddr::from_str(&args.rpc_addr).map_err(|error| format!("RPC 리스닝 IP 오류: {error}"))?;
    let rpc = RpcServer::new(RpcConfig {
        listen_ip: rpc_ip,
        port: args.rpc_port,
        chain_id: args.chain_id,
    });
    tokio::spawn(async move {
        if let Err(error) = rpc.run().await {
            eprintln!("{error}");
        }
    });
    println!("AAH 노드 시작: {peer_id}");
    println!("같은 LAN의 노드는 mDNS로 자동 검색합니다. 종료: Ctrl+C");

    loop {
        tokio::select! {
            event = events.recv() => {
                match event {
                    Some(event) => println!("네트워크 이벤트: {event:?}"),
                    None => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("노드를 안전하게 종료합니다.");
                break;
            }
        }
    }
    Ok(())
}
