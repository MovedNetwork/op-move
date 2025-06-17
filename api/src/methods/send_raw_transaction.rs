use {
    crate::{json_utils, jsonrpc::JsonRpcError},
    alloy::rlp::Decodable,
    umi_app::{Command, CommandQueue},
    umi_execution::transaction::{
        NormalizedEthTransaction, UmiTxEnvelope,
    },
    umi_shared::primitives::{B256, Bytes},
};

pub async fn execute(
    request: serde_json::Value,
    queue: CommandQueue,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx = parse_params(request)?;
    let response = inner_execute(tx, queue).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params(request: serde_json::Value) -> Result<NormalizedEthTransaction, JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError::not_enough_params_error(request)),
        [x] => {
            let bytes: Bytes = json_utils::deserialize(x)?;
            let mut slice: &[u8] = bytes.as_ref();

            // First decode to UmiTxEnvelope to reject unsupported types early
            let umi_tx: UmiTxEnvelope = UmiTxEnvelope::decode(&mut slice)
                .map_err(|e| {
                    JsonRpcError::parse_error(
                        request,
                        format!("Unsupported or invalid transaction type: {e}"),
                    )
                })?;

            // Normalize transaction at API border
            let normalized_tx: NormalizedEthTransaction = umi_tx.try_into()?;

            Ok(normalized_tx)
        }
        _ => Err(JsonRpcError::too_many_params_error(request)),
    }
}

async fn inner_execute(tx: NormalizedEthTransaction, queue: CommandQueue) -> Result<B256, JsonRpcError> {
    let tx_hash = tx.tx_hash;

    let msg = Command::AddTransaction { tx };
    queue.send(msg).await;

    Ok(tx_hash)
}

#[cfg(test)]
pub mod tests {
    use {super::*, crate::methods::tests::create_app};

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
        let (_reader, mut app) = create_app();
        let (queue, state) = umi_app::create(&mut app, 10);

        umi_app::run_with_actor(state, async move {
            let request = example_request();

            let expected_response: serde_json::Value = serde_json::from_str(
                r#""0x3545efb3ce7a22353c346c98771640131b81baa64eb03113b20ad2bef5c0ec53""#,
            )
            .unwrap();

            let response = execute(request, queue).await.unwrap();

            assert_eq!(response, expected_response);
        })
        .await;
    }
}
