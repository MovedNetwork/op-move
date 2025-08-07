use {
    crate::{json_utils::parse_params_2, jsonrpc::JsonRpcError, schema::GetBlockResponse},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (block_hash, include_transactions) = parse_params_2(request)?;

    let response = app
        .block_by_hash(block_hash, include_transactions)
        .map(GetBlockResponse::from)?;

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_app, alloy::primitives::B256, std::str::FromStr};

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
        let (reader, _app) = create_app();
        let request = example_request();

        let expected_response: serde_json::Value = serde_json::from_str(r#"
        {
            "hash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "miner": "0x0000000000000000000000000000000000000000",
            "stateRoot": "0x7a60fc9568ab4beac4305f381312125963c32ffb7a0d5b3afdd4a9ecca902348",
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
            "size": "0x1f0",
            "uncles": [],
            "transactions": [],
            "withdrawals": []
        }"#).unwrap();

        let response = execute(request, &reader).await.unwrap();

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_bad_input() {
        let (reader, _app) = create_app();

        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByHash",
            "params": [
                // bad hash
                "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb00000",
                false
            ],
            "id": 1
        });

        let response = execute(request, &reader).await;

        let block_hash =
            B256::from_str("0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb00000")
                .unwrap();

        assert_eq!(
            response.unwrap_err(),
            JsonRpcError::block_not_found(umi_shared::error::Error::User(
                umi_shared::error::UserError::InvalidBlockHash(block_hash)
            ))
        );
    }
}
