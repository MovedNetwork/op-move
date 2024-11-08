use {
    crate::{json_utils, json_utils::access_state_error, jsonrpc::JsonRpcError},
    alloy::{eips::BlockNumberOrTag, primitives::Address},
    moved::types::state::StateMessage,
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (address, block_number) = parse_params(request)?;
    let response = inner_execute(address, block_number, state_channel).await?;

    // Format the balance as a hex string
    Ok(serde_json::to_value(format!("0x{:x}", response))
        .expect("Must be able to JSON-serialize response"))
}

fn parse_params(request: serde_json::Value) -> Result<(Address, BlockNumberOrTag), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [a, b] => {
            let address: Address = json_utils::deserialize(a)?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(b)?;
            Ok((address, block_number))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute(
    address: Address,
    block_number: BlockNumberOrTag,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<u64, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = StateMessage::GetBalance {
        address,
        block_number,
        response_channel: tx,
    };
    state_channel.send(msg).await.map_err(access_state_error)?;
    let response = rx.await.map_err(access_state_error)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        moved::{
            block::{BlockMemory, Eip1559GasFee, InMemoryBlockRepository},
            genesis::init_state,
            primitives::{B256, U256, U64},
            state_actor::{InMemoryQueries, StatePayloadId},
            storage::InMemoryState,
        },
        std::{str::FromStr, sync::Arc},
        test_case::test_case,
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params_with_block_number(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [
                "0x0000000000000000000000000000000000000001",
                block,
            ],
            "id": 1
        });

        let (address, block_number) = parse_params(request).unwrap();
        assert_eq!(
            address,
            Address::from_str("0x0000000000000000000000000000000000000001").unwrap()
        );
        match block {
            "latest" => assert_eq!(block_number, BlockNumberOrTag::Latest),
            "pending" => assert_eq!(block_number, BlockNumberOrTag::Pending),
            _ => assert_eq!(
                block_number,
                BlockNumberOrTag::Number(U64::from_str(block).unwrap().into_limbs()[0])
            ),
        }
    }

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    #[tokio::test]
    async fn test_execute(block: &str) {
        let genesis_config = moved::genesis::config::GenesisConfig::default();
        let mut state = InMemoryState::new();
        init_state(&genesis_config, &mut state);

        let (state_channel, rx) = mpsc::channel(10);
        let block_memory = Arc::new(BlockMemory::default());
        let state_actor = moved::state_actor::StateActor::new(
            rx,
            state,
            B256::ZERO,
            genesis_config,
            StatePayloadId,
            B256::ZERO,
            InMemoryBlockRepository::new(block_memory.clone()),
            Eip1559GasFee::default(),
            U256::ZERO,
            (),
            InMemoryQueries::new(block_memory),
        );

        let state_handle = state_actor.spawn();
        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [
                "0x0000000000000000000000000000000000000001",
                block,
            ],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x0""#).unwrap();
        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
