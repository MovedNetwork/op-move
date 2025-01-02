use {
    crate::{
        json_utils,
        json_utils::{access_state_error, transaction_error},
        jsonrpc::JsonRpcError,
    },
    alloy::{eips::BlockNumberOrTag, rpc::types::TransactionRequest},
    moved::types::state::{Query, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (transaction, block_number) = parse_params(request)?;
    let response = inner_execute(transaction, block_number, state_channel).await?;

    // Format the gas estimate as a hex string
    Ok(serde_json::to_value(format!("0x{:x}", response))
        .expect("Must be able to JSON-serialize response"))
}

fn parse_params(
    request: serde_json::Value,
) -> Result<(TransactionRequest, BlockNumberOrTag), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [a, b] => {
            let transaction: TransactionRequest = json_utils::deserialize(a)?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(b)?;
            Ok((transaction, block_number))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute(
    transaction: TransactionRequest,
    block_number: BlockNumberOrTag,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<u64, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = Query::EstimateGas {
        transaction,
        block_number,
        response_channel: tx,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let response = rx.await.map_err(access_state_error)?;
    response.map_err(transaction_error)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::{create_state_actor, deposit_eth},
        alloy::primitives::Address,
        moved::primitives::U64,
        std::str::FromStr,
        test_case::test_case,
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [
                {
                    "from": "0x0000000000000000000000000000000000000001",
                    "to": null,
                    "input": "0xa11ce0"
                },
                block,
            ],
            "id": 1
        });

        let (transaction, block_number) = parse_params(request.clone()).unwrap();
        assert_eq!(
            transaction.from.unwrap(),
            Address::from_str("0x0000000000000000000000000000000000000001").unwrap()
        );
        match block {
            "latest" => assert_eq!(block_number, BlockNumberOrTag::Latest),
            "pending" => assert_eq!(block_number, BlockNumberOrTag::Pending),
            _ => assert_eq!(
                block_number,
                BlockNumberOrTag::Number(U64::from_str(block).unwrap().into_limbs()[0])
            ),
        }
    }

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    #[tokio::test]
    async fn test_execute(block: &str) {
        let (state_actor, state_channel) = create_state_actor();
        let state_handle = state_actor.spawn();

        deposit_eth("0x8fd379246834eac74b8419ffda202cf8051f7a03", &state_channel).await;

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [
                {
                    "from": "0x8fd379246834eac74b8419ffda202cf8051f7a03",
                    "input": "0x01fd01a11ceb0b0600000009010002020204030614051a0e07283d0865200a8501050c8a01490dd3010200000001080000020001000003000200000400030000050403000105010101030002060c0301070307636f756e74657207436f756e7465720e636f756e7465725f657869737473096765745f636f756e7409696e6372656d656e74077075626c69736801690000000000000000000000008fd379246834eac74b8419ffda202cf8051f7a0300020106030001000003030b00290002010100010003050b002b00100014020201040100050b0b002a000f000c010a0114060100000000000000160b0115020301040003050b000b0112002d0002000000"
                },
                block,
            ],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x2ee0""#).unwrap();
        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
