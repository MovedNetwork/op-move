use {
    crate::{json_utils::parse_params_1, jsonrpc::JsonRpcError, schema::GetTransactionResponse},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx_hash = parse_params_1(request)?;

    let response = app
        .transaction_by_hash(tx_hash)?
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
        serde_json::json,
        std::iter,
    };

    #[tokio::test]
    async fn test_bad_input() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionByHash",
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

        umi_app::run_with_actor(state, async move {
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

            let request = serde_json::Value::Object(
                iter::once((
                    "params".to_string(),
                    serde_json::Value::Array(vec![tx_hash]),
                ))
                .collect(),
            );
            let actual_response: serde_json::Value =
                serde_json::from_value(execute(request, &reader).await.unwrap()).unwrap();
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
        })
        .await;
    }
}
