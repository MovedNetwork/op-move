use {
    crate::{
        json_utils::{self, access_state_error},
        types::{
            engine_api::{ExecutionPayloadV3, GetPayloadResponseV3, PayloadStatusV1, Status},
            jsonrpc::JsonRpcError,
            state::StateMessage,
        },
    },
    ethers_core::types::H256,
    tokio::sync::{mpsc, oneshot},
};

#[cfg(test)]
use {
    crate::{
        methods::{forkchoice_updated, get_payload},
        types::engine_api::PayloadId,
    },
    ethers_core::types::{Bytes, H160, U256, U64},
    std::str::FromStr,
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
) -> Result<(ExecutionPayloadV3, Vec<H256>, H256), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] | [_] | [_, _] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [x, y, z] => {
            let execution_payload: ExecutionPayloadV3 = json_utils::deserialize(x)?;
            let expected_blob_versioned_hashes: Vec<H256> = json_utils::deserialize(y)?;
            let parent_beacon_block_root: H256 = json_utils::deserialize(z)?;
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
    expected_blob_versioned_hashes: Vec<H256>,
    parent_beacon_block_root: H256,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<PayloadStatusV1, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification

    let (tx, rx) = oneshot::channel();
    let msg = StateMessage::GetPayloadByBlockHash {
        block_hash: execution_payload.block_hash,
        response_channel: tx,
    };
    state_channel.send(msg).await.map_err(access_state_error)?;
    let maybe_response = rx.await.map_err(access_state_error)?;

    // TODO: in theory we should start syncing to learn about this block hash.
    let response = maybe_response.ok_or(JsonRpcError {
        code: -1,
        data: serde_json::to_value(execution_payload.block_hash)
            .expect("Must serialize block hash"),
        message: "Unknown block hash".into(),
    })?;

    validate_payload(
        execution_payload,
        expected_blob_versioned_hashes,
        parent_beacon_block_root,
        response,
    )
}

fn validate_payload(
    execution_payload: ExecutionPayloadV3,
    expected_blob_versioned_hashes: Vec<H256>,
    parent_beacon_block_root: H256,
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
    )
    .unwrap();

    let params = parse_params_v3(request).unwrap();

    let expected_params = (
        ExecutionPayloadV3 {
            parent_hash: H256::from_str("0x781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449").unwrap(),
            fee_recipient: H160::from_str("0x4200000000000000000000000000000000000011").unwrap(),
            state_root: H256::from_str("0x316850949fd480573fec2a2cb07c9c22d7f18a390d9ad4b6847a4326b1a4a5eb").unwrap(),
            receipts_root: H256::from_str("0x619a992b2d1905328560c3bd9c7fc79b57f012afbff3de92d7a82cfdf8aa186c").unwrap(),
            logs_bloom: vec![0; 256].into(),
            prev_randao: H256::from_str("0x5e52abb859f1fff3a4bf38e076b67815214e8cff662055549b91ba33f5cb7fba").unwrap(),
            block_number: U64::one(),
            gas_limit: U64::from_str("0x1c9c380").unwrap(),
            gas_used: U64::from_str("0x2728a").unwrap(),
            timestamp: U64::from_str("0x666c9d8d").unwrap(),
            extra_data: Vec::new().into(),
            base_fee_per_gas: U256::from_str("0x3b5dc100").unwrap(),
            block_hash: H256::from_str("0xc013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3").unwrap(),
            transactions: vec![
                Bytes::from_str("0x7ef8f8a0d449f5de7f558fa593dce80637d3a3f52cfaaee2913167371dd6ffd9014e431d94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e20000f424000000000000000000000000100000000666c9d8b0000000000000028000000000000000000000000000000000000000000000000000000000049165f0000000000000000000000000000000000000000000000000000000000000001d05450763214e6060d285b39ef5fe51ef9526395e5cef6ecb27ba06f9598f27d000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425").unwrap()
            ],
            withdrawals: Vec::new(),
            blob_gas_used: U64::zero(),
            excess_blob_gas: U64::zero(),
        },
        Vec::new(),
        H256::from_str("0x1a274bb1e783ec35804dee78ec3d7cecd03371f311b2f946500613e994f024a5").unwrap()
    );

    assert_eq!(params, expected_params);
}

#[tokio::test]
async fn test_execute_v3() {
    let (state_channel, rx) = tokio::sync::mpsc::channel(10);
    let state = crate::state_actor::StateActor::new_in_memory(rx);
    let state_handle = state.spawn();

    // Set payload id
    let msg = StateMessage::SetPayloadId {
        id: PayloadId::from_str("0x0306d51fc5aa1533").unwrap(),
    };
    state_channel.send(msg).await.unwrap();

    // Set known block height
    let head_hash =
        H256::from_str("0x781f09c5b7629a7ca30668e440ea40557f01461ad6f105b371f61ff5824b2449")
            .unwrap();
    let msg = StateMessage::NewBlock {
        block_hash: head_hash,
        block_height: U64::zero(),
    };
    state_channel.send(msg).await.unwrap();

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

    let msg = StateMessage::NewBlock {
        block_hash: H256::from_str(
            "0xc013e1ff1b8bca9f0d074618cc9e661983bc91d7677168b156765781aee775d3",
        )
        .unwrap(),
        block_height: U64::one(),
    };
    state_channel.send(msg).await.unwrap();

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