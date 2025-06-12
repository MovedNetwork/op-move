use {
    crate::{json_utils::parse_params_1, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute(
    request: serde_json::Value,
    app: &ApplicationReader<impl Dependencies>,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx_hash = parse_params_1(request)?;

    let response = app.transaction_receipt(tx_hash)?;

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
        std::iter,
        umi_blockchain::receipt::TransactionReceipt,
    };

    #[tokio::test]
    async fn test_bad_input() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [
                // bad hash
                "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb00000",
            ],
            "id": 1
        });

        let response = execute(request, &reader).await;

        // A missing transaction is not an error, just a null response.
        assert_eq!(response.unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_execute() {
        let (reader, mut app) = create_app();
        let (queue, state) = umi_app::create(&mut app, 10);

        umi_app::run(state, async move {
            // 1. Send transaction
            let tx_hash = send_raw_transaction::execute(
                send_raw_transaction::tests::example_request(),
                queue.clone(),
            )
            .await
            .unwrap();

            // 2. Trigger block production
            let forkchoice_response: ForkchoiceUpdatedResponseV1 = serde_json::from_value(
                forkchoice_updated::execute_v3(
                    forkchoice_updated::tests::example_request(),
                    queue.clone(),
                    &0x03421ee50df45cacu64,
                )
                .await
                .unwrap(),
            )
            .unwrap();

            queue.wait_for_pending_commands().await;

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
                serde_json::from_value(get_payload::execute_v3(request, &reader).await.unwrap())
                    .unwrap();
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
                serde_json::from_value(execute(request, &reader).await.unwrap()).unwrap();

            // Confirm the receipt contains correct information about the transaction
            assert_eq!(receipt.inner.transaction_index, Some(2));
            assert_eq!(receipt.inner.block_hash, Some(block_hash));
            assert!(receipt.inner.inner.status());
            assert_eq!(receipt.inner.inner.logs().len(), 2);
        })
        .await;
    }
}
