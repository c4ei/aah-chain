use crate::{Blockchain, Mempool, Wallet};
use axum::{Json, Router, routing::post};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

/// geth/web3 도구가 접속할 HTTP JSON-RPC 설정입니다.
#[derive(Clone, Debug)]
pub struct RpcConfig {
    pub listen_ip: IpAddr,
    pub port: u16,
    pub chain_id: u64,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            // 개발 기본값은 외부에 노출되지 않는 localhost입니다.
            listen_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8545,
            chain_id: 31337,
        }
    }
}

#[derive(Debug)]
struct RpcState {
    chain: Blockchain,
    pool: Mempool,
    /// RPC의 0x 주소를 실제 Ed25519 지갑에 연결합니다.
    wallets: HashMap<String, Wallet>,
    faucet_alias: String,
    producer: Wallet,
    chain_id: u64,
}

/// 기존 geth 스크립트에서 자주 쓰는 계정·잔액·송금 API를 제공하는 호환 계층입니다.
///
/// 현재 원장의 Ed25519 서명 방식은 유지하며, RPC 화면에만 Ethereum 모양의
/// 20바이트 `0x` 별칭을 표시합니다. 따라서 raw Ethereum transaction이나
/// Solidity EVM을 구현한 것은 아닙니다.
pub struct RpcServer {
    config: RpcConfig,
    state: Arc<RwLock<RpcState>>,
}

impl RpcServer {
    pub fn new(config: RpcConfig) -> Self {
        // 첫 번째 계정은 개발용 faucet입니다. 실제 운영망에서는 genesis/config와
        // 암호화된 keystore로 교체해야 합니다.
        let faucet = Wallet::from_seed([42; 32]);
        let producer = Wallet::from_seed([43; 32]);
        let faucet_alias = rpc_alias(&faucet.address());
        let chain = Blockchain::new(vec![(faucet.address(), 1_000_000_000_000_000_000)]);
        let mut wallets = HashMap::new();
        wallets.insert(faucet_alias.clone(), faucet);
        Self {
            state: Arc::new(RwLock::new(RpcState {
                chain,
                pool: Mempool::default(),
                wallets,
                faucet_alias,
                producer,
                chain_id: config.chain_id,
            })),
            config,
        }
    }

    pub async fn run(self) -> Result<(), String> {
        let address = SocketAddr::new(self.config.listen_ip, self.config.port);
        let app = Router::new()
            .route("/", post(handle_rpc))
            .with_state(self.state);
        let listener = tokio::net::TcpListener::bind(address)
            .await
            .map_err(|error| format!("JSON-RPC 포트 열기 실패: {error}"))?;
        println!("geth 호환 JSON-RPC 대기: http://{address}");
        axum::serve(listener, app)
            .await
            .map_err(|error| format!("JSON-RPC 서버 오류: {error}"))
    }
}

async fn handle_rpc(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<RpcState>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = request
        .get("params")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let result = dispatch(&state, method, &params);
    Json(match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
        Err((code, message)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        }
    })
}

fn dispatch(
    state: &Arc<RwLock<RpcState>>,
    method: &str,
    params: &[Value],
) -> Result<Value, (i64, String)> {
    match method {
        "web3_clientVersion" => Ok(json!("AAH-Chain/v0.0.3/rust")),
        "net_version" => {
            let state = read_state(state)?;
            Ok(json!(state.chain_id.to_string()))
        }
        "net_listening" => Ok(json!(true)),
        "net_peerCount" => Ok(json!("0x0")),
        "rpc_modules" => Ok(json!({
            "eth": "1.0",
            "net": "1.0",
            "personal": "1.0",
            "web3": "1.0"
        })),
        "eth_chainId" => {
            let state = read_state(state)?;
            Ok(json!(quantity(state.chain_id)))
        }
        "eth_syncing" => Ok(json!(false)),
        "eth_blockNumber" => {
            let state = read_state(state)?;
            let height = state.chain.blocks.last().map(|block| block.height).unwrap_or(0);
            Ok(json!(quantity(height)))
        }
        "eth_accounts" | "personal_listAccounts" => {
            let state = read_state(state)?;
            let mut accounts: Vec<_> = state.wallets.keys().cloned().collect();
            accounts.sort();
            Ok(json!(accounts))
        }
        "eth_coinbase" => {
            let state = read_state(state)?;
            Ok(json!(state.faucet_alias))
        }
        "personal_newAccount" => {
            let mut state = write_state(state)?;
            let wallet = Wallet::new();
            let alias = rpc_alias(&wallet.address());
            state.wallets.insert(alias.clone(), wallet);
            Ok(json!(alias))
        }
        "personal_unlockAccount" => {
            let address = string_param(params, 0)?;
            let state = read_state(state)?;
            Ok(json!(state.wallets.contains_key(&normalize_address(address))))
        }
        "eth_getBalance" => {
            let address = string_param(params, 0)?;
            let state = read_state(state)?;
            let ledger_address = resolve_ledger_address(&state, address);
            Ok(json!(quantity(state.chain.balance_of(&ledger_address))))
        }
        "eth_getTransactionCount" => {
            let address = string_param(params, 0)?;
            let state = read_state(state)?;
            let ledger_address = resolve_ledger_address(&state, address);
            Ok(json!(quantity(state.chain.next_nonce(&ledger_address))))
        }
        "eth_gasPrice" => Ok(json!("0x1")),
        "eth_estimateGas" => Ok(json!("0x5208")),
        "eth_getCode" => Ok(json!("0x")),
        "eth_sendTransaction" | "personal_sendTransaction" => send_transaction(state, params),
        "eth_sendRawTransaction" => Err((
            -32004,
            "v0.0.3은 Ethereum RLP/secp256k1 raw transaction을 아직 지원하지 않습니다."
                .into(),
        )),
        _ => Err((-32601, format!("지원하지 않는 JSON-RPC 메서드: {method}"))),
    }
}

fn send_transaction(
    shared: &Arc<RwLock<RpcState>>,
    params: &[Value],
) -> Result<Value, (i64, String)> {
    let request = params
        .first()
        .and_then(Value::as_object)
        .ok_or_else(|| (-32602, "거래 객체가 필요합니다.".into()))?;
    let from = request
        .get("from")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "from 주소가 필요합니다.".into()))?;
    let to = request
        .get("to")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "to 주소가 필요합니다.".into()))?;
    let amount = parse_quantity(
        request
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("0x0"),
    )?;
    let fee = parse_quantity(
        request
            .get("gasPrice")
            .and_then(Value::as_str)
            .unwrap_or("0x1"),
    )?;

    let mut state = write_state(shared)?;
    let from_alias = normalize_address(from);
    let wallet = state
        .wallets
        .get(&from_alias)
        .cloned()
        .ok_or_else(|| (-32000, "노드가 관리하지 않는 from 계정입니다.".into()))?;
    let to_ledger = resolve_ledger_address(&state, to);
    let nonce = match request.get("nonce").and_then(Value::as_str) {
        Some(value) => parse_quantity(value)?,
        None => state.chain.next_nonce(&wallet.address()),
    };
    let transaction = wallet.sign_transfer(to_ledger, amount, fee, nonce);
    let transaction_id = format!("0x{}", transaction.id());
    state
        .pool
        .add(transaction)
        .map_err(|message| (-32000, message))?;

    // v0.0.3 개발 노드는 거래가 들어오면 즉시 작은 블록을 만듭니다.
    // 이후 BFT 실행 루프가 결합되면 여기서는 mempool 제출만 수행해야 합니다.
    let transactions = state.pool.drain(1_000);
    let producer = state.producer.address();
    state
        .chain
        .add_block(transactions, producer)
        .map_err(|message| (-32000, message))?;
    Ok(json!(transaction_id))
}

fn resolve_ledger_address(state: &RpcState, address: &str) -> String {
    let normalized = normalize_address(address);
    state
        .wallets
        .get(&normalized)
        .map(Wallet::address)
        .unwrap_or(normalized)
}

fn rpc_alias(public_key_address: &str) -> String {
    let digest = Sha256::digest(public_key_address.as_bytes());
    format!("0x{}", hex::encode(&digest[digest.len() - 20..]))
}

fn normalize_address(address: &str) -> String {
    address.to_ascii_lowercase()
}

fn quantity(value: u64) -> String {
    format!("0x{value:x}")
}

fn parse_quantity(value: &str) -> Result<u64, (i64, String)> {
    let hex = value
        .strip_prefix("0x")
        .ok_or_else(|| (-32602, "수량은 0x 접두사가 있는 hex여야 합니다.".into()))?;
    u64::from_str_radix(if hex.is_empty() { "0" } else { hex }, 16)
        .map_err(|_| (-32602, "수량이 u64 범위를 벗어났거나 잘못되었습니다.".into()))
}

fn string_param(params: &[Value], index: usize) -> Result<&str, (i64, String)> {
    params
        .get(index)
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, format!("{index}번 문자열 파라미터가 필요합니다.")))
}

fn read_state(
    state: &Arc<RwLock<RpcState>>,
) -> Result<std::sync::RwLockReadGuard<'_, RpcState>, (i64, String)> {
    state
        .read()
        .map_err(|_| (-32603, "RPC 상태 읽기 잠금이 손상되었습니다.".into()))
}

fn write_state(
    state: &Arc<RwLock<RpcState>>,
) -> Result<std::sync::RwLockWriteGuard<'_, RpcState>, (i64, String)> {
    state
        .write()
        .map_err(|_| (-32603, "RPC 상태 쓰기 잠금이 손상되었습니다.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geth_style_account_balance_and_transfer_work() {
        let shared = RpcServer::new(RpcConfig::default()).state;
        let accounts = dispatch(&shared, "eth_accounts", &[])
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let faucet = accounts[0].as_str().unwrap();
        let receiver = dispatch(&shared, "personal_newAccount", &[])
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        let tx = json!({"from": faucet, "to": receiver, "value": "0x64", "gasPrice": "0x1"});
        assert!(dispatch(&shared, "eth_sendTransaction", &[tx]).is_ok());
        assert_eq!(
            dispatch(&shared, "eth_getBalance", &[json!(receiver), json!("latest")]).unwrap(),
            json!("0x64")
        );
    }

    #[test]
    fn raw_ethereum_transaction_is_explicitly_rejected() {
        let shared = RpcServer::new(RpcConfig::default()).state;
        let error = dispatch(&shared, "eth_sendRawTransaction", &[json!("0x00")]).unwrap_err();
        assert_eq!(error.0, -32004);
    }
}
