use {
    crate::{json_utils, jsonrpc::JsonRpcError},
    alloy::eips::BlockNumberOrTag,
    moved_app::{ApplicationReader, Dependencies},
};

pub async fn execute(
    request: serde_json::Value,
    app: ApplicationReader<impl Dependencies>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (block_count, block_number, reward_percentiles) = parse_params(request)?;

    let response = app.fee_history(block_count, block_number, reward_percentiles);

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params(
    request: serde_json::Value,
) -> Result<(u64, BlockNumberOrTag, Option<Vec<f64>>), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] | [_] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [a, b] => {
            let block_count = parse_block_count(a)?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(b)?;
            Ok((block_count, block_number, None))
        }
        [a, b, c] => {
            let block_count = parse_block_count(a)?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(b)?;
            let reward_percentiles: Vec<f64> = json_utils::deserialize(c)?;
            if reward_percentiles
                .iter()
                .any(|reward| *reward < 0.0 || *reward > 100.0)
            {
                return Err(JsonRpcError {
                    code: -32602,
                    data: 0.into(),
                    message: "Incorrect reward percentile".into(),
                });
            }
            Ok((block_count, block_number, Some(reward_percentiles)))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

fn parse_block_count(value: &serde_json::Value) -> Result<u64, JsonRpcError> {
    let block_count: String = json_utils::deserialize(value)?;
    let block_count = block_count.trim_start_matches("0x");
    u64::from_str_radix(block_count, 16).map_err(|_| JsonRpcError {
        code: -32602,
        data: 0.into(),
        message: "Block count parsing error".into(),
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*, crate::methods::tests::create_app, moved_shared::primitives::U64,
        std::str::FromStr, test_case::test_case,
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": ["0x1", block, [10.0]],
            "id": 1
        });

        let (block_count, block_number, reward_percentiles) = parse_params(request).unwrap();
        assert_eq!(block_count, 1);
        assert_eq!(reward_percentiles, Some(vec![10f64]));
        match block {
            "latest" => assert_eq!(block_number, BlockNumberOrTag::Latest),
            "pending" => assert_eq!(block_number, BlockNumberOrTag::Pending),
            _ => assert_eq!(
                block_number,
                BlockNumberOrTag::Number(U64::from_str(block).unwrap().into_limbs()[0])
            ),
        }

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": ["0x1", block],
            "id": 1
        });
        let (_, _, reward_percentiles) = parse_params(request).unwrap();
        assert_eq!(reward_percentiles, None);
    }

    #[test]
    fn test_parse_wrong_params() {
        // No params
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": [],
            "id": 1
        });
        let err = parse_params(request).unwrap_err();
        assert_eq!(err.message, "Not enough params");

        // Single param
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": ["0x1"],
            "id": 1
        });
        let err = parse_params(request).unwrap_err();
        assert_eq!(err.message, "Not enough params");

        // Incorrect block count
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": ["0xwrong", "latest", []],
            "id": 1
        });
        let err = parse_params(request).unwrap_err();
        assert_eq!(err.message, "Block count parsing error");

        // Incorrect reward percentile
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": ["0x1", "latest", [-10]],
            "id": 1
        });
        let err = parse_params(request).unwrap_err();
        assert_eq!(err.message, "Incorrect reward percentile");
    }

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    #[tokio::test]
    async fn test_execute(block: &str) {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": [
                "0x2",
                block,
                [
                    20.0
                ],
            ],
            "id": 1
        });

        let expected_response: serde_json::Value =
            serde_json::json!({"gasUsedRatio": [], "oldestBlock": "0x0"});
        let response = execute(request, reader.clone()).await.unwrap();

        assert_eq!(response, expected_response);
    }
}
