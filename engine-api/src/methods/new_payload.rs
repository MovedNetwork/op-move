use {
    crate::{
        json_utils::{self, access_state_error},
        jsonrpc::JsonRpcError,
        schema::{ExecutionPayloadV3, GetPayloadResponseV3, PayloadStatusV1, Status},
    },
    moved::{
        primitives::B256,
        types::state::{Command, StateMessage},
    },
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute_v3(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (execution_payload, expected_blob_versioned_hashes, parent_beacon_block_root) =
        parse_params_v3(request)?;
    let response = inner_execute_v3(
        execution_payload,
        expected_blob_versioned_hashes,
        parent_beacon_block_root,
        state_channel,
    )
    .await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params_v3(
    request: serde_json::Value,
) -> Result<(ExecutionPayloadV3, Vec<B256>, B256), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] | [_] | [_, _] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [x, y, z] => {
            let execution_payload: ExecutionPayloadV3 = json_utils::deserialize(x)?;
            let expected_blob_versioned_hashes: Vec<B256> = json_utils::deserialize(y)?;
            let parent_beacon_block_root: B256 = json_utils::deserialize(z)?;
            Ok((
                execution_payload,
                expected_blob_versioned_hashes,
                parent_beacon_block_root,
            ))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute_v3(
    execution_payload: ExecutionPayloadV3,
    expected_blob_versioned_hashes: Vec<B256>,
    parent_beacon_block_root: B256,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<PayloadStatusV1, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification

    let (tx, rx) = oneshot::channel();
    let msg = Command::GetPayloadByBlockHash {
        block_hash: execution_payload.block_hash,
        response_channel: tx,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let maybe_response = rx.await.map_err(access_state_error)?;

    // TODO: in theory we should start syncing to learn about this block hash.
    let response = maybe_response
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
        moved::{
            block::{
                Block, BlockMemory, BlockRepository, Eip1559GasFee, InMemoryBlockQueries,
                InMemoryBlockRepository,
            },
            genesis::{self, config::GenesisConfig},
            move_execution::{InMemoryStateQueries, StateMemory},
            primitives::{Address, Bytes, B2048, U256, U64},
            storage::InMemoryState,
        },
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

        let params = parse_params_v3(request).unwrap();

        let expected_params = (
            ExecutionPayloadV3 {
                parent_hash: B256::new(hex!("781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449")),
                fee_recipient: Address::new(hex!("4200000000000000000000000000000000000011")),
                state_root: B256::new(hex!("316850949fd480573fec2a2cb07c9c22d7f18a390d9ad4b6847a4326b1a4a5eb")),
                receipts_root: B256::new(hex!("619a992b2d1905328560c3bd9c7fc79b57f012afbff3de92d7a82cfdf8aa186c")),
                logs_bloom: B2048::ZERO,
                prev_randao: B256::new(hex!("5e52abb859f1fff3a4bf38e076b67815214e8cff662055549b91ba33f5cb7fba")),
                block_number: U64::from_be_slice(&hex!("01")),
                gas_limit: U64::from_be_slice(&hex!("01c9c380")),
                gas_used: U64::from_be_slice(&hex!("02728a")),
                timestamp: U64::from_be_slice(&hex!("666c9d8d")),
                extra_data: Vec::new().into(),
                base_fee_per_gas: U256::from_be_slice(&hex!("3b5dc100")),
                block_hash: B256::new(hex!("c013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3")),
                transactions: vec![
                    Bytes::from_static(&hex!("7ef8f8a0d449f5de7f558fa593dce80637d3a3f52cfaaee2913167371dd6ffd9014e431d94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e20000f424000000000000000000000000100000000666c9d8b0000000000000028000000000000000000000000000000000000000000000000000000000049165f0000000000000000000000000000000000000000000000000000000000000001d05450763214e6060d285b39ef5fe51ef9526395e5cef6ecb27ba06f9598f27d000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"))
                ],
                withdrawals: Vec::new(),
                blob_gas_used: U64::ZERO,
                excess_blob_gas: U64::ZERO,
            },
            Vec::new(),
            B256::new(hex!("1a274bb1e783ec35804dee78ec3d7cecd03371f311b2f946500613e994f024a5"))
        );

        assert_eq!(params, expected_params);
    }

    #[tokio::test]
    async fn test_execute_v3() {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        // Set known block height
        let head_hash = B256::new(hex!(
            "781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut block_memory = BlockMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut block_memory, genesis_block);

        let mut state = InMemoryState::new();
        let (changes, table_changes) = genesis::init(&genesis_config, &state);
        let state_memory = StateMemory::from_genesis(changes.clone());
        genesis::apply(changes, table_changes, &genesis_config, &mut state);

        let state = moved::state_actor::StateActor::new(
            rx,
            state,
            head_hash,
            0,
            genesis_config,
            0x0306d51fc5aa1533u64,
            B256::from(hex!(
                "c013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3"
            )),
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            (),
            InMemoryBlockQueries,
            block_memory,
            InMemoryStateQueries::new(state_memory),
        );
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

        forkchoice_updated::execute_v3(fc_updated_request, state_channel.clone())
            .await
            .unwrap();

        get_payload::execute_v3(get_payload_request, state_channel.clone())
            .await
            .unwrap();

        let response = execute_v3(new_payload_request, state_channel)
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
        state_handle.await.unwrap();
    }
}
