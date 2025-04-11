use {
    crate::jsonrpc::JsonRpcError,
    moved_app::{Application, Dependencies},
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub async fn execute(
    app: &Arc<RwLock<Application<impl Dependencies>>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let response = app.read().await.chain_id();
    Ok(serde_json::to_value(format!("{response:#x}"))
        .expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_app};

    #[tokio::test]
    async fn test_execute() {
        let app = create_app();

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x194""#).unwrap();
        let actual_response = execute(&app).await.unwrap();

        assert_eq!(actual_response, expected_response);
    }
}
