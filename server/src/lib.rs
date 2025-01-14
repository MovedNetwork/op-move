use {
    crate::mirror::MirrorLog,
    clap::Parser,
    flate2::read::GzDecoder,
    jsonwebtoken::{DecodingKey, Validation},
    moved::{
        block::{
            Block, BlockHash, BlockMemory, BlockRepository, Eip1559GasFee, ExtendedBlock, Header,
            InMemoryBlockQueries, InMemoryBlockRepository, MovedBlockHash,
        },
        genesis::{self, config::GenesisConfig},
        move_execution::{CreateEcotoneL1GasFee, CreateMovedL2GasFee, MovedBaseTokenAccounts},
        primitives::U256,
        state_actor::{InMemoryStateQueries, StateActor, StatePayloadId},
        storage::InMemoryState,
        types::state::{Command, StateMessage},
    },
    once_cell::sync::Lazy,
    std::{
        fs,
        io::Read,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        path::Path,
        time::SystemTime,
    },
    tokio::sync::mpsc,
    warp::{
        hyper::{body::Bytes, Body, Response},
        path::FullPath,
        Filter, Rejection,
    },
    warp_reverse_proxy::{
        extract_request_data_filter, proxy_to_and_forward_response, Headers, Method,
        QueryParameters,
    },
};

mod geth_genesis;
mod mirror;

#[cfg(test)]
mod tests;

pub type ProductionStateActor = StateActor<
    InMemoryState,
    StatePayloadId,
    MovedBlockHash,
    InMemoryBlockRepository,
    Eip1559GasFee,
    CreateEcotoneL1GasFee,
    CreateMovedL2GasFee,
    MovedBaseTokenAccounts,
    InMemoryBlockQueries,
    BlockMemory,
    InMemoryStateQueries,
>;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    jwtsecret: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Claims {
    iat: u64,
}

const EIP1559_ELASTICITY_MULTIPLIER: u64 = 6;
const EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR: U256 = U256::from_limbs([250, 0, 0, 0]);
const JWT_VALID_DURATION_IN_SECS: u64 = 60;
/// JWT secret key is either passed in as an env var `JWT_SECRET` or file path arg `--jwtsecret`
static JWTSECRET: Lazy<Vec<u8>> = Lazy::new(|| {
    let mut jwt = std::env::var("JWT_SECRET").unwrap_or_default();
    if jwt.is_empty() {
        let args = Args::parse();
        jwt = fs::read_to_string(args.jwtsecret).expect("JWT file should exist");
    }
    hex::decode(jwt).expect("JWT secret should be a hex string")
});

pub async fn run() {
    // TODO: think about channel size bound
    let (state_channel, rx) = mpsc::channel(1_000);

    // TODO: genesis should come from a file (path specified by CLI)
    let genesis_config = GenesisConfig {
        chain_id: 42069,
        l2_contract_genesis: Path::new(
            "src/tests/optimism/packages/contracts-bedrock/deployments/genesis.json",
        )
        .into(),
        ..Default::default()
    };

    let state = initialize_state_actor(genesis_config, rx);

    let http_state_channel = state_channel.clone();
    let http_server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8545));
    let http_route = warp::any()
        .map(move || http_state_channel.clone())
        .and(extract_request_data_filter())
        .and_then(|state_channel, path, query, method, headers, body| {
            // TODO: Limit engine API access to only authenticated endpoint
            mirror(state_channel, path, query, method, headers, body, "9545")
        });

    let auth_state_channel = state_channel;
    let auth_server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8551));
    let auth_route = warp::any()
        .map(move || auth_state_channel.clone())
        .and(extract_request_data_filter())
        .and(validate_jwt())
        .and_then(|state_channel, path, query, method, headers, body, _| {
            mirror(state_channel, path, query, method, headers, body, "9551")
        });

    let (_, _, state_result) = tokio::join!(
        warp::serve(http_route).run(http_server_addr),
        warp::serve(auth_route).run(auth_server_addr),
        state.spawn(),
    );
    state_result.unwrap();
}

pub fn initialize_state_actor(
    genesis_config: GenesisConfig,
    rx: mpsc::Receiver<StateMessage>,
) -> ProductionStateActor {
    let block_hash = MovedBlockHash;
    let genesis_block = create_genesis_block(&block_hash, &genesis_config);

    let mut block_memory = BlockMemory::new();
    let mut repository = InMemoryBlockRepository::new();
    let head = genesis_block.hash;
    repository.add(&mut block_memory, genesis_block);

    let mut state = InMemoryState::new();
    let (genesis_changes, table_changes) = genesis::init(&genesis_config, &state);
    let state_query = InMemoryStateQueries::from_genesis(genesis_config.initial_state_root);
    genesis::apply(genesis_changes, table_changes, &genesis_config, &mut state);

    let base_token = MovedBaseTokenAccounts::new(genesis_config.treasury);
    StateActor::new(
        rx,
        state,
        head,
        0,
        genesis_config,
        StatePayloadId,
        block_hash,
        repository,
        Eip1559GasFee::new(
            EIP1559_ELASTICITY_MULTIPLIER,
            EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
        ),
        CreateEcotoneL1GasFee,
        CreateMovedL2GasFee,
        base_token,
        InMemoryBlockQueries,
        block_memory,
        state_query,
        moved::state_actor::StateActor::on_tx_in_memory(),
        moved::state_actor::StateActor::on_tx_batch_in_memory(),
    )
}

fn create_genesis_block(
    block_hash: &impl BlockHash,
    genesis_config: &GenesisConfig,
) -> ExtendedBlock {
    let genesis_header = Header {
        state_root: genesis_config.initial_state_root,
        ..Default::default()
    };
    let hash = block_hash.block_hash(&genesis_header);
    let genesis_block = Block::new(genesis_header, Vec::new());

    genesis_block.with_hash(hash).with_value(U256::ZERO)
}

pub fn validate_jwt() -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    warp::header::<String>("authorization").and_then(|token: String| async move {
        // Token is embedded as a string in the form of `Bearer the.actual.token`
        let token = token.trim_start_matches("Bearer ").to_string();
        let mut validation = Validation::default();
        // OP node only sends `issued at` claims in the JWT token
        validation.set_required_spec_claims(&["iat"]);
        let decoded = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(&JWTSECRET),
            &validation,
        );
        let iat = decoded.map_err(|_| warp::reject::reject())?.claims.iat;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Current system time should be available")
            .as_secs();
        if now > iat + JWT_VALID_DURATION_IN_SECS {
            return Err(warp::reject::reject());
        }
        Ok(token)
    })
}

async fn mirror(
    state_channel: mpsc::Sender<StateMessage>,
    path: FullPath,
    query: QueryParameters,
    method: Method,
    headers: Headers,
    body: Bytes,
    port: &str,
) -> std::result::Result<warp::reply::Response, Rejection> {
    use std::result::Result;

    let is_zipped = headers
        .get("accept-encoding")
        .map(|x| x.to_str().unwrap().contains("gzip"))
        .unwrap_or(false);
    let request: Result<serde_json::Value, _> = serde_json::from_slice(&body);
    let (geth_response_parts, geth_response_bytes, parsed_geth_response) =
        match proxy(path, query, method, headers.clone(), body, port).await {
            Ok(response) => {
                let (parts, body) = response.into_parts();
                let raw_bytes = hyper::body::to_bytes(body)
                    .await
                    .expect("Failed to get geth response");
                let bytes = if is_zipped {
                    match try_decompress(&raw_bytes) {
                        Ok(x) => x,
                        Err(e) => {
                            println!("WARN: gz decompression failed: {e:?}");
                            let body = hyper::Body::from(raw_bytes);
                            return Ok(warp::reply::Response::from_parts(parts, body));
                        }
                    }
                } else {
                    raw_bytes.to_vec()
                };
                match serde_json::from_slice::<serde_json::Value>(&bytes) {
                    Ok(parsed_response) => (parts, raw_bytes, parsed_response),
                    Err(_) => {
                        println!(
                            "Request: {}",
                            serde_json::to_string_pretty(&request.unwrap()).unwrap()
                        );
                        println!("headers: {headers:?}");
                        println!("WARN: op-geth non-json response: {:?}", bytes);
                        let body = hyper::Body::from(bytes);
                        return Ok(warp::reply::Response::from_parts(parts, body));
                    }
                }
            }
            Err(e) => return Err(e),
        };

    let request = request.expect("geth responded, so body must have been JSON");
    let op_move_response =
        moved_engine_api::request::handle(request.clone(), state_channel.clone()).await;
    let log = MirrorLog {
        request: &request,
        geth_response: &parsed_geth_response,
        op_move_response: &op_move_response,
        port,
    };
    println!("{}", serde_json::to_string_pretty(&log).unwrap());

    // TODO: this is a hack because we currently can't compute the genesis
    // hash expected by op-node.
    if geth_genesis::is_genesis_block_request(&request).unwrap_or(false) {
        let block = geth_genesis::extract_genesis_block(&parsed_geth_response)
            .expect("Must get genesis from geth");
        state_channel
            .send(Command::GenesisUpdate { block }.into())
            .await
            .ok();
        let body = hyper::Body::from(geth_response_bytes);
        return Ok(warp::reply::Response::from_parts(geth_response_parts, body));
    }

    let body = hyper::Body::from(serde_json::to_vec(&op_move_response).unwrap());
    Ok(warp::reply::Response::new(body))
}

async fn proxy(
    path: FullPath,
    query: QueryParameters,
    method: Method,
    headers: Headers,
    body: Bytes,
    port: &str,
) -> std::result::Result<Response<Body>, Rejection> {
    proxy_to_and_forward_response(
        format!("http://0.0.0.0:{}", port),
        "".to_string(),
        path,
        query,
        method,
        headers,
        body,
    )
    .await
}

fn try_decompress(raw_bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let gz = GzDecoder::new(raw_bytes);
    gz.bytes().collect()
}
