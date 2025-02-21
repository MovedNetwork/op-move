use {
    crate::mirror::MirrorLog,
    clap::Parser,
    flate2::read::GzDecoder,
    jsonwebtoken::{DecodingKey, Validation},
    moved::{
        block::{
            BaseGasFee, Block, BlockHash, BlockQueries, BlockRepository, Eip1559GasFee,
            ExtendedBlock, Header,
        },
        move_execution::{
            BaseTokenAccounts, CreateEcotoneL1GasFee, CreateL1GasFee, CreateL2GasFee,
            CreateMovedL2GasFee,
        },
        payload::{NewPayloadId, PayloadQueries, StatePayloadId},
        receipt::{ReceiptQueries, ReceiptRepository},
        state_actor::{StateActor, StateQueries},
        transaction::{TransactionQueries, TransactionRepository},
        types::state::{Command, StateMessage},
    },
    moved_api::method_name::MethodName,
    moved_genesis::config::GenesisConfig,
    moved_shared::primitives::U256,
    moved_state::State,
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
        Filter, Rejection, Reply,
    },
    warp_reverse_proxy::{
        extract_request_data_filter, proxy_to_and_forward_response, Headers, Method,
        QueryParameters, Request,
    },
};

mod dependency;
mod geth_genesis;
mod mirror;

#[cfg(test)]
mod tests;

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
            mirror(
                state_channel,
                (path, query, method, headers, body),
                "9545",
                // Limit engine API access to only authenticated endpoint
                MethodName::is_non_engine_api,
            )
        })
        .with(warp::cors().allow_any_origin());

    let auth_state_channel = state_channel;
    let auth_server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8551));
    let auth_route = warp::any()
        .map(move || auth_state_channel.clone())
        .and(extract_request_data_filter())
        .and(validate_jwt())
        .and_then(|state_channel, path, query, method, headers, body, _| {
            mirror(
                state_channel,
                (path, query, method, headers, body),
                "9551",
                |_| true,
            )
        })
        .with(warp::cors().allow_any_origin());

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
) -> StateActor<
    impl State + Send + Sync + 'static,
    impl NewPayloadId + Send + Sync + 'static,
    impl BlockHash + Send + Sync + 'static,
    impl BlockRepository<Storage = dependency::SharedStorage> + Send + Sync + 'static,
    impl BaseGasFee + Send + Sync + 'static,
    impl CreateL1GasFee + Send + Sync + 'static,
    impl CreateL2GasFee + Send + Sync + 'static,
    impl BaseTokenAccounts + Send + Sync + 'static,
    impl BlockQueries<Storage = dependency::SharedStorage> + Send + Sync + 'static,
    dependency::SharedStorage,
    impl StateQueries + Send + Sync + 'static,
    impl TransactionRepository<Storage = dependency::SharedStorage> + Send + Sync + 'static,
    impl TransactionQueries<Storage = dependency::SharedStorage> + Send + Sync + 'static,
    dependency::ReceiptStorage,
    impl ReceiptRepository<Storage = dependency::ReceiptStorage> + Send + Sync + 'static,
    impl ReceiptQueries<Storage = dependency::ReceiptStorage> + Send + Sync + 'static,
    impl PayloadQueries<Storage = dependency::SharedStorage> + Send + Sync + 'static,
> {
    let block_hash = dependency::block_hash();
    let base_token = dependency::base_token(&genesis_config);
    let mut state = dependency::state();
    let state_query = dependency::state_query(&genesis_config);
    let mut storage = dependency::memory();
    let mut block_repository = dependency::block_repository();
    let block_queries = dependency::block_queries();
    let transaction_repository = dependency::transaction_repository();
    let transaction_queries = dependency::transaction_queries();
    let receipt_storage = dependency::receipt_memory();
    let receipt_repository = dependency::receipt_repository();
    let receipt_queries = dependency::receipt_queries();
    let payload_queries = dependency::payload_queries();

    let (genesis_changes, table_changes) = {
        #[cfg(test)]
        {
            moved_genesis_image::load()
        }
        #[cfg(not(test))]
        {
            moved_genesis::build(&moved_genesis::MovedVm, &genesis_config, &state)
        }
    };
    moved_genesis::apply(genesis_changes, table_changes, &genesis_config, &mut state);

    let genesis_block = create_genesis_block(&block_hash, &genesis_config);
    let head = genesis_block.hash;
    block_repository
        .add(&mut storage, genesis_block)
        .expect("Database should be ready");

    StateActor::new(
        rx,
        state,
        head,
        0,
        genesis_config,
        StatePayloadId,
        block_hash,
        block_repository,
        Eip1559GasFee::new(
            EIP1559_ELASTICITY_MULTIPLIER,
            EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
        ),
        CreateEcotoneL1GasFee,
        CreateMovedL2GasFee,
        base_token,
        block_queries,
        storage,
        state_query,
        transaction_repository,
        transaction_queries,
        receipt_storage,
        receipt_repository,
        receipt_queries,
        payload_queries,
        dependency::on_tx(),
        dependency::on_tx_batch(),
        dependency::on_payload(),
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
    request: Request,
    port: &str,
    is_allowed: impl Fn(&MethodName) -> bool,
) -> Result<warp::reply::Response, Rejection> {
    let (path, query, method, headers, body) = request;

    // Handle load balancer health check with a simple response
    if method == Method::GET {
        return Ok("healthy".into_response());
    }

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
        moved_api::request::handle(request.clone(), state_channel.clone(), is_allowed).await;
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
) -> Result<Response<Body>, Rejection> {
    let addr = std::env::var("OP_GETH_ADDR").unwrap_or("0.0.0.0".to_owned());
    proxy_to_and_forward_response(
        format!("http://{addr}:{port}"),
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
