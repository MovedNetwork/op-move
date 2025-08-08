use {
    crate::{
        json_utils::parse_params_3,
        jsonrpc::JsonRpcError,
        schema::{ExecutionPayloadV3, GetPayloadResponseV3, PayloadStatusV1, Status},
    },
    alloy::{
        consensus::{EMPTY_OMMER_ROOT_HASH, Header, constants::EMPTY_WITHDRAWALS},
        primitives::{B64, Bloom, U64, U256},
    },
    umi_app::{ApplicationReader, Dependencies},
    umi_shared::primitives::B256,
};

pub async fn execute_v3<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (execution_payload, expected_blob_versioned_hashes, parent_beacon_block_root) =
        parse_params_3(request)?;
    let response = inner_execute_v3(
        execution_payload,
        expected_blob_versioned_hashes,
        parent_beacon_block_root,
        app,
    )
    .await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

async fn inner_execute_v3<'reader>(
    execution_payload: ExecutionPayloadV3,
    expected_blob_versioned_hashes: Vec<B256>,
    parent_beacon_block_root: B256,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<PayloadStatusV1, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification

    if let Err(status) = validate_payload_block_hash(&execution_payload, parent_beacon_block_root) {
        return Ok(status);
    }

    // TODO: we're always assuming here that this RPC call can only bring us already known payloads
    // as we're operating in single-node mode. However, even in that case unknown payloads could
    // come in if an L1 reorg or op-move restart happen. In that case, a sync should be triggered.
    let response = app
        .payload_by_block_hash(execution_payload.block_hash)
        .map(GetPayloadResponseV3::from)?;

    if let Err(status) = validate_payload_format(&execution_payload, expected_blob_versioned_hashes)
        .and_then(|_| {
            validate_known_payload(&execution_payload, parent_beacon_block_root, response)
        })
    {
        return Ok(status);
    };
    Ok(PayloadStatusV1 {
        status: Status::Valid,
        latest_valid_hash: Some(execution_payload.block_hash),
        validation_error: None,
    })
}

fn validate_payload_format(
    execution_payload: &ExecutionPayloadV3,
    expected_blob_versioned_hashes: Vec<B256>,
) -> Result<(), PayloadStatusV1> {
    // Anything EIP-4844 related is disabled in Optimism (<https://specs.optimism.io/protocol/exec-engine.html#ecotone-disable-blob-transactions>)
    // and Umi, as we have chosen to reject this tx kind at the API border. Thus for v3 engine API
    // all the values should be 0.
    if execution_payload.excess_blob_gas != U64::ZERO
        || execution_payload.blob_gas_used != U64::ZERO
    {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Unexpected non-zero blob gas fields".into()),
        });
    }
    if !expected_blob_versioned_hashes.is_empty() {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Unexpected non-empty blob hashes".into()),
        });
    }

    if execution_payload.transactions.iter().any(|s| s.len() == 0) {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("All transactions should be non-zero length".into()),
        });
    }

    // TODO: (#201) should contain eip1559params post OP stack Holocene upgrade
    if execution_payload.block_number != U64::ZERO && !execution_payload.extra_data.is_empty() {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Pre-holocene payload extraData should be empty".into()),
        });
    }

    Ok(())
}

fn validate_payload_block_hash(
    execution_payload: &ExecutionPayloadV3,
    parent_beacon_block_root: B256,
) -> Result<(), PayloadStatusV1> {
    let transactions_root = alloy_trie::root::ordered_trie_root(&execution_payload.transactions);

    let payload_header = Header {
        parent_hash: execution_payload.parent_hash,
        ommers_hash: EMPTY_OMMER_ROOT_HASH,
        beneficiary: execution_payload.fee_recipient,
        state_root: execution_payload.state_root,
        transactions_root,
        receipts_root: execution_payload.receipts_root,
        logs_bloom: Bloom::new(execution_payload.logs_bloom.into()),
        difficulty: U256::ZERO,
        number: execution_payload.block_number.saturating_to(),
        gas_limit: execution_payload.gas_limit.saturating_to(),
        gas_used: execution_payload.gas_used.saturating_to(),
        timestamp: execution_payload.timestamp.saturating_to(),
        extra_data: execution_payload.extra_data.clone(),
        mix_hash: execution_payload.prev_randao,
        nonce: B64::ZERO,
        base_fee_per_gas: Some(execution_payload.base_fee_per_gas.saturating_to()),
        withdrawals_root: Some(EMPTY_WITHDRAWALS),
        blob_gas_used: Some(execution_payload.blob_gas_used.saturating_to()),
        excess_blob_gas: Some(execution_payload.excess_blob_gas.saturating_to()),
        parent_beacon_block_root: Some(parent_beacon_block_root),
        requests_hash: None,
    };
    let computed_hash = alloy::primitives::keccak256(alloy::rlp::encode(&payload_header));

    if computed_hash != execution_payload.block_hash {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some(format!(
                "Received payload hash {} and computed hash {} don't match",
                execution_payload.block_hash, computed_hash
            )),
        });
    }
    Ok(())
}

fn validate_known_payload(
    execution_payload: &ExecutionPayloadV3,
    parent_beacon_block_root: B256,
    known_payload: GetPayloadResponseV3,
) -> Result<(), PayloadStatusV1> {
    if execution_payload.block_number != known_payload.execution_payload.block_number {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect block height".into()),
        });
    }

    if execution_payload.extra_data != known_payload.execution_payload.extra_data {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect extra data".into()),
        });
    }

    if execution_payload.fee_recipient != known_payload.execution_payload.fee_recipient {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect fee recipient".into()),
        });
    }

    if execution_payload.gas_limit != known_payload.execution_payload.gas_limit {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect gas limit".into()),
        });
    }

    if execution_payload.parent_hash != known_payload.execution_payload.parent_hash {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect parent hash".into()),
        });
    }

    if execution_payload.prev_randao != known_payload.execution_payload.prev_randao {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect prev randao".into()),
        });
    }

    if execution_payload.timestamp != known_payload.execution_payload.timestamp {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect timestamp".into()),
        });
    }

    if execution_payload.withdrawals != known_payload.execution_payload.withdrawals {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Withdrawals mismatch".into()),
        });
    }

    if parent_beacon_block_root != known_payload.parent_beacon_block_root {
        return Err(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect parent beacon block root".into()),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::{forkchoice_updated, get_payload},
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
        umi_shared::primitives::{Address, B2048, Bytes, U64, U256},
        umi_state::{InMemoryState, InMemoryTrieDb},
    };

    #[test]
    fn test_parse_params_v3() {
        let request: serde_json::Value = serde_json::from_str(
            r#"
            {
                "jsonrpc": "2.0",
                "id": 9,
                "method": "engine_newPayloadV3",
                "params": [
                {
                    "parentHash": "0x781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449",
                    "feeRecipient": "0x4200000000000000000000000000000000000011",
                    "stateRoot": "0x316850949fd480573fec2a2cb07c9c22d7f18a390d9ad4b6847a4326b1a4a5eb",
                    "receiptsRoot": "0x619a992b2d1905328560c3bd9c7fc79b57f012afbff3de92d7a82cfdf8aa186c",
                    "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                    "prevRandao": "0x5e52abb859f1fff3a4bf38e076b67815214e8cff662055549b91ba33f5cb7fba",
                    "blockNumber": "0x1",
                    "gasLimit": "0x1c9c380",
                    "gasUsed": "0x2728a",
                    "timestamp": "0x666c9d8d",
                    "extraData": "0x",
                    "baseFeePerGas": "0x3b5dc100",
                    "blockHash": "0xc013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3",
                    "transactions": [
                    "0x7ef8f8a0d449f5de7f558fa593dce80637d3a3f52cfaaee2913167371dd6ffd9014e431d94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e20000f424000000000000000000000000100000000666c9d8b0000000000000028000000000000000000000000000000000000000000000000000000000049165f0000000000000000000000000000000000000000000000000000000000000001d05450763214e6060d285b39ef5fe51ef9526395e5cef6ecb27ba06f9598f27d000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"
                    ],
                    "withdrawals": [],
                    "blobGasUsed": "0x0",
                    "excessBlobGas": "0x0"
                },
                [],
                "0x1a274bb1e783ec35804dee78ec3d7cecd03371f311b2f946500613e994f024a5"
                ]
            }
        "#,
        ).unwrap();

        let params: (ExecutionPayloadV3, Vec<B256>, B256) = parse_params_3(request).unwrap();

        let expected_params = (
            ExecutionPayloadV3 {
                parent_hash: B256::new(hex!(
                    "781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449"
                )),
                fee_recipient: Address::new(hex!("4200000000000000000000000000000000000011")),
                state_root: B256::new(hex!(
                    "316850949fd480573fec2a2cb07c9c22d7f18a390d9ad4b6847a4326b1a4a5eb"
                )),
                receipts_root: B256::new(hex!(
                    "619a992b2d1905328560c3bd9c7fc79b57f012afbff3de92d7a82cfdf8aa186c"
                )),
                logs_bloom: B2048::ZERO,
                prev_randao: B256::new(hex!(
                    "5e52abb859f1fff3a4bf38e076b67815214e8cff662055549b91ba33f5cb7fba"
                )),
                block_number: U64::from_be_slice(&hex!("01")),
                gas_limit: U64::from_be_slice(&hex!("01c9c380")),
                gas_used: U64::from_be_slice(&hex!("02728a")),
                timestamp: U64::from_be_slice(&hex!("666c9d8d")),
                extra_data: Vec::new().into(),
                base_fee_per_gas: U256::from_be_slice(&hex!("3b5dc100")),
                block_hash: B256::new(hex!(
                    "c013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3"
                )),
                transactions: vec![Bytes::from_static(&hex!(
                    "7ef8f8a0d449f5de7f558fa593dce80637d3a3f52cfaaee2913167371dd6ffd9014e431d94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e20000f424000000000000000000000000100000000666c9d8b0000000000000028000000000000000000000000000000000000000000000000000000000049165f0000000000000000000000000000000000000000000000000000000000000001d05450763214e6060d285b39ef5fe51ef9526395e5cef6ecb27ba06f9598f27d000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"
                ))],
                withdrawals: Vec::new(),
                blob_gas_used: U64::ZERO,
                excess_blob_gas: U64::ZERO,
            },
            Vec::new(),
            B256::new(hex!(
                "1a274bb1e783ec35804dee78ec3d7cecd03371f311b2f946500613e994f024a5"
            )),
        );

        assert_eq!(params, expected_params);
    }

    #[tokio::test]
    async fn test_execute_v3() {
        let genesis_config = GenesisConfig::default();

        // Set known block height
        let head_hash = B256::new(hex!(
            "781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449"
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
            gas_fee: Eip1559GasFee::default(),
            base_token: (),
            l1_fee: U256::ZERO,
            l2_fee: U256::ZERO,
            block_hash: UmiBlockHash,
            block_hash_writer: block_hash_cache.clone(),
            block_hash_lookup: block_hash_cache.clone(),
            block_queries: InMemoryBlockQueries,
            block_repository: repository,
            on_payload: CommandActor::on_payload_in_memory(),
            on_tx: CommandActor::on_tx_noop(),
            on_tx_batch: CommandActor::on_tx_batch_noop(),
            payload_queries: InMemoryPayloadQueries::new(in_progress_payloads.clone()),
            receipt_queries: InMemoryReceiptQueries::new(),
            receipt_repository: InMemoryReceiptRepository::new(),
            receipt_memory,
            storage: memory,
            receipt_memory_reader: receipt_memory_reader.clone(),
            storage_reader: memory_reader.clone(),
            state,
            evm_storage: evm_storage.clone(),
            transaction_queries: InMemoryTransactionQueries::new(),
            state_queries: InMemoryStateQueries::new(
                memory_reader.clone(),
                trie_db.clone(),
                genesis_state_root,
            ),
            transaction_repository: InMemoryTransactionRepository::new(),
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
            let fc_updated_request: serde_json::Value = serde_json::from_str(
                r#"
                    {
                        "jsonrpc": "2.0",
                        "id": 7,
                        "method": "engine_forkchoiceUpdatedV3",
                        "params": [
                        {
                            "headBlockHash": "0x781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449",
                            "safeBlockHash": "0x781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449",
                            "finalizedBlockHash": "0x781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449"
                        },
                        {
                            "timestamp": "0x666c9d8d",
                            "prevRandao": "0x5e52abb859f1fff3a4bf38e076b67815214e8cff662055549b91ba33f5cb7fba",
                            "suggestedFeeRecipient": "0x4200000000000000000000000000000000000011",
                            "withdrawals": [],
                            "parentBeaconBlockRoot": "0x1a274bb1e783ec35804dee78ec3d7cecd03371f311b2f946500613e994f024a5",
                            "transactions": [
                            "0x7ef8f8a0d449f5de7f558fa593dce80637d3a3f52cfaaee2913167371dd6ffd9014e431d94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e20000f424000000000000000000000000100000000666c9d8b0000000000000028000000000000000000000000000000000000000000000000000000000049165f0000000000000000000000000000000000000000000000000000000000000001d05450763214e6060d285b39ef5fe51ef9526395e5cef6ecb27ba06f9598f27d000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"
                            ],
                            "gasLimit": "0x1c9c380"
                        }
                        ]
                    }
            "#,
            )
                .unwrap();
            let get_payload_request: serde_json::Value = serde_json::from_str(
                r#"
                    {
                        "jsonrpc": "2.0",
                        "id": 8,
                        "method": "engine_getPayloadV3",
                        "params": [
                            "0x0306d51fc5aa1533"
                        ]
                    }
            "#,
            )
                .unwrap();

            forkchoice_updated::execute_v3(
                fc_updated_request,
                queue.clone(),
                &reader,
                &0x0306d51fc5aa1533u64,
            )
                .await
                .unwrap();

            queue.wait_for_pending_commands().await;

            let get_payload_response: GetPayloadResponseV3 = serde_json::from_value(get_payload::execute_v3(get_payload_request, &reader)
                .await
                .unwrap()).unwrap();

            let valid_hash = get_payload_response.execution_payload.block_hash;

            let new_payload_request: serde_json::Value = serde_json::from_str(
                &format!(r#"
                   {{
                        "jsonrpc": "2.0",
                        "id": 9,
                        "method": "engine_newPayloadV3",
                        "params": [
                        {},
                        [],
                        "0x1a274bb1e783ec35804dee78ec3d7cecd03371f311b2f946500613e994f024a5"
                        ]
                    }}
            "#, serde_json::to_string(&get_payload_response.execution_payload).unwrap()),
            )
                .unwrap();
            let response = execute_v3(new_payload_request, &reader).await.unwrap();

            let expected_response: serde_json::Value = serde_json::from_str(
                &format!(r#"
                {{
                    "status": "VALID",
                    "latestValidHash": "{valid_hash}",
                    "validationError": null
                }}
                "#),
            )
                .unwrap();

            assert_eq!(response, expected_response);
        }).await;
    }
}
