use {
    crate::tests::test_context::{handle_request, TestContext},
    alloy::hex,
    umi_api::schema::{
        BlobsBundleV1, ExecutionPayloadV3, ForkchoiceUpdatedResponseV1, GetPayloadResponseV3,
        PayloadId, PayloadStatusV1, Status,
    },
    umi_execution::U256,
    umi_shared::primitives::{Address, Bytes, B2048, B256, U64},
};

#[tokio::test]
async fn test_sending_the_same_payload_twice_produces_one_block() -> anyhow::Result<()> {
    TestContext::run(|ctx| async move {
        let block_hash = "0x96f5d9746c63cf212c7ad848425c746bcbbdb738352f6b81eac13375a838cec7";

        let request = serde_json::json!({
            "jsonrpc":"2.0",
            "id":10,
            "method":"engine_forkchoiceUpdatedV3",
            "params":[
                {
                    "headBlockHash": format!("{block_hash}"),
                    "safeBlockHash": format!("{block_hash}"),
                    "finalizedBlockHash": format!("{block_hash}")
                },
                {
                    "timestamp":"0x68235d0f",
                    "prevRandao":"0xdd9b0c0d88d7d9e5fe6718d97f5f2cfd9d825cf6265a39c08650de249e138339",
                    "suggestedFeeRecipient":"0x4200000000000000000000000000000000000011",
                    "withdrawals":[],
                    "parentBeaconBlockRoot":"0x0000000000000000000000000000000000000000000000000000000000000000",
                    "transactions":["0x7ef8f8a08c2449b17ee7c7ad9a93f6dbd0ac4d3a666f5c3183aa19f8c2dcc8a310cc878894deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000022950000c5f4f00000000000000010000000068235d0e00000000000000200000000000000000000000000000000000000000000000000000000000d9858400000000000000000000000000000000000000000000000000000000000000015d16200108a6cd9dd5bcf4b1e8670dafbb7854a380091e3e3da33ed07cf8214f0000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9"],
                    "gasLimit":"0x1c9c380"
                }
            ]
        });
        let payload_id = PayloadId::new(268041640064641444);
        let expected_response = ForkchoiceUpdatedResponseV1 {
            payload_status: PayloadStatusV1 {
                status: Status::Valid,
                latest_valid_hash: Some(B256::new(hex!(
                    "96f5d9746c63cf212c7ad848425c746bcbbdb738352f6b81eac13375a838cec7"
                ))),
                validation_error: None,
            },
            payload_id: Some(payload_id),
        };

        let actual_response: ForkchoiceUpdatedResponseV1 =
            handle_request("/", request.clone(), &ctx.queue, &ctx.reader)
                .await
                .unwrap();

        assert_eq!(actual_response, expected_response);

        let actual_response: ForkchoiceUpdatedResponseV1 =
            handle_request("/", request.clone(), &ctx.queue, &ctx.reader)
                .await
                .unwrap();

        assert_eq!(actual_response, expected_response);

        ctx.queue.wait_for_pending_commands().await;

        let request = serde_json::json!({
            "jsonrpc":"2.0",
            "id":11,
            "method":"engine_getPayloadV3",
            "params":[
                format!("{payload_id}")
            ]
        });

        let actual_response: GetPayloadResponseV3 =
            handle_request("/", request, &ctx.queue, &ctx.reader)
                .await
                .unwrap();

        let expected_response: GetPayloadResponseV3 = GetPayloadResponseV3 {
            execution_payload: ExecutionPayloadV3 {
                parent_hash: B256::new(hex!("f66be0592e99afa8984377a46f22a6e5bf5f028baccda2677c06536c41fd31ed")),
                fee_recipient: Address::new(hex!("4200000000000000000000000000000000000011")),
                state_root: B256::new(hex!("aaef519dd1e591d139ab2174aa45c1f46cc35e47983189613b6f1642d84f92a9")),
                receipts_root: B256::new(hex!("0d4c87e20ba9c234ff06a6a6668099b1cfe4fd956ecf9a0d257b8ffd067c1c5a")),
                logs_bloom: B2048::new(hex!("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")),
                prev_randao: B256::new(hex!("dd9b0c0d88d7d9e5fe6718d97f5f2cfd9d825cf6265a39c08650de249e138339")),
                block_number: U64::from_limbs([1]),
                gas_limit: U64::from_limbs([30000000]),
                gas_used: U64::from_limbs([139445]),
                timestamp: U64::from_limbs([1747148047]),
                extra_data: Bytes::new(),
                base_fee_per_gas: U256::ZERO,
                block_hash: B256::new(hex!("62581f9d2327af2d1387ff4e0fdb08e1f4b28547b8a92fd2b0cb96bf4121d931")),
                transactions: vec![Bytes::from_static(&hex!("7ef8f8a08c2449b17ee7c7ad9a93f6dbd0ac4d3a666f5c3183aa19f8c2dcc8a310cc878894deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000022950000c5f4f00000000000000010000000068235d0e00000000000000200000000000000000000000000000000000000000000000000000000000d9858400000000000000000000000000000000000000000000000000000000000000015d16200108a6cd9dd5bcf4b1e8670dafbb7854a380091e3e3da33ed07cf8214f0000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9"))],
                withdrawals: vec![],
                blob_gas_used: U64::ZERO,
                excess_blob_gas: U64::ZERO,
            },
            block_value: U256::ZERO,
            blobs_bundle: BlobsBundleV1 { commitments: vec![], proofs: vec![], blobs: vec![] },
            should_override_builder: false,
            parent_beacon_block_root: B256::ZERO,
        };

        assert_eq!(actual_response, expected_response);

        ctx.shutdown().await;
        Ok(())
    }).await
}
