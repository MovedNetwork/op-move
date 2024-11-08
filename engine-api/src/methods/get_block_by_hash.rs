use {
    crate::{json_utils, jsonrpc::JsonRpcError, schema::GetBlockResponse},
    moved::{primitives::B256, types::state::StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (block_hash, include_transactions) = parse_params(request)?;
    let response = inner_execute(block_hash, include_transactions, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

async fn inner_execute(
    block_hash: B256,
    include_transactions: bool,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<Option<GetBlockResponse>, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = StateMessage::GetBlockByHash {
        block_hash,
        include_transactions,
        response_channel: tx,
    };
    state_channel.send(msg).await?;
    Ok(rx.await.map(|v| v.map(Into::into))?)
}

fn parse_params(request: serde_json::Value) -> Result<(B256, bool), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] | [_] => Err(JsonRpcError::parse_error(request, "Not enough params")),
        [x, y] => {
            let block_hash: B256 = json_utils::deserialize(x)?;
            let include_transactions: bool = json_utils::deserialize(y)?;
            Ok((block_hash, include_transactions))
        }
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::hex,
        moved::{
            block::{Block, BlockMemory, BlockRepository, Eip1559GasFee, InMemoryBlockRepository},
            genesis::{config::GenesisConfig, init_state},
            primitives::U256,
            state_actor::InMemoryQueries,
            storage::InMemoryState,
        },
        std::sync::Arc,
    };

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
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let block_memory = Arc::new(BlockMemory::default());
        let mut repository = InMemoryBlockRepository::new(block_memory.clone());
        repository.add(genesis_block);

        let mut state = InMemoryState::new();
        init_state(&genesis_config, &mut state);

        let state = moved::state_actor::StateActor::new(
            rx,
            state,
            head_hash,
            genesis_config,
            0x03421ee50df45cacu64,
            B256::ZERO,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            (),
            InMemoryQueries::new(block_memory),
        );
        let state_handle = state.spawn();
        let request = example_request();

        let expected_response: serde_json::Value = serde_json::from_str(r#"
        {
            "number": 0,
            "hash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d", 
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "nonce": 0,
            "sha3Uncles": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "transactionsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "miner": "0x0000000000000000000000000000000000000000",
            "difficulty": 0,
            "totalDifficulty": 0,
            "extraData": "0x",
            "size": 0,
            "gasLimit": 0,
            "gasUsed": 0,
            "timestamp": 0,
            "transactions": [],
            "uncles": []
        }"#).unwrap();

        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
