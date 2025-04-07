use {
    crate::{json_utils::parse_params_1, jsonrpc::JsonRpcError},
    moved_app::{Application, Dependencies},
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub async fn execute(
    request: serde_json::Value,
    app: &Arc<RwLock<Application<impl Dependencies>>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx_hash = parse_params_1(request)?;

    let response = app.read().await.transaction_receipt(tx_hash);

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            methods::{forkchoice_updated, get_payload, send_raw_transaction, tests::create_app},
            schema::{ForkchoiceUpdatedResponseV1, GetPayloadResponseV3},
        },
        moved_app::StateActor,
        moved_blockchain::receipt::TransactionReceipt,
        std::iter,
        tokio::sync::mpsc,
    };

    #[tokio::test]
    async fn test_execute() {
        let (state_channel, rx) = mpsc::channel(10);
        let app = create_app();
        let state_actor = StateActor::new(rx, app.clone());
        let state_handle = state_actor.spawn();

        // 1. Send transaction
        let tx_hash = send_raw_transaction::execute(
            send_raw_transaction::tests::example_request(),
            state_channel.clone(),
        )
        .await
        .unwrap();

        // 2. Trigger block production
        let forkchoice_response: ForkchoiceUpdatedResponseV1 = serde_json::from_value(
            forkchoice_updated::execute_v3(
                forkchoice_updated::tests::example_request(),
                state_channel.clone(),
                &0x03421ee50df45cacu64,
            )
            .await
            .unwrap(),
        )
        .unwrap();

        drop(state_channel);
        state_handle.await.unwrap();

        let request = serde_json::Value::Object(
            iter::once((
                "params".to_string(),
                serde_json::Value::Array(vec![
                    serde_json::to_value(forkchoice_response.payload_id.unwrap()).unwrap(),
                ]),
            ))
            .collect(),
        );
        let payload_response: GetPayloadResponseV3 =
            serde_json::from_value(get_payload::execute_v3(request, &app).await.unwrap()).unwrap();
        let block_hash = payload_response.execution_payload.block_hash;

        // 3. Get transaction receipt
        let request = serde_json::Value::Object(
            iter::once((
                "params".to_string(),
                serde_json::Value::Array(vec![tx_hash]),
            ))
            .collect(),
        );
        let receipt: TransactionReceipt =
            serde_json::from_value(execute(request, &app).await.unwrap()).unwrap();

        // Confirm the receipt contains correct information about the transaction
        assert_eq!(receipt.inner.transaction_index, Some(2));
        assert_eq!(receipt.inner.block_hash, Some(block_hash));
        assert!(receipt.inner.inner.status());
        assert_eq!(receipt.inner.inner.logs().len(), 2);
    }
}
