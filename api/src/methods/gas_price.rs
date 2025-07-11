use umi_app::{ApplicationReader, Dependencies};

use crate::{json_utils::parse_params_0, jsonrpc::JsonRpcError};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params_0(request)?;

    let response = app.gas_price()?;

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
            "method": "eth_gasPrice",
            "params": [],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x3b9aca00""#).unwrap();
        let response = execute(request, &reader).await.unwrap();

        assert_eq!(response, expected_response);
    }
}
