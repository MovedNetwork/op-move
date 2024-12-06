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

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
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
) -> Result<Vec<u8>, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = Query::Call {
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
        crate::methods::tests::{create_state_actor, deploy_contract, deposit_eth},
        alloy::{
            hex::FromHex,
            primitives::{Address, Bytes},
        },
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
    async fn test_execute_call_entry_fn(block: &str) {
        let (state_actor, state_channel) = create_state_actor();
        let state_handle = state_actor.spawn();

        // Add funds to the account to deploy the `counter` contract
        deposit_eth("0x8fd379246834eac74b8419ffda202cf8051f7a03", &state_channel).await;
        deploy_contract(Bytes::from_hex("01fd01a11ceb0b0600000009010002020204030614051a0e07283d0865200a8501050c8a01490dd3010200000001080000020001000003000200000400030000050403000105010101030002060c0301070307636f756e74657207436f756e7465720e636f756e7465725f657869737473096765745f636f756e7409696e6372656d656e74077075626c69736801690000000000000000000000008fd379246834eac74b8419ffda202cf8051f7a0300020106030001000003030b00290002010100010003050b002b00100014020201040100050b0b002a000f000c010a0114060100000000000000160b0115020301040003050b000b0112002d0002000000").unwrap(), &state_channel).await;

        // Check if the count exists for an address, which returns false
        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [
                {
                    "from": "0x8fd379246834eac74b8419ffda202cf8051f7a03",
                    "to": "0x8fd379246834eac74b8419ffda202cf8051f7a03",
                    "input": "0x020000000000000000000000008fd379246834eac74b8419ffda202cf8051f7a0307636f756e7465720e636f756e7465725f6578697374730001200000000000000000000000000000000000000000000000000000000000000b0b"
                },
                block,
            ],
            "id": 1
        });

        let expected_response = serde_json::json!([1, 1, 0, 0]);
        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    #[tokio::test]
    async fn test_execute_call_script(block: &str) {
        let (state_actor, state_channel) = create_state_actor();
        let state_handle = state_actor.spawn();

        // Add funds to the account to deploy the `counter` contract
        deposit_eth("0x8fd379246834eac74b8419ffda202cf8051f7a03", &state_channel).await;
        deploy_contract(Bytes::from_hex("01fd01a11ceb0b0600000009010002020204030614051a0e07283d0865200a8501050c8a01490dd3010200000001080000020001000003000200000400030000050403000105010101030002060c0301070307636f756e74657207436f756e7465720e636f756e7465725f657869737473096765745f636f756e7409696e6372656d656e74077075626c69736801690000000000000000000000008fd379246834eac74b8419ffda202cf8051f7a0300020106030001000003030b00290002010100010003050b002b00100014020201040100050b0b002a000f000c010a0114060100000000000000160b0115020301040003050b000b0112002d0002000000").unwrap(), &state_channel).await;

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [
                {
                    "from": "0x8fd379246834eac74b8419ffda202cf8051f7a03",
                    "input": "00fb01a11ceb0b060000000501000403041405180c072436085a4000000101000203010001030002000104010400010501020002060c0301050001060c0103067369676e657207636f756e7465720a616464726573735f6f66077075626c697368096765745f636f756e7409696e6372656d656e7400000000000000000000000000000000000000000000000000000000000000010000000000000000000000008fd379246834eac74b8419ffda202cf8051f7a030000011b0a0011000c020b000a0111010a0211020a0121040c050e060000000000000000270a0211030b0211020b0106010000000000000016210418051a06010000000000000027020001010d00000000000000"
                },
                block,
            ],
            "id": 1
        });
        // Counter script call should succeed
        execute(request, state_channel).await.unwrap();

        state_handle.await.unwrap();
    }
}
