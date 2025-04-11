use {
    crate::{
        json_utils::{self, access_state_error},
        jsonrpc::JsonRpcError,
    },
    alloy::{consensus::transaction::TxEnvelope, rlp::Decodable},
    moved_app::Command,
    moved_shared::primitives::{B256, Bytes},
    tokio::sync::mpsc,
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<Command>,
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
    state_channel: mpsc::Sender<Command>,
) -> Result<B256, JsonRpcError> {
    let tx_hash = tx.tx_hash().0.into();

    let msg = Command::AddTransaction { tx };
    state_channel.send(msg).await.map_err(access_state_error)?;

    Ok(tx_hash)
}

#[cfg(test)]
pub mod tests {
    use {super::*, crate::methods::tests::create_state_actor};

    pub fn example_request() -> serde_json::Value {
        serde_json::from_str(
            r#"
                {
                    "method": "eth_sendRawTransaction",
                    "params": [
                    "0xb86d02f86a82019480808088ffffffffffffffff948fd379246834eac74b8419ffda202cf8051f7a033d80c080a078c716fef14bfcb7c2c9ff4abeb741529874fe7046ac042871f9d8490db55f5ca001fd5186e08990692d54912b476496f12c48bd7cc540a92d211dde232133ed17"
                    ],
                    "id": 4,
                    "jsonrpc": "2.0"
                }
        "#,
        ).unwrap()
    }

    #[tokio::test]
    async fn test_execute() {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();

        let request = example_request();

        let expected_response: serde_json::Value = serde_json::from_str(
            r#""0x3545efb3ce7a22353c346c98771640131b81baa64eb03113b20ad2bef5c0ec53""#,
        )
        .unwrap();

        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
