use {
    crate::{
        json_utils::parse_params_1,
        jsonrpc::JsonRpcError,
        schema::{GetPayloadResponseV3, PayloadId},
    },
    moved_app::{Application, Dependencies},
    std::sync::Arc,
    tokio::sync::RwLock,
};

pub async fn execute_v3(
    request: serde_json::Value,
    app: &Arc<RwLock<Application<impl Dependencies>>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let payload_id: PayloadId = parse_params_1(request)?;

    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-2
    let response = app
        .read()
        .await
        .payload(payload_id.clone().into())
        .map(GetPayloadResponseV3::from)
        .ok_or_else(|| JsonRpcError {
            code: -38001,
            data: serde_json::to_value(payload_id).expect("Must serialize payload id"),
            message: "Unknown payload".into(),
        })?;

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::forkchoice_updated,
        alloy::primitives::hex,
        moved_app::{Application, Command, CommandActor, TestDependencies},
        moved_blockchain::{
            block::{
                Block, BlockRepository, Eip1559GasFee, InMemoryBlockQueries,
                InMemoryBlockRepository,
            },
            in_memory::SharedMemory,
            payload::InMemoryPayloadQueries,
            receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, ReceiptMemory},
            state::InMemoryStateQueries,
            transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
        },
        moved_evm_ext::state::InMemoryStorageTrieRepository,
        moved_genesis::config::GenesisConfig,
        moved_shared::primitives::{B256, U256},
        moved_state::InMemoryState,
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

        let params: PayloadId = parse_params_1(request).unwrap();

        let expected_params = PayloadId::from(0x03421ee50df45cacu64);

        assert_eq!(params, expected_params);
    }

    #[tokio::test]
    async fn test_execute_v3() {
        let genesis_config = GenesisConfig::default();

        // Set known block height
        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut memory = SharedMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut memory, genesis_block).unwrap();

        let mut state = InMemoryState::new();
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let (changes, table_changes, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes.clone(),
            table_changes,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );
        let initial_state_root = genesis_config.initial_state_root;

        let app = Arc::new(RwLock::new(Application::<TestDependencies<_, _, _, _>> {
            mem_pool: Default::default(),
            genesis_config,
            head: head_hash,
            height: 0,
            state,
            block_hash: head_hash,
            block_repository: repository,
            gas_fee: Eip1559GasFee::default(),
            base_token: (),
            l1_fee: U256::ZERO,
            l2_fee: U256::ZERO,
            block_queries: InMemoryBlockQueries,
            storage: memory,
            state_queries: InMemoryStateQueries::from_genesis(initial_state_root),
            transaction_repository: InMemoryTransactionRepository::new(),
            transaction_queries: InMemoryTransactionQueries::new(),
            receipt_memory: ReceiptMemory::new(),
            receipt_repository: InMemoryReceiptRepository::new(),
            receipt_queries: InMemoryReceiptQueries::new(),
            payload_queries: InMemoryPayloadQueries::new(),
            evm_storage,
            on_tx: CommandActor::on_tx_noop(),
            on_tx_batch: CommandActor::on_tx_batch_noop(),
            on_payload: CommandActor::on_payload_in_memory(),
        }));
        let (queue, state) = moved_app::create(app.clone(), 10);
        let state_handle = state.spawn();

        // Set head block hash
        let msg = Command::UpdateHead {
            block_hash: head_hash,
        };
        queue.send(msg).await;

        // Update the state with an execution payload
        forkchoice_updated::execute_v3(
            forkchoice_updated::tests::example_request(),
            queue.clone(),
            &0x03421ee50df45cacu64,
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
                    "stateRoot": "0x0611dad6081ad1467ba59fb1bb84733ed862faf002c50728bf3a9999c19e6fd6",
                    "receiptsRoot": "0x5e632f22d10d3a1312d54fd4d5b65d6a17d9a0b2b3cd3459b422668764c8a620",
                    "logsBloom": "0x00000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000008000000000000000000000000000000000000000000008000000000000000000000000000000000400000000000001000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000400",
                    "prevRandao": "0xbde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9",
                    "blockNumber": "0x1",
                    "gasLimit": "0x1c9c380",
                    "gasUsed": "0x25a38",
                    "timestamp": "0x6660737b",
                    "extraData": "0x",
                    "baseFeePerGas": "0x0",
                    "blockHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    "transactions": [
                    "0x7ef8f8a0de86bef815fc910df65a9459ccb2b9a35fa8596dfcfed1ff01bbf28891d86d5e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000000000000006660735b00000000000001a9000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000017ae3f74f0134521a7d62a387ac75a5153bcd1aab1c7e003e9b9e15a5d8846363000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425",
                    "0x7ef858a000000000000000000000000000000000000000000000000000000000000000009488f9b82462f6c4bf4a0fb15e5c3971559a316e7f9488f9b82462f6c4bf4a0fb15e5c3971559a316e7f7b7b88ffffffffffffffff8080"
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

        drop(queue);
        state_handle.await.unwrap();
        let actual_response = execute_v3(request, &app).await.unwrap();

        assert_eq!(actual_response, expected_response);
    }
}
