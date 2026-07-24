use aah_chain::{NetworkConfig, P2pNode};
use clap::Parser;
use libp2p::Multiaddr;
use std::time::Duration;

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
