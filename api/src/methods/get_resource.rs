use {
    crate::{json_utils::parse_params_3, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
    umi_shared::primitives::{Address, ToMoveAddress},
};

/// Fetches `resource` by name that belongs to the `account` in the blockchain state corresponding
/// with the block `number`.
///
/// # Arguments
/// * `account`: A "0x" prefixed, 20-byte long, hex encoded number that represents an Ethereum
///   account address. Example: `0x0000000000000000000000000000000000000001`
/// * `resource`: A string encoded identifier of the resource. Example: `event`
/// * `number`: A string that represents a tagged block height, or a "0x" prefixed, hex encoded
///   number that represents the exact block height to read from. Example: `latest`
pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (account, resource, number): (Address, String, _) = parse_params_3(request)?;

    let response =
        app.move_resource_by_height(account.to_move_address(), resource.as_str(), number)?;

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
        umi_shared::primitives::{ToEthAddress, U64},
    };

    const TYPE: &str = "0x1::code::PackageRegistry";

    pub fn example_request(tag: BlockNumberOrTag) -> serde_json::Value {
        serde_json::json!({
            "id": 1,
            "jsonrpc": "2.0",
            "method": "mv_getResource",
            "params": [umi_genesis::FRAMEWORK_ADDRESS.to_eth_address(), "0x1::code::PackageRegistry", tag]
        })
    }

    pub fn type_from_response(response: serde_json::Value) -> String {
        response
            .as_object()
            .unwrap()
            .get("type")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn test_execute_reads_module_successfully() {
        let (reader, _app) = create_app();
        let request = example_request(Number(0));

        let expected_response: serde_json::Value =
            serde_json::from_str(include_str!("get_resource_output.json")).unwrap();

        let response = execute(request, &reader).await.unwrap();

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_non_existent_block_is_bad_input() {
        let (reader, _app) = create_app();

        let request = example_request(BlockNumberOrTag::Number(5));

        let response = execute(request, &reader).await;

        let expected_response = JsonRpcError::block_not_found(umi_shared::error::Error::User(
            umi_shared::error::UserError::InvalidBlockHeight(5),
        ));

        assert_eq!(response.unwrap_err(), expected_response);
    }

    #[tokio::test]
    async fn test_byte_code_is_kept_with_newly_built_block() {
        let (state_channel, rx) = mpsc::channel(10);
        let (reader, mut app) = create_app();
        let state: CommandActor<TestDependencies> = CommandActor::new(rx, &mut app);

        umi_app::run_with_actor(state, async move {
            let request = example_request(Latest);
            let response = execute(request, &reader).await.unwrap();

            let actual_bytecode = type_from_response(response);
            let expected_bytecode = TYPE;

            assert_eq!(actual_bytecode, expected_bytecode);

            // Create a block, so the block height becomes 1
            let msg = Command::StartBlockBuild {
                payload_attributes: Default::default(),
                payload_id: U64::from(0x03421ee50df45cacu64),
            };
            state_channel.send(msg).await.unwrap();

            state_channel.reserve_many(10).await.unwrap();

            let request = example_request(Latest);
            let response = execute(request, &reader).await.unwrap();

            let actual_bytecode = type_from_response(response);
            let expected_bytecode = TYPE;

            assert_eq!(actual_bytecode, expected_bytecode);
        })
        .await;
    }

    #[test_case(Safe; "safe")]
    #[test_case(Pending; "pending")]
    #[test_case(Finalized; "finalized")]
    #[tokio::test]
    async fn test_latest_block_byte_code_is_same_as_tag(tag: BlockNumberOrTag) {
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

            let actual_bytecode = type_from_response(response);
            let expected_bytecode = TYPE;

            assert_eq!(actual_bytecode, expected_bytecode);
        })
        .await;
    }
}
