use {
    crate::{json_utils::parse_params_2, jsonrpc::JsonRpcError, schema::GetBlockResponse},
    moved_app::{Application, Dependencies},
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub async fn execute(
    request: serde_json::Value,
    app: &Arc<RwLock<Application<impl Dependencies>>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (block_hash, include_transactions) = parse_params_2(request)?;

    let response = app
        .read()
        .await
        .block_by_hash(block_hash, include_transactions)
        .map(GetBlockResponse::from);

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_app};

    pub fn example_request() -> serde_json::Value {
        serde_json::from_str(
            r#"
            {
                "id": 1,
                "jsonrpc": "2.0",
                "method": "eth_getBlockByHash",
                "params": [
                    "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    false
                ]
            }
        "#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_execute_reads_genesis_block_successfully() {
        let app = create_app();
        let request = example_request();

        let expected_response: serde_json::Value = serde_json::from_str(r#"
        {
            "hash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "miner": "0x0000000000000000000000000000000000000000",
            "stateRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "difficulty": "0x0",
            "number": "0x0",
            "gasLimit": "0x0",
            "gasUsed": "0x0",
            "timestamp": "0x0",
            "extraData": "0x",
            "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "nonce": "0x0000000000000000",
            "uncles": [],
            "transactions": [],
            "withdrawals": []
        }"#).unwrap();

        let response = execute(request, &app).await.unwrap();

        assert_eq!(response, expected_response);
    }
}
