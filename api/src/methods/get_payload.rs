use {
    crate::{
        json_utils::parse_params_1,
        jsonrpc::JsonRpcError,
        schema::{GetPayloadResponseV3, PayloadId},
    },
    umi_app::{ApplicationReader, Dependencies},
    umi_blockchain::payload::MaybePayloadResponse,
};

pub async fn execute_v3<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let payload_id: PayloadId = parse_params_1(request)?;

    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-2
    let response = match app.payload(payload_id.into())? {
        MaybePayloadResponse::Some(response) => response,
        MaybePayloadResponse::Delayed(mut rx) => {
            if let Ok(response) = rx.recv().await {
                response
            } else {
                return Err(JsonRpcError::unknown_payload(payload_id));
            }
        }
        MaybePayloadResponse::Unknown => {
            return Err(JsonRpcError::unknown_payload(payload_id));
        }
    };

    Ok(serde_json::to_value(GetPayloadResponseV3::from(response))
        .expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::forkchoice_updated,
        alloy::primitives::hex,
        std::sync::Arc,
        umi_app::{Application, CommandActor, HybridBlockHashCache, TestDependencies},
        umi_blockchain::{
            block::{
                Block, BlockRepository, Eip1559GasFee, InMemoryBlockQueries,
                InMemoryBlockRepository, UmiBlockHash,
            },
            in_memory::{SharedMemoryReader, shared_memory},
            payload::{InMemoryPayloadQueries, InProgressPayloads},
            receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, receipt_memory},
            state::InMemoryStateQueries,
            transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
        },
        umi_evm_ext::state::{BlockHashWriter, InMemoryStorageTrieRepository},
        umi_genesis::config::GenesisConfig,
        umi_shared::primitives::{B256, U256},
        umi_state::{InMemoryState, InMemoryTrieDb},
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
        let genesis_block = Block::default()
            .into_extended_with_hash(head_hash)
            .with_value(U256::ZERO);

        let (memory_reader, mut memory) = shared_memory::new();
        let mut block_hash_cache =
            HybridBlockHashCache::new(memory_reader.clone(), InMemoryBlockQueries);
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut memory, genesis_block).unwrap();
        block_hash_cache.push(0, head_hash);

        let trie_db = Arc::new(InMemoryTrieDb::empty());
        let mut state = InMemoryState::empty(trie_db.clone());
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let (changes, evm_storage_changes) = umi_genesis_image::load();
        umi_genesis::apply(
            changes.clone(),
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );
        let (receipt_memory_reader, receipt_memory) = receipt_memory::new();
        let genesis_state_root = genesis_config.initial_state_root;
        let in_progress_payloads = InProgressPayloads::default();

        let mut app = Application::<TestDependencies<_, _, _, _>> {
            mem_pool: Default::default(),
            genesis_config: genesis_config.clone(),
            state,
            block_hash: head_hash,
            block_hash_lookup: block_hash_cache.clone(),
            block_hash_writer: block_hash_cache.clone(),
            block_repository: repository,
            gas_fee: Eip1559GasFee::default(),
            base_token: (),
            l1_fee: U256::ZERO,
            l2_fee: U256::ZERO,
            block_queries: InMemoryBlockQueries,
            storage: memory,
            receipt_memory_reader: receipt_memory_reader.clone(),
            storage_reader: memory_reader.clone(),
            state_queries: InMemoryStateQueries::new(
                memory_reader.clone(),
                trie_db.clone(),
                genesis_state_root,
            ),
            transaction_repository: InMemoryTransactionRepository::new(),
            transaction_queries: InMemoryTransactionQueries::new(),
            receipt_memory,
            receipt_repository: InMemoryReceiptRepository::new(),
            receipt_queries: InMemoryReceiptQueries::new(),
            payload_queries: InMemoryPayloadQueries::new(in_progress_payloads.clone()),
            evm_storage: evm_storage.clone(),
            on_tx: CommandActor::on_tx_noop(),
            on_tx_batch: CommandActor::on_tx_batch_noop(),
            on_payload: CommandActor::on_payload_in_memory(),
            resolver_cache: Default::default(),
        };
        let reader = ApplicationReader::<
            TestDependencies<
                _,
                InMemoryState,
                (),
                UmiBlockHash,
                _,
                InMemoryBlockRepository,
                _,
                _,
                InMemoryReceiptRepository,
                _,
                _,
                _,
                _,
                HybridBlockHashCache<SharedMemoryReader, InMemoryBlockQueries>,
                HybridBlockHashCache<SharedMemoryReader, InMemoryBlockQueries>,
                _,
                _,
                InMemoryTransactionRepository,
                Eip1559GasFee,
                U256,
                U256,
            >,
        > {
            genesis_config,
            base_token: (),
            block_hash_lookup: block_hash_cache,
            block_queries: InMemoryBlockQueries,
            storage: memory_reader.clone(),
            state_queries: InMemoryStateQueries::new(memory_reader, trie_db, genesis_state_root),
            transaction_queries: InMemoryTransactionQueries::new(),
            receipt_memory: receipt_memory_reader,
            receipt_queries: InMemoryReceiptQueries::new(),
            payload_queries: InMemoryPayloadQueries::new(in_progress_payloads),
            evm_storage,
        };
        let (queue, state) = umi_app::create(&mut app, 10);

        umi_app::run_with_actor(state, async move {
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
                        "stateRoot": "0x7e53bf5c6f519dc1bfb5db267a82ecad319446c9820f0df44cc676af68fb408b",
                        "receiptsRoot": "0xe50ad6079418eb8a970970d47b82f83976ed19177e538317ae4aec3c765698aa",
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

            queue.wait_for_pending_commands().await;

            let actual_response = execute_v3(request, &reader).await.unwrap();

            assert_eq!(actual_response, expected_response);
        }).await;
    }
}
