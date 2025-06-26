use {
    crate::{json_utils, jsonrpc::JsonRpcError},
    alloy::rlp::Decodable,
    umi_app::{Command, CommandQueue},
    umi_execution::transaction::{NormalizedEthTransaction, UmiTxEnvelope},
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
            Ok(parse_transaction_bytes(&bytes)?)
        }
        _ => Err(JsonRpcError::too_many_params_error(request)),
    }
}

fn parse_transaction_bytes(
    bytes: &Bytes,
) -> Result<NormalizedEthTransaction, umi_shared::error::Error> {
    let mut slice: &[u8] = bytes.as_ref();
    let l1_gas_fee_input = slice.into();
    let umi_tx = UmiTxEnvelope::decode(&mut slice)?;
    let normalized_tx =
        NormalizedEthTransaction::try_from(umi_tx)?.with_gas_input(l1_gas_fee_input);
    Ok(normalized_tx)
}

async fn inner_execute(
    tx: NormalizedEthTransaction,
    queue: CommandQueue,
) -> Result<B256, JsonRpcError> {
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

    pub fn example_bad_request() -> serde_json::Value {
        // deposit tx
        serde_json::from_str(
            r#"
                {
                    "method": "eth_sendRawTransaction",
                    "params": [
                    "7ef8f8a032595a51f0561028c684fbeeb46c7221a34be9a2eedda60a93069dd77320407e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000000000000000000000000000000000000006807cdc800000000000000220000000000000000000000000000000000000000000000000000000000a68a3a000000000000000000000000000000000000000000000000000000000000000198663a8bf712c08273a02876877759b43dc4df514214cc2f6008870b9a8503380000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9"
                    ],
                    "id": 11,
                    "jsonrpc": "2.0"
                }
        "#,
        ).unwrap()
    }

    #[tokio::test]
    async fn test_bad_input() {
        let (_reader, mut app) = create_app();

        let (queue, state) = umi_app::create(&mut app, 10);

        umi_app::run_with_actor(state, async move {
            let request = example_bad_request();

            // This is actually alloy's response, as it doesn't recognize OP deposit type
            let expected_response =
                JsonRpcError::transaction_error("Could not decode RLP bytes: unexpected tx type");

            let response = execute(request, queue).await.unwrap_err();

            assert_eq!(response, expected_response);
        })
        .await;
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
