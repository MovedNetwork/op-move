use {
    crate::{json_utils, json_utils::access_state_error, jsonrpc::JsonRpcError},
    alloy::{
        eips::BlockNumberOrTag,
        primitives::{Address, U256},
    },
    moved::types::state::{Query, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (address, block_number) = parse_params(request)?;
    let response = inner_execute(address, block_number, state_channel).await?;

    // Format the balance as a hex string
    Ok(serde_json::to_value(format!("0x{:x}", response))
        .expect("Must be able to JSON-serialize response"))
}

fn parse_params(request: serde_json::Value) -> Result<(Address, BlockNumberOrTag), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [a, b] => {
            let address: Address = json_utils::deserialize(a)?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(b)?;
            Ok((address, block_number))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute(
    address: Address,
    block_number: BlockNumberOrTag,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<U256, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = Query::BalanceByHeight {
        address,
        height: block_number,
        response_channel: tx,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let response = rx.await.map_err(access_state_error)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::{create_state_actor, create_state_actor_with_mock_state_queries},
        alloy::hex,
        move_core_types::account_address::AccountAddress,
        moved::primitives::U64,
        std::str::FromStr,
        test_case::test_case,
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params_with_block_number(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [
                "0x0000000000000000000000000000000000000001",
                block,
            ],
            "id": 1
        });

        let (address, block_number) = parse_params(request).unwrap();
        assert_eq!(
            address,
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
        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [
                "0x0000000000000000000000000000000000000001",
                block,
            ],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x0""#).unwrap();
        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_endpoint_returns_json_encoded_balance_query_result_successfully() {
        let expected_balance = 5;
        let height = 3;
        let (state_actor, state_channel) = create_state_actor_with_mock_state_queries(
            height,
            AccountAddress::new(hex!(
                "0000000000000000000000002222222222222223333333333333333333111100"
            )),
        );
        let address = "2222222222222223333333333333333333111100";

        let state_handle = state_actor.spawn();
        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [
                format!("0x{address}"),
                format!("0x{height}"),
            ],
            "id": 1
        });

        let expected_response = serde_json::Value::String(format!("0x{expected_balance}"));
        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
