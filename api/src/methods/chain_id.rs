use {
    crate::{json_utils::parse_params_0, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params_0(request)?;
    let response = app.chain_id();

    Ok(serde_json::to_value(format!("{response:#x}"))
        .expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_app};

    #[tokio::test]
    async fn test_execute() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": [],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x194""#).unwrap();
        let actual_response = execute(request, &reader).await.unwrap();

        assert_eq!(actual_response, expected_response);
    }
}
