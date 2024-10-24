use {
    crate::{
        json_utils::{self, access_state_error},
        types::{
            engine_api::{
                ForkchoiceStateV1, ForkchoiceUpdatedResponseV1, PayloadAttributesV3,
                PayloadStatusV1, Status,
            },
            jsonrpc::JsonRpcError,
            state::StateMessage,
        },
    },
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute_v3(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (forkchoice_state, payload_attributes) = parse_params_v3(request)?;
    let response = inner_execute_v3(forkchoice_state, payload_attributes, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params_v3(
    request: serde_json::Value,
) -> Result<(ForkchoiceStateV1, Option<PayloadAttributesV3>), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [x] => {
            let fc_state: ForkchoiceStateV1 = json_utils::deserialize(x)?;
            Ok((fc_state, None))
        }
        [x, y] => {
            let fc_state: ForkchoiceStateV1 = json_utils::deserialize(x)?;
            let payload_attributes: Option<PayloadAttributesV3> = json_utils::deserialize(y)?;
            Ok((fc_state, payload_attributes))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute_v3(
    forkchoice_state: ForkchoiceStateV1,
    payload_attributes: Option<PayloadAttributesV3>,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<ForkchoiceUpdatedResponseV1, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-1

    // TODO: implement proper validation of Forkchoice state

    // Update the state with the new head
    let msg = StateMessage::UpdateHead {
        block_hash: forkchoice_state.head_block_hash,
    };
    state_channel.send(msg).await.map_err(access_state_error)?;

    let payload_status = PayloadStatusV1 {
        status: Status::Valid,
        latest_valid_hash: Some(forkchoice_state.head_block_hash),
        validation_error: None,
    };

    // If `payload_attributes` are present then tell state to start producing a new block
    let payload_id = if let Some(attrs) = payload_attributes {
        let (tx, rx) = oneshot::channel();
        let msg = StateMessage::StartBlockBuild {
            payload_attributes: attrs,
            response_channel: tx,
        };
        state_channel.send(msg).await.map_err(access_state_error)?;
        Some(rx.await.map_err(access_state_error)?)
    } else {
        None
    };

    Ok(ForkchoiceUpdatedResponseV1 {
        payload_status,
        payload_id,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use {
        super::*,
        crate::{
            block::{Block, BlockRepository, Eip1559GasFee, InMemoryBlockRepository},
            genesis::{config::GenesisConfig, init_state},
            primitives::{Address, Bytes, B256, U256, U64},
            storage::InMemoryState,
        },
        alloy::primitives::hex,
    };

    pub fn example_request() -> serde_json::Value {
        serde_json::from_str(r#"
            {
                "id": 30053,
                "jsonrpc": "2.0",
                "method": "engine_forkchoiceUpdatedV3",
                "params": [
                {
                    "finalizedBlockHash": "0x2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d",
                    "headBlockHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    "safeBlockHash": "0xc9488c812782fac769416f918718107ca8f44f98fd2fe7dbcc12b9f5afa276dd"
                },
                {
                    "gasLimit": "0x1c9c380",
                    "parentBeaconBlockRoot": "0x2bd857e239f7e5b5e6415608c76b90600d51fa0f7f0bbbc04e2d6861b3186f1c",
                    "prevRandao": "0xbde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9",
                    "suggestedFeeRecipient": "0x4200000000000000000000000000000000000011",
                    "timestamp": "0x6660737b",
                    "transactions": [
                        "0x7ef8f8a0de86bef815fc910df65a9459ccb2b9a35fa8596dfcfed1ff01bbf28891d86d5e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000000000000006660735b00000000000001a9000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000017ae3f74f0134521a7d62a387ac75a5153bcd1aab1c7e003e9b9e15a5d8846363000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"
                    ],
                    "withdrawals": []
                }
                ]
            }
        "#).unwrap()
    }

    #[test]
    fn test_parse_params_v3() {
        let request: serde_json::Value = serde_json::from_str(r#"
            {
                "id": 30053,
                "jsonrpc": "2.0",
                "method": "engine_forkchoiceUpdatedV3",
                "params": [
                {
                    "finalizedBlockHash": "0x2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d",
                    "headBlockHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    "safeBlockHash": "0xc9488c812782fac769416f918718107ca8f44f98fd2fe7dbcc12b9f5afa276dd"
                },
                {
                    "gasLimit": "0x1c9c380",
                    "parentBeaconBlockRoot": "0x2bd857e239f7e5b5e6415608c76b90600d51fa0f7f0bbbc04e2d6861b3186f1c",
                    "prevRandao": "0xbde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9",
                    "suggestedFeeRecipient": "0x4200000000000000000000000000000000000011",
                    "timestamp": "0x6660737b",
                    "transactions": [
                        "0x7ef8f8a0de86bef815fc910df65a9459ccb2b9a35fa8596dfcfed1ff01bbf28891d86d5e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000000000000006660735b00000000000001a9000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000017ae3f74f0134521a7d62a387ac75a5153bcd1aab1c7e003e9b9e15a5d8846363000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"
                    ],
                    "withdrawals": []
                }
                ]
            }
        "#).unwrap();

        let params = parse_params_v3(request).unwrap();

        let expected_params = (
            ForkchoiceStateV1 {
                head_block_hash: B256::new(
                    hex!("e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d")),
                safe_block_hash: B256::new(
                    hex!("c9488c812782fac769416f918718107ca8f44f98fd2fe7dbcc12b9f5afa276dd")),
                finalized_block_hash: B256::new(
                    hex!("2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d")),
            },
            Some(PayloadAttributesV3 {
                timestamp: U64::from_be_slice(&hex!("6660737b")),
                prev_randao: B256::new(
                    hex!("bde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9")),
                suggested_fee_recipient: Address::new(hex!("4200000000000000000000000000000000000011")),
                withdrawals: Vec::new(),
                parent_beacon_block_root: B256::new(
                    hex!("2bd857e239f7e5b5e6415608c76b90600d51fa0f7f0bbbc04e2d6861b3186f1c")),
                transactions: vec![
                    Bytes::from_static(&hex!("7ef8f8a0de86bef815fc910df65a9459ccb2b9a35fa8596dfcfed1ff01bbf28891d86d5e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000000000000006660735b00000000000001a9000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000017ae3f74f0134521a7d62a387ac75a5153bcd1aab1c7e003e9b9e15a5d8846363000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"))
                ],
                gas_limit: U64::from_be_slice(&hex!("01c9c380")),
            }),
        );

        assert_eq!(params, expected_params);

        let request: serde_json::Value = serde_json::from_str(r#"
            {
                "id": 32034,
                "jsonrpc": "2.0",
                "method": "engine_forkchoiceUpdatedV3",
                "params": [
                    {
                    "finalizedBlockHash": "0x2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d",
                    "headBlockHash": "0xb412d0583c92bd00d1987291ba05a894af7483ff9b6e33891a47cf125f400ce2",
                    "safeBlockHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
                    },
                    null
                ]
            }
        "#).unwrap();

        let params = parse_params_v3(request).unwrap();

        let expected_params = (
            ForkchoiceStateV1 {
                head_block_hash: B256::new(hex!(
                    "b412d0583c92bd00d1987291ba05a894af7483ff9b6e33891a47cf125f400ce2"
                )),
                safe_block_hash: B256::new(hex!(
                    "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
                )),
                finalized_block_hash: B256::new(hex!(
                    "2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d"
                )),
            },
            None,
        );

        assert_eq!(params, expected_params);
    }

    #[tokio::test]
    async fn test_execute_v3() {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut repository = InMemoryBlockRepository::new();
        repository.add(genesis_block);

        let mut state = InMemoryState::new();
        init_state(&genesis_config, &mut state);

        let state = crate::state_actor::StateActor::new(
            rx,
            state,
            head_hash,
            genesis_config,
            0x03421ee50df45cacu64,
            B256::ZERO,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
        );
        let state_handle = state.spawn();
        let request = example_request();

        let expected_response: serde_json::Value = serde_json::from_str(r#"
            {
                "payloadStatus": {
                    "status": "VALID",
                    "latestValidHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    "validationError": null
                },
                "payloadId": "0x03421ee50df45cac"
            }
        "#).unwrap();

        let response = execute_v3(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
