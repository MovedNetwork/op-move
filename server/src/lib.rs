use {
    crate::mirror::MirrorLog,
    jsonwebtoken::{DecodingKey, Validation},
    move_core_types::account_address::AccountAddress,
    std::{
        future::Future,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        path::Path,
        time::SystemTime,
    },
    tracing::level_filters::LevelFilter,
    tracing_subscriber::{fmt::format::FmtSpan, EnvFilter},
    umi_api::{
        method_name::MethodName,
        request::{RequestModifiers, SerializationKind},
    },
    umi_app::{Application, ApplicationReader, CommandQueue, Dependencies},
    umi_blockchain::{
        block::{Block, BlockHash, BlockQueries, ExtendedBlock, Header},
        payload::{NewPayloadId, StatePayloadId},
    },
    umi_genesis::config::GenesisConfig,
    umi_server_args::{
        Config, DatabaseBackend, DefaultLayer, OptionalAuthSocket, OptionalConfig,
        OptionalDatabase, OptionalGenesis, OptionalHttpSocket,
    },
    umi_shared::{
        hex,
        primitives::{
            ToSaturatedU64, B2048, B256, B64, EMPTY_LIST_ROOT, EMPTY_OMMERS_ROOT_HASH, U256,
        },
    },
    warp::{
        http::{header::CONTENT_TYPE, HeaderMap, HeaderValue, StatusCode},
        hyper::Response,
        Filter, Rejection, Reply,
    },
    warp_reverse_proxy::{extract_request_data_filter, Method, Request},
};

mod allow;
mod dependency;
mod mirror;
#[cfg(test)]
mod tests;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Claims {
    iat: u64,
}

pub fn defaults() -> DefaultLayer {
    let umi_root_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Cargo manifest has a parent");
    DefaultLayer::new(OptionalConfig {
        auth: Some(OptionalAuthSocket {
            addr: Some(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(0, 0, 0, 0),
                8551,
            ))),
            jwt_secret: None,
        }),
        http: Some(OptionalHttpSocket {
            addr: Some(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(0, 0, 0, 0),
                8545,
            ))),
        }),
        db: Some(OptionalDatabase {
            backend: Some(DatabaseBackend::InMemory),
            dir: Some(Path::new("db").into()),
            purge: Some(false),
        }),
        genesis: Some(OptionalGenesis {
            chain_id: Some(42069),
            initial_state_root: Some(B256::new(hex!(
                "4805267476cb522274ec2fe790b4dc6e889ed0d57377f90770d4a658f6b8e4ae"
            ))),
            treasury: Some(AccountAddress::ONE), // TODO: fill in the real address,
            l2_contract_genesis: Some(
                umi_root_path.join("server/src/tests/optimism/packages/contracts-bedrock/deployments/genesis.json")
                    .into(),
            ),
            token_list: Some(
                umi_root_path.join(
                    "execution/src/tests/res/bridged_tokens_test.json",
                )
                .into(),
            ),
        }),
        max_buffered_commands: Some(1_000), // TODO: think about channel size bound
    })
}

const EIP1559_ELASTICITY_MULTIPLIER: u64 = 6;
const EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR: U256 = U256::from_limbs([250, 0, 0, 0]);
const JWT_VALID_DURATION_IN_SECS: u64 = 60;

pub fn set_global_tracing_subscriber() {
    // TODO: config options for logging (debug level, output to file, etc)

    // Default to debug level logging, except for hyper and alloy because they are too verbose.
    let filter = EnvFilter::default()
        .add_directive(LevelFilter::DEBUG.into())
        .add_directive("hyper=warn".parse().expect("Is valid directive"))
        .add_directive("alloy=info".parse().expect("Is valid directive"));

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_span_events(FmtSpan::FULL)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

pub async fn run(args: Config) {
    let genesis_config = GenesisConfig::try_new(
        args.genesis.chain_id,
        args.genesis.initial_state_root,
        args.genesis.treasury,
        args.genesis.l2_contract_genesis.as_ref(),
        args.genesis.token_list.as_ref(),
    )
    .expect("Must construct genesis config to run the app");

    let deps = dependency::dependencies(args.db);
    let reader = {
        let genesis_config = genesis_config.clone();
        let deps = deps.reader();
        move || ApplicationReader::new(deps, &genesis_config)
    };
    let app = move || Application::new(deps, &genesis_config).with_genesis(&genesis_config);
    let jwt = DecodingKey::from_secret(
        hex::decode(args.auth.jwt_secret)
            .expect("JWT secret must be valid")
            .as_slice(),
    );

    umi_app::run(
        (reader, app),
        args.max_buffered_commands,
        |queue, reader| {
            tokio::spawn(async move {
                tokio::join!(
                    serve(args.http.addr, &queue, &reader, "9545", &allow::http, None),
                    serve(
                        args.auth.addr,
                        &queue,
                        &reader,
                        "9551",
                        &allow::auth,
                        Some(jwt)
                    ),
                );
            })
        },
    )
    .await
    .inspect_err(|e| {
        tracing::error!("Failed to join spawned server task: {e:?}");
    })
    .ok();
}

fn serve(
    addr: SocketAddr,
    queue: &CommandQueue,
    reader: &ApplicationReader<'static, dependency::ReaderDependency>,
    port: &'static str,
    is_allowed: &'static (impl Fn(&MethodName) -> bool + Send + Sync),
    jwt: Option<DecodingKey>,
) -> impl Future<Output = ()> {
    let services = (queue.clone(), reader.clone());
    let content_type =
        HeaderMap::from_iter([(CONTENT_TYPE, HeaderValue::from_static("application/json"))]);

    let route = warp::any()
        .map(move || services.clone())
        .and(extract_request_data_filter())
        .and(validate_jwt(jwt))
        .and_then(
            move |(queue, reader), path, query, method, headers, body, _| {
                handle_request(
                    queue,
                    (path, query, method, headers, body),
                    port,
                    is_allowed,
                    &StatePayloadId,
                    reader,
                )
            },
        )
        .with(warp::reply::with::headers(content_type))
        .with(warp::cors().allow_any_origin());

    warp::serve(route)
        .bind_with_graceful_shutdown(addr, queue.shutdown_listener())
        .1
}

/// An extension trait adds features for applying genesis state to an empty blockchain state.
pub trait GenesisStateExt: Sized {
    /// Determines if the blockchain state is empty.
    ///
    /// Empty blockchain state is defined as a tree with zero nodes, not even genesis.
    fn is_state_empty(&self) -> bool;

    /// Applies genesis blockchain state changes onto `self`.
    fn initialize_genesis_state(&mut self, genesis_config: &GenesisConfig);

    /// Applies genesis blockchain state changes onto `self`, but only if the state is empty.
    fn initialize_genesis_state_if_empty(&mut self, genesis_config: &GenesisConfig) {
        if self.is_state_empty() {
            self.initialize_genesis_state(genesis_config);
        }
    }

    /// Returns `self` that has genesis state changes applied. The implementation should not apply
    /// the genesis changes if the state is not empty.
    fn with_genesis(mut self, genesis_config: &GenesisConfig) -> Self {
        self.initialize_genesis_state_if_empty(genesis_config);
        self
    }
}

impl<'db, D: Dependencies<'db>> GenesisStateExt for Application<'db, D> {
    fn is_state_empty(&self) -> bool {
        self.block_queries
            .latest(&self.storage_reader)
            .expect("Must access block queries to run app")
            .is_none()
    }

    fn initialize_genesis_state(&mut self, genesis_config: &GenesisConfig) {
        let (genesis_changes, evm_storage_changes) = {
            #[cfg(test)]
            {
                umi_genesis_image::load()
            }
            #[cfg(not(test))]
            {
                umi_genesis::build(
                    &umi_genesis::UmiVm::new(genesis_config),
                    genesis_config,
                    &self.evm_storage,
                )
            }
        };
        umi_genesis::apply(
            genesis_changes,
            evm_storage_changes,
            genesis_config,
            &mut self.state,
            &mut self.evm_storage,
        );

        let genesis_block = create_genesis_block(&self.block_hash, genesis_config);
        self.genesis_update(genesis_block)
            .expect("Must add genesis block to state");
    }
}

pub fn initialize_app(
    args: umi_server_args::Database,
    genesis_config: &GenesisConfig,
) -> (
    Application<'static, dependency::Dependency>,
    ApplicationReader<'static, dependency::ReaderDependency>,
) {
    let (mut app, app_reader) = dependency::create(args, genesis_config);
    app.initialize_genesis_state_if_empty(genesis_config);
    (app, app_reader)
}

fn create_genesis_block(
    block_hash: &impl BlockHash,
    genesis_config: &GenesisConfig,
) -> ExtendedBlock {
    let genesis_header = Header {
        base_fee_per_gas: genesis_config
            .l2_contract_genesis
            .base_fee_per_gas
            .map(ToSaturatedU64::to_saturated_u64),
        blob_gas_used: genesis_config.l2_contract_genesis.blob_gas_used,
        difficulty: genesis_config.l2_contract_genesis.difficulty,
        excess_blob_gas: genesis_config.l2_contract_genesis.excess_blob_gas,
        extra_data: genesis_config.l2_contract_genesis.extra_data.clone(),
        gas_limit: genesis_config.l2_contract_genesis.gas_limit,
        gas_used: 0,
        logs_bloom: B2048::ZERO.into(),
        mix_hash: genesis_config.l2_contract_genesis.mix_hash,
        nonce: B64::from(genesis_config.l2_contract_genesis.nonce),
        number: genesis_config.l2_contract_genesis.number.unwrap_or(0),
        parent_beacon_block_root: Some(B256::ZERO),
        parent_hash: B256::ZERO,
        receipts_root: EMPTY_LIST_ROOT,
        state_root: B256::new(hex!(
            "30b67e4b5ef34eacb9e083c07fd5578982c2cb4e0ee1dc0a14d72b99a28ed80e"
        )),
        timestamp: genesis_config.l2_contract_genesis.timestamp,
        transactions_root: EMPTY_LIST_ROOT,
        withdrawals_root: Some(EMPTY_LIST_ROOT),
        beneficiary: genesis_config.l2_contract_genesis.coinbase,
        ommers_hash: EMPTY_OMMERS_ROOT_HASH,
        requests_hash: None,
    };
    let hash = block_hash.block_hash(&genesis_header);
    let genesis_block = Block::new(genesis_header, Vec::new());

    genesis_block
        .into_extended_with_hash(hash)
        .with_value(U256::ZERO)
}

pub fn validate_jwt(
    secret: Option<DecodingKey>,
) -> impl Filter<Extract = (Option<String>,), Error = Rejection> + Clone {
    let is_unprotected = secret.is_none();

    warp::header::<String>("authorization")
        .map(Some)
        .or_else(move |err| async move {
            if is_unprotected {
                Ok((None,))
            } else {
                Err(err)
            }
        })
        .and_then(move |token: Option<String>| {
            let secret = secret.clone();

            async move {
                let Some((secret, token)) = secret.zip(token) else {
                    return Ok(None);
                };
                // Token is embedded as a string in the form of `Bearer the.actual.token`
                let token = token.trim_start_matches("Bearer ").to_string();
                let mut validation = Validation::default();
                // OP node only sends `issued at` claims in the JWT token
                validation.set_required_spec_claims(&["iat"]);
                let decoded = jsonwebtoken::decode::<Claims>(&token, &secret, &validation);
                let iat = decoded.map_err(|_| warp::reject::reject())?.claims.iat;
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("Current system time should be available")
                    .as_secs();
                if now > iat + JWT_VALID_DURATION_IN_SECS {
                    return Err(warp::reject::reject());
                }
                Ok(Some(token))
            }
        })
}

async fn handle_request<'reader>(
    queue: CommandQueue,
    request: Request,
    port: &str,
    is_allowed: &impl Fn(&MethodName) -> bool,
    payload_id: &impl NewPayloadId,
    app: ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<warp::reply::Response, Rejection> {
    let (path, _, method, _, body) = request;

    // Handle load balancer health check with a simple response
    if method == Method::GET {
        return Ok(StatusCode::OK.into_response());
    }

    let Ok(request) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    };

    let serialization_tag = if path.as_str().contains("evm") {
        SerializationKind::Evm
    } else {
        SerializationKind::Bcs
    };
    let modifiers = RequestModifiers::new(is_allowed, payload_id, serialization_tag);
    let op_move_response =
        umi_api::request::handle(request.clone(), queue.clone(), modifiers, app).await;
    let log = MirrorLog {
        request: &request,
        op_move_response: &op_move_response,
        port,
    };
    serde_json::to_string(&log)
        .map(|json| tracing::info!("{json}"))
        .ok();

    let body = hyper::Body::from(
        serde_json::to_vec(&op_move_response).expect("Must be able to serialize response"),
    );
    Ok(Response::new(body))
}
