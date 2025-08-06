use {
    crate::{json_utils::parse_params_0, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params_0(request)?;
    let response = app.client_version();

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_app};

    #[tokio::test]
    async fn test_execute() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "web3_clientVersion",
            "params": [],
            "id": 1
        });

        let actual_response = execute(request, &reader).await.unwrap();

        let parts = actual_response
            .as_str()
            .unwrap()
            .split("/")
            .collect::<Vec<_>>();
        dbg!(&parts);

        assert_eq!(parts.len(), 4);

        assert_eq!(parts[0], "op-move");

        let pkg_ver_parts = parts[1].split(".").collect::<Vec<_>>();
        assert_eq!(pkg_ver_parts.len(), 3);

        let target_triplet_parts = parts[2].split("-").collect::<Vec<_>>();
        assert!(target_triplet_parts.len() > 2 && target_triplet_parts.len() < 5);

        assert!(parts[3].starts_with("rust"));
    }

    #[tokio::test]
    async fn test_bad_input() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "web3_clientVersion",
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
