use {
    crate::{json_utils, jsonrpc::JsonRpcError},
    alloy::{eips::BlockNumberOrTag, primitives::Address},
    moved_app::{Application, Dependencies},
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub async fn execute(
    request: serde_json::Value,
    app: &Arc<RwLock<Application<impl Dependencies>>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (address, block_number) = parse_params(request)?;

    let response = app
        .read()
        .await
        .nonce_by_height(address, block_number)
        .ok_or(JsonRpcError::block_not_found(block_number))?;

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
        [a] => {
            let address: Address = json_utils::deserialize(a)?;
            Ok((address, BlockNumberOrTag::Latest))
        }
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

#[cfg(test)]
mod tests {
    use {
        super::*, crate::methods::tests::create_app_with_mock_state_queries, alloy::hex,
        move_core_types::account_address::AccountAddress, moved_shared::primitives::U64,
        std::str::FromStr, test_case::test_case,
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params_with_block_number(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getNonce",
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
        let app = create_app_with_mock_state_queries(AccountAddress::ONE, 1);

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getNonce",
            "params": [
                "0x0000000000000000000000000000000000000001",
                block,
            ],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x3""#).unwrap();
        let response = execute(request, &app).await.unwrap();

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_endpoint_returns_json_encoded_nonce_query_result_successfully() {
        let expected_nonce = 3;
        let height = 2;
        let app = create_app_with_mock_state_queries(
            AccountAddress::new(hex!(
                "0000000000000000000000002222222222222223333333333333333333111100"
            )),
            height,
        );
        let address = "2222222222222223333333333333333333111100";

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getNonce",
            "params": [
                format!("0x{address}"),
                format!("0x{height}"),
            ],
            "id": 1
        });

        let expected_response = serde_json::Value::String(format!("0x{expected_nonce}"));
        let response = execute(request, &app).await.unwrap();

        assert_eq!(response, expected_response);
    }
}
