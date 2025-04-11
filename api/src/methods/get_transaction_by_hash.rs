use {
    crate::{json_utils::parse_params_1, jsonrpc::JsonRpcError, schema::GetTransactionResponse},
    moved_app::{Application, Dependencies},
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub async fn execute(
    request: serde_json::Value,
    app: &Arc<RwLock<Application<impl Dependencies>>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx_hash = parse_params_1(request)?;

    let response = app
        .read()
        .await
        .transaction_by_hash(tx_hash)
        .map(GetTransactionResponse::from);

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
        moved_app::{StateActor, TestDependencies},
        serde_json::json,
        std::iter,
        tokio::sync::mpsc,
    };

    #[tokio::test]
    async fn test_execute() {
        let app = create_app();
        let (state_channel, rx) = mpsc::channel(10);
        let state: StateActor<TestDependencies> = StateActor::new(rx, app.clone());
        let state_handle = state.spawn();

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

        let request = serde_json::Value::Object(
            iter::once((
                "params".to_string(),
                serde_json::Value::Array(vec![tx_hash]),
            ))
            .collect(),
        );
        let actual_response: serde_json::Value =
            serde_json::from_value(execute(request, &app).await.unwrap()).unwrap();
        let expected_response = json!({
            "type": "0x2",
            "chainId": "0x194",
            "nonce": "0x0",
            "gas": "0xffffffffffffffff",
            "maxFeePerGas": "0x0",
            "maxPriorityFeePerGas": "0x0",
            "to": "0x8fd379246834eac74b8419ffda202cf8051f7a03",
            "value": "0x3d",
            "accessList": [],
            "input": "0x",
            "r": "0x78c716fef14bfcb7c2c9ff4abeb741529874fe7046ac042871f9d8490db55f5c",
            "s": "0x1fd5186e08990692d54912b476496f12c48bd7cc540a92d211dde232133ed17",
            "yParity": "0x0",
            "v": "0x0",
            "hash": "0x3545efb3ce7a22353c346c98771640131b81baa64eb03113b20ad2bef5c0ec53",
            "blockHash": block_hash,
            "blockNumber": "0x1",
            "transactionIndex": "0x2",
            "from": "0x88f9b82462f6c4bf4a0fb15e5c3971559a316e7f",
            "gasPrice": "0x0"
        });

        assert_eq!(actual_response, expected_response);
    }
}
