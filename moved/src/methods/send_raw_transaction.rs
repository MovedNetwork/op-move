use {
    crate::{
        json_utils::{self, access_state_error},
        types::{jsonrpc::JsonRpcError, state::StateMessage},
    },
    alloy_consensus::transaction::TxEnvelope,
    alloy_rlp::Decodable,
    ethers_core::types::{Bytes, H256},
    tokio::sync::mpsc,
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx = parse_params(request)?;
    let response = inner_execute(tx, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params(request: serde_json::Value) -> Result<TxEnvelope, JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [x] => {
            let bytes: Bytes = json_utils::deserialize(x)?;
            let mut slice: &[u8] = bytes.as_ref();
            let tx = TxEnvelope::decode(&mut slice).map_err(|e| JsonRpcError {
                code: -32602,
                data: request,
                message: format!("RLP decode failed: {e:?}"),
            })?;
            Ok(tx)
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute(
    tx: TxEnvelope,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<H256, JsonRpcError> {
    let tx_hash = tx.tx_hash().0.into();

    let msg = StateMessage::AddTransaction { tx };
    state_channel.send(msg).await.map_err(access_state_error)?;

    Ok(tx_hash)
}

#[tokio::test]
async fn test_execute() {
    let (state_channel, rx) = tokio::sync::mpsc::channel(10);
    let state = crate::state_actor::StateActor::new_in_memory(rx);
    let state_handle = state.spawn();

    let request: serde_json::Value = serde_json::from_str(
        r#"
            {
                "method": "eth_sendRawTransaction",
                "params": [
                "0x02f86f82a45580808346a8928252089465d08a056c17ae13370565b04cf77d2afa1cb9fa8806f05b59d3b2000080c080a0dd50efde9a4d2f01f5248e1a983165c8cfa5f193b07b4b094f4078ad4717c1e4a017db1be1e8751b09e033bcffca982d0fe4919ff6b8594654e06647dee9292750"
                ],
                "id": 4,
                "jsonrpc": "2.0"
            }
    "#,
    )
    .unwrap();

    let expected_response: serde_json::Value = serde_json::from_str(
        r#""0x7185c49a6b650a42cae042cde2228bf11a3f7e32c9a62dd59b4b52ebd5d3e090""#,
    )
    .unwrap();

    let response = execute(request, state_channel).await.unwrap();

    assert_eq!(response, expected_response);
    state_handle.await.unwrap();
}