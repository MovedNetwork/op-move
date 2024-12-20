use {
    crate::{
        json_utils::{self, access_state_error},
        jsonrpc::JsonRpcError,
        schema::{GetPayloadResponseV3, PayloadId},
    },
    moved::types::state::{Command, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute_v3(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let payload_id = parse_params_v3(request)?;
    let response = inner_execute_v3(payload_id, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params_v3(request: serde_json::Value) -> Result<PayloadId, JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [x] => {
            let id: PayloadId = json_utils::deserialize(x)?;
            Ok(id)
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute_v3(
    payload_id: PayloadId,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<GetPayloadResponseV3, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-2

    let (tx, rx) = oneshot::channel();
    let msg = Command::GetPayload {
        id: payload_id.clone().into(),
        response_channel: tx,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let maybe_response = rx.await.map_err(access_state_error)?.map(Into::into);

    maybe_response.ok_or_else(|| JsonRpcError {
        code: -38001,
        data: serde_json::to_value(payload_id).expect("Must serialize payload id"),
        message: "Unknown payload".into(),
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::forkchoice_updated,
        alloy::primitives::hex,
        moved::{
            block::{
                Block, BlockMemory, BlockRepository, Eip1559GasFee, InMemoryBlockQueries,
                InMemoryBlockRepository,
            },
            genesis::{self, config::GenesisConfig},
            primitives::{B256, U256},
            state_actor::InMemoryStateQueries,
            storage::InMemoryState,
            types::state::Command,
        },
    };

    #[test]
    fn test_parse_params_v3() {
        let request: serde_json::Value = serde_json::from_str(
            r#"
            {
                "id": 30054,
                "jsonrpc": "2.0",
                "method": "engine_getPayloadV3",
                "params": [
                    "0x03421ee50df45cac"
                ]
            }
        "#,
        )
        .unwrap();

        let params = parse_params_v3(request).unwrap();

        let expected_params = PayloadId::from(0x03421ee50df45cacu64);

        assert_eq!(params, expected_params);
    }

    #[tokio::test]
    async fn test_execute_v3() {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        // Set known block height
        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut block_memory = BlockMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut block_memory, genesis_block);

        let mut state = InMemoryState::new();
        let (changes, table_changes) = genesis::init(&genesis_config, &state);
        genesis::apply(changes.clone(), table_changes, &genesis_config, &mut state);

        let state = moved::state_actor::StateActor::new(
            rx,
            state,
            head_hash,
            0,
            genesis_config,
            0x03421ee50df45cacu64,
            head_hash,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            (),
            InMemoryBlockQueries,
            block_memory,
            InMemoryStateQueries::from_genesis(changes),
            moved::state_actor::StateActor::on_tx_noop(),
            moved::state_actor::StateActor::on_tx_batch_noop(),
        );
        let state_handle = state.spawn();

        // Set head block hash
        let msg = Command::UpdateHead {
            block_hash: head_hash,
        }
        .into();
        state_channel.send(msg).await.unwrap();

        // Update the state with an execution payload
        forkchoice_updated::execute_v3(
            forkchoice_updated::tests::example_request(),
            state_channel.clone(),
        )
        .await
        .unwrap();

        let request: serde_json::Value = serde_json::from_str(
            r#"
            {
                "id": 30054,
                "jsonrpc": "2.0",
                "method": "engine_getPayloadV3",
                "params": [
                    "0x03421ee50df45cac"
                ]
            }
        "#,
        )
        .unwrap();

        let expected_response: serde_json::Value = serde_json::from_str(r#"
            {
                "executionPayload": {
                    "parentHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    "feeRecipient": "0x4200000000000000000000000000000000000011",
                    "stateRoot": "0xc4a20322cea334322fbaf17d529ca7b9078ff0bee4783857afcf545fc093cd36",
                    "receiptsRoot": "0x05fddc251b06be4b6305c662f0ffa27284ed876b673b02cbc2f6cab452bd8bb6",
                    "logsBloom": "0x00000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000400",
                    "prevRandao": "0xbde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9",
                    "blockNumber": "0x1",
                    "gasLimit": "0x1c9c380",
                    "gasUsed": "0x8",
                    "timestamp": "0x6660737b",
                    "extraData": "0x",
                    "baseFeePerGas": "0x0",
                    "blockHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    "transactions": [
                    "0x7ef8f8a0de86bef815fc910df65a9459ccb2b9a35fa8596dfcfed1ff01bbf28891d86d5e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000000000000006660735b00000000000001a9000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000017ae3f74f0134521a7d62a387ac75a5153bcd1aab1c7e003e9b9e15a5d8846363000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425",
                    "0x7ef858a000000000000000000000000000000000000000000000000000000000000000009488f9b82462f6c4bf4a0fb15e5c3971559a316e7f9488f9b82462f6c4bf4a0fb15e5c3971559a316e7f807b88ffffffffffffffff8080"
                    ],
                    "withdrawals": [],
                    "blobGasUsed": "0x0",
                    "excessBlobGas": "0x0"
                },
                "blockValue": "0x0",
                "blobsBundle": {
                    "commitments": [],
                    "proofs": [],
                    "blobs": []
                },
                "shouldOverrideBuilder": false,
                "parentBeaconBlockRoot": "0x2bd857e239f7e5b5e6415608c76b90600d51fa0f7f0bbbc04e2d6861b3186f1c"
            }
        "#).unwrap();

        let response = execute_v3(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
