pub use error::*;

use {
    self::{
        genesis::config::GenesisConfig,
        types::{
            jsonrpc::{JsonRpcError, JsonRpcResponse},
            method_name::MethodName,
            mirror::MirrorLog,
            state::StateMessage,
        },
    },
    crate::state_actor::StatePayloadId,
    clap::Parser,
    ethers_core::types::{H256, U64},
    flate2::read::GzDecoder,
    jsonwebtoken::{DecodingKey, Validation},
    once_cell::sync::Lazy,
    std::{
        fs,
        io::Read,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
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

mod error;
mod genesis;
mod json_utils;
mod methods;
mod move_execution;
mod state_actor;
mod storage;
pub(crate) mod types;

mod primitives;
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

#[tokio::main]
async fn main() {
    // TODO: think about channel size bound
    let (state_channel, rx) = mpsc::channel(1_000);
    let genesis_config = GenesisConfig::default();
    let state = state_actor::StateActor::new_in_memory(rx, genesis_config, StatePayloadId);

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

    // If geth knows about an execution payload, we extract some data from it
    // TODO: remove reliance on geth
    let maybe_execution_payload = json_utils::get_field(
        &json_utils::get_field(&parsed_geth_response, "result"),
        "executionPayload",
    );
    let block_height: Result<Option<U64>, _> = serde_json::from_value(json_utils::get_field(
        &maybe_execution_payload,
        "blockNumber",
    ));
    let block_hash: Result<Option<H256>, _> =
        serde_json::from_value(json_utils::get_field(&maybe_execution_payload, "blockHash"));
    if let (Ok(Some(block_height)), Ok(Some(block_hash))) = (block_height, block_hash) {
        let msg = StateMessage::NewBlock {
            block_hash,
            block_height,
        };
        state_channel.send(msg).await.ok();
    }

    let request = request.expect("geth responded, so body must have been JSON");
    let op_move_response = handle_request(request.clone(), state_channel).await;
    let log = MirrorLog {
        request: &request,
        geth_response: &parsed_geth_response,
        op_move_response: &op_move_response,
        port,
    };
    println!("{}", serde_json::to_string_pretty(&log).unwrap());
    // TODO: use op_move_response
    let body = hyper::Body::from(geth_response_bytes);
    Ok(warp::reply::Response::from_parts(geth_response_parts, body))
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

async fn handle_request(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> JsonRpcResponse {
    let id = json_utils::get_field(&request, "id");
    let jsonrpc = json_utils::get_field(&request, "jsonrpc");
    let result = match inner_handle_request(request, state_channel).await {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse {
                id,
                jsonrpc,
                result: None,
                error: Some(e),
            }
        }
    };
    JsonRpcResponse {
        id,
        jsonrpc,
        result: Some(result),
        error: None,
    }
}

async fn inner_handle_request(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> std::result::Result<serde_json::Value, JsonRpcError> {
    let method: MethodName = match json_utils::get_field(&request, "method") {
        serde_json::Value::String(m) => m.parse()?,
        _ => {
            return Err(JsonRpcError {
                code: -32601,
                data: serde_json::Value::Null,
                message: "Invalid/missing method".into(),
            });
        }
    };

    match method {
        MethodName::ForkChoiceUpdatedV3 => {
            methods::forkchoice_updated::execute_v3(request, state_channel).await
        }
        MethodName::GetPayloadV3 => methods::get_payload::execute_v3(request, state_channel).await,
        MethodName::NewPayloadV3 => methods::new_payload::execute_v3(request, state_channel).await,
        MethodName::SendRawTransaction => {
            methods::send_raw_transaction::execute(request, state_channel).await
        }
        MethodName::ForkChoiceUpdatedV2 => todo!(),
        MethodName::GetPayloadV2 => todo!(),
        MethodName::NewPayloadV2 => todo!(),
    }
}

fn try_decompress(raw_bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let gz = GzDecoder::new(raw_bytes);
    gz.bytes().collect()
}
