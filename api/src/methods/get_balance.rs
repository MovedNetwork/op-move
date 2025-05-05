use {
    crate::{json_utils::parse_params_2, jsonrpc::JsonRpcError},
    moved_app::{Application, ApplicationReader, Dependencies},
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub async fn execute(
    request: serde_json::Value,
    app: &ApplicationReader<impl Dependencies>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (address, block_number) = parse_params_2(request)?;

    let response = app
        .balance_by_height(address, block_number)
        .ok_or(JsonRpcError::block_not_found(block_number))?;

    // Format the balance as a hex string
    Ok(serde_json::Value::String(format!("0x{response:x}")))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::create_app_with_mock_state_queries,
        alloy::{eips::BlockNumberOrTag, hex},
        move_core_types::account_address::AccountAddress,
        moved_shared::primitives::{Address, U64},
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

        let (address, block_number): (Address, BlockNumberOrTag) = parse_params_2(request).unwrap();
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
            "method": "eth_getBalance",
            "params": [
                "0x0000000000000000000000000000000000000001",
                block,
            ],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x5""#).unwrap();
        let response = execute(request, &app).await.unwrap();

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_endpoint_returns_json_encoded_balance_query_result_successfully() {
        let expected_balance = 5;
        let height = 3;
        let app = create_app_with_mock_state_queries(
            AccountAddress::new(hex!(
                "0000000000000000000000002222222222222223333333333333333333111100"
            )),
            height,
        );
        let address = "2222222222222223333333333333333333111100";

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
        let response = execute(request, &app).await.unwrap();

        assert_eq!(response, expected_response);
    }
}
