use {
    crate::{json_utils::parse_params_2, jsonrpc::JsonRpcError, schema::GetBlockResponse},
    umi_app::{ApplicationReader, Dependencies},
};

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (number, include_transactions) = parse_params_2(request)?;

    let response = app
        .block_by_height(number, include_transactions)
        .map(GetBlockResponse::from)?;

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::create_app,
        alloy::eips::BlockNumberOrTag::{self, *},
        test_case::test_case,
        tokio::sync::mpsc,
        umi_app::{Command, CommandActor, TestDependencies},
        umi_shared::primitives::U64,
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
        let (reader, _app) = create_app();
        let request = example_request(Number(0));

        let expected_response: serde_json::Value = serde_json::from_str(r#"
        {
            "hash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "miner": "0x0000000000000000000000000000000000000000",
            "stateRoot": "0x4805267476cb522274ec2fe790b4dc6e889ed0d57377f90770d4a658f6b8e4ae",
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

        let response = execute(request, &reader).await.unwrap();

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_bad_input() {
        let (reader, _app) = create_app();

        let request = example_request(BlockNumberOrTag::Number(5));

        let response = execute(request, &reader).await;

        let expected_response = JsonRpcError::block_not_found(umi_shared::error::Error::User(
            umi_shared::error::UserError::InvalidBlockHeight(5),
        ));

        assert_eq!(response.unwrap_err(), expected_response);
    }

    #[tokio::test]
    async fn test_latest_block_height_is_updated_with_newly_built_block() {
        let (state_channel, rx) = mpsc::channel(10);
        let (reader, mut app) = create_app();
        let state: CommandActor<TestDependencies> = CommandActor::new(rx, &mut app);

        umi_app::run_with_actor(state, async move {
            let request = example_request(Latest);
            let response = execute(request, &reader).await.unwrap();
            assert_eq!(get_block_number_from_response(response), "0x0");

            // Create a block, so the block height becomes 1
            let msg = Command::StartBlockBuild {
                payload_attributes: Default::default(),
                payload_id: U64::from(0x03421ee50df45cacu64),
            };
            state_channel.send(msg).await.unwrap();

            state_channel.reserve_many(10).await.unwrap();

            let request = example_request(Latest);
            let response = execute(request, &reader).await.unwrap();

            assert_eq!(get_block_number_from_response(response), "0x1");

            drop(state_channel);
        })
        .await;
    }

    #[test_case(Safe; "safe")]
    #[test_case(Pending; "pending")]
    #[test_case(Finalized; "finalized")]
    #[tokio::test]
    async fn test_latest_block_height_is_same_as_tag(tag: BlockNumberOrTag) {
        let (state_channel, rx) = mpsc::channel(10);
        let (reader, mut app) = create_app();
        let state: CommandActor<TestDependencies> = CommandActor::new(rx, &mut app);

        umi_app::run_with_actor(state, async move {
            let msg = Command::StartBlockBuild {
                payload_attributes: Default::default(),
                payload_id: U64::from(0x03421ee50df45cacu64),
            };
            state_channel.send(msg).await.unwrap();

            state_channel.reserve_many(10).await.unwrap();

            let request = example_request(tag);
            let response = execute(request, &reader).await.unwrap();
            assert_eq!(get_block_number_from_response(response), "0x1");
        })
        .await;
    }
}
