use {
    crate::jsonrpc::JsonRpcError,
    moved_app::{ApplicationReader, Dependencies},
};

pub async fn execute(
    app: &ApplicationReader<impl Dependencies>,
) -> Result<serde_json::Value, JsonRpcError> {
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

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x194""#).unwrap();
        let actual_response = execute(&reader).await.unwrap();

        assert_eq!(actual_response, expected_response);
    }
}
