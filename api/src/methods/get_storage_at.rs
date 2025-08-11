use {
    crate::{json_utils::parse_params_3, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (address, index, block_number) = parse_params_3(request)?;

    let value = app.storage(address, index, block_number)?;

    Ok(serde_json::Value::String(format!("0x{value:064x}")))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::create_app_with_mock_state_queries,
        alloy::eips::BlockNumberOrTag,
        move_core_types::account_address::AccountAddress,
        std::str::FromStr,
        test_case::test_case,
        umi_shared::primitives::{Address, U64, U256},
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params_with_block_number(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getStorageAt",
            "params": [
                "0x0000000000000000000000000000000000000001",
                "0x0",
                block,
            ],
            "id": 1
        });

        let (address, _index, block_number): (Address, U256, BlockNumberOrTag) =
            parse_params_3(request).unwrap();
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
        let (reader, _app) = *create_app_with_mock_state_queries(AccountAddress::ONE, 1);

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getStorageAt",
            "params": [
                "0x0000000000000000000000000000000000000001",
                "0x0",
                block,
            ],
            "id": 1
        });

        // MockStateQueries::proof_at returns default response with empty storage_proof,
        // so the value should be zero and formatted as 32-byte hex.
        let expected_response = serde_json::Value::String(format!("0x{:064x}", U256::ZERO));
        let response = execute(request, &reader).await.unwrap();

        assert_eq!(response, expected_response);
    }
}
