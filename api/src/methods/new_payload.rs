use {
    crate::{
        json_utils::parse_params_3,
        jsonrpc::JsonRpcError,
        schema::{ExecutionPayloadV3, GetPayloadResponseV3, PayloadStatusV1, Status},
    },
    moved_app::{ApplicationReader, Dependencies},
    moved_shared::primitives::B256,
};

pub async fn execute_v3(
    request: serde_json::Value,
    app: ApplicationReader<impl Dependencies>,
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

async fn inner_execute_v3(
    execution_payload: ExecutionPayloadV3,
    expected_blob_versioned_hashes: Vec<B256>,
    parent_beacon_block_root: B256,
    app: ApplicationReader<impl Dependencies>,
) -> Result<PayloadStatusV1, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification

    // TODO: in theory we should start syncing to learn about this block hash.
    let response = app
        .payload_by_block_hash(execution_payload.block_hash)
        .ok_or(JsonRpcError {
            code: -1,
            data: serde_json::to_value(execution_payload.block_hash)
                .expect("Must serialize block hash"),
            message: "Unknown block hash".into(),
        })?
        .into();

    validate_payload(
        execution_payload,
        expected_blob_versioned_hashes,
        parent_beacon_block_root,
        response,
    )
}

fn validate_payload(
    execution_payload: ExecutionPayloadV3,
    expected_blob_versioned_hashes: Vec<B256>,
    parent_beacon_block_root: B256,
    known_payload: GetPayloadResponseV3,
) -> Result<PayloadStatusV1, JsonRpcError> {
    if execution_payload.block_number != known_payload.execution_payload.block_number {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect block height".into()),
        });
    }

    if execution_payload.extra_data != known_payload.execution_payload.extra_data {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect extra data".into()),
        });
    }

    if execution_payload.fee_recipient != known_payload.execution_payload.fee_recipient {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect fee recipient".into()),
        });
    }

    if execution_payload.gas_limit != known_payload.execution_payload.gas_limit {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect gas limit".into()),
        });
    }

    if execution_payload.parent_hash != known_payload.execution_payload.parent_hash {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect parent hash".into()),
        });
    }

    if execution_payload.prev_randao != known_payload.execution_payload.prev_randao {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect prev randao".into()),
        });
    }

    if execution_payload.timestamp != known_payload.execution_payload.timestamp {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect timestamp".into()),
        });
    }

    if execution_payload.withdrawals != known_payload.execution_payload.withdrawals {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect withdraws".into()),
        });
    }

    // TODO: validate execution relates fields once op-geth no longer used
    // base_fee_per_gas, gas_used, logs_bool, receipts_root, state_root, transactions

    // TODO: Support blobs (low priority).
    if !expected_blob_versioned_hashes.is_empty() {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Unexpected blob hashes".into()),
        });
    }

    if parent_beacon_block_root != known_payload.parent_beacon_block_root {
        return Ok(PayloadStatusV1 {
            status: Status::Invalid,
            latest_valid_hash: None,
            validation_error: Some("Incorrect parent beacon block root".into()),
        });
    }

    Ok(PayloadStatusV1 {
        status: Status::Valid,
        latest_valid_hash: Some(execution_payload.block_hash),
        validation_error: None,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::{forkchoice_updated, get_payload},
        alloy::primitives::hex,
        moved_app::{Application, CommandActor, TestDependencies},
        moved_blockchain::{
            block::{
                Block, BlockRepository, Eip1559GasFee, InMemoryBlockQueries,
                InMemoryBlockRepository, MovedBlockHash,
            },
            in_memory::shared_memory,
            payload::InMemoryPayloadQueries,
            receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, receipt_memory},
            state::InMemoryStateQueries,
            transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
        },
        moved_evm_ext::state::InMemoryStorageTrieRepository,
        moved_genesis::config::GenesisConfig,
        moved_shared::primitives::{Address, B2048, Bytes, U64, U256},
        moved_state::InMemoryState,
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
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let (memory_reader, mut memory) = shared_memory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut memory, genesis_block).unwrap();

        let trie_db = InMemoryState::create_db();
        let mut state = InMemoryState::new(trie_db.clone());
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
        let (receipt_memory_reader, receipt_memory) = receipt_memory::new();

        let app = Application::<TestDependencies<_, _, _, _>> {
            mem_pool: Default::default(),
            genesis_config: genesis_config.clone(),
            gas_fee: Eip1559GasFee::default(),
            base_token: (),
            l1_fee: U256::ZERO,
            l2_fee: U256::ZERO,
            block_hash: B256::from(hex!(
                "c013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3"
            )),
            block_queries: InMemoryBlockQueries,
            block_repository: repository,
            on_payload: CommandActor::on_payload_in_memory(),
            on_tx: CommandActor::on_tx_noop(),
            on_tx_batch: CommandActor::on_tx_batch_noop(),
            payload_queries: InMemoryPayloadQueries::new(),
            receipt_queries: InMemoryReceiptQueries::new(),
            receipt_repository: InMemoryReceiptRepository::new(),
            receipt_memory,
            storage: memory,
            receipt_memory_reader: receipt_memory_reader.clone(),
            storage_reader: memory_reader.clone(),
            state,
            evm_storage: evm_storage.clone(),
            transaction_queries: InMemoryTransactionQueries::new(),
            state_queries: InMemoryStateQueries::new(memory_reader.clone(), trie_db.clone()),
            transaction_repository: InMemoryTransactionRepository::new(),
        };
        let reader = ApplicationReader::<
            TestDependencies<
                _,
                InMemoryState,
                _,
                MovedBlockHash,
                _,
                (),
                _,
                _,
                (),
                _,
                _,
                _,
                _,
                _,
                _,
                (),
                Eip1559GasFee,
                U256,
                U256,
            >,
        > {
            genesis_config,
            base_token: (),
            block_queries: InMemoryBlockQueries,
            storage: memory_reader.clone(),
            state_queries: InMemoryStateQueries::new(memory_reader, trie_db),
            transaction_queries: InMemoryTransactionQueries::new(),
            receipt_memory: receipt_memory_reader,
            receipt_queries: InMemoryReceiptQueries::new(),
            payload_queries: InMemoryPayloadQueries::new(),
            evm_storage,
        };
        let (queue, state) = moved_app::create(Box::new(app), 10);
        let state_handle = state.spawn();

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
        let new_payload_request: serde_json::Value = serde_json::from_str(
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
        )
        .unwrap();

        forkchoice_updated::execute_v3(fc_updated_request, queue.clone(), &0x0306d51fc5aa1533u64)
            .await
            .unwrap();

        queue.wait_for_pending_commands().await;

        get_payload::execute_v3(get_payload_request, reader.clone())
            .await
            .unwrap();

        let response = execute_v3(new_payload_request, reader.clone())
            .await
            .unwrap();

        let expected_response: serde_json::Value = serde_json::from_str(
            r#"
            {
                "status": "VALID",
                "latestValidHash": "0xc013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3",
                "validationError": null
            }
            "#,
        )
        .unwrap();

        assert_eq!(response, expected_response);

        drop(queue);
        state_handle.await.unwrap();
    }
}
