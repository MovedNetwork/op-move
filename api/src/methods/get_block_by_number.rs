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
    let (number, include_transactions) = parse_params_2(request)?;

    let response = app
        .read()
        .await
        .block_by_height(number, include_transactions)
        .map(GetBlockResponse::from);

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::create_app,
        alloy::eips::BlockNumberOrTag::{self, *},
        moved_app::{Command, StateActor, TestDependencies},
        moved_shared::primitives::U64,
        test_case::test_case,
        tokio::sync::mpsc,
    };

    pub fn example_request(tag: BlockNumberOrTag) -> serde_json::Value {
        serde_json::json!({
            "id": 1,
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": [tag, false]
        })
    }

    pub fn get_block_number_from_response(response: serde_json::Value) -> String {
        response
            .as_object()
            .unwrap()
            .get("number") // Block number
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn test_execute_reads_genesis_block_successfully() {
        let app = create_app();
        let request = example_request(Number(0));

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

    #[tokio::test]
    async fn test_latest_block_height_is_updated_with_newly_built_block() {
        let (state_channel, rx) = mpsc::channel(10);
        let app = create_app();
        let state: StateActor<TestDependencies> = StateActor::new(rx, app.clone());
        let state_handle = state.spawn();

        let request = example_request(Latest);
        let response = execute(request, &app).await.unwrap();
        assert_eq!(get_block_number_from_response(response), "0x0");

        // Create a block, so the block height becomes 1
        let msg = Command::StartBlockBuild {
            payload_attributes: Default::default(),
            payload_id: U64::from(0x03421ee50df45cacu64),
        }
        .into();
        state_channel.send(msg).await.unwrap();
        drop(state_channel);
        state_handle.await.unwrap();

        let request = example_request(Latest);
        let response = execute(request, &app).await.unwrap();
        assert_eq!(get_block_number_from_response(response), "0x1");
    }

    #[test_case(Safe; "safe")]
    #[test_case(Pending; "pending")]
    #[test_case(Finalized; "finalized")]
    #[tokio::test]
    async fn test_latest_block_height_is_same_as_tag(tag: BlockNumberOrTag) {
        let (state_channel, rx) = mpsc::channel(10);
        let app = create_app();
        let state: StateActor<TestDependencies> = StateActor::new(rx, app.clone());
        let state_handle = state.spawn();

        let msg = Command::StartBlockBuild {
            payload_attributes: Default::default(),
            payload_id: U64::from(0x03421ee50df45cacu64),
        }
        .into();
        state_channel.send(msg).await.unwrap();
        drop(state_channel);
        state_handle.await.unwrap();

        let request = example_request(tag);
        let response = execute(request, &app).await.unwrap();
        assert_eq!(get_block_number_from_response(response), "0x1");
    }
}
