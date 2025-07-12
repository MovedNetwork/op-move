use umi_app::{ApplicationReader, Dependencies};

use crate::{json_utils::parse_params_0, jsonrpc::JsonRpcError};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params_0(request)?;

    let response = app.max_priority_fee_per_gas()?;

    Ok(serde_json::to_value(format!("{response:#x}"))
        .expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use crate::methods::tests::create_app;

    use super::*;

    #[tokio::test]
    async fn test_execute() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_maxPriorityFeePerGas",
            "params": [],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0xf4240""#).unwrap();
        let response = execute(request, &reader).await.unwrap();

        assert_eq!(response, expected_response);

        let hex_str = response.as_str().unwrap().strip_prefix("0x").unwrap();
        // The value is the minimum suggested fee constant without base fee
        assert_eq!(u64::from_str_radix(hex_str, 16).unwrap(), 1_000_000);
    }

    #[tokio::test]
    async fn test_bad_input() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_maxPriorityFeePerGas",
            "params": ["wrong"],
            "id": 1
        });

        let response = execute(request.clone(), &reader).await;
        assert_eq!(
            response.unwrap_err(),
            JsonRpcError::too_many_params_error(request)
        );
    }
}
