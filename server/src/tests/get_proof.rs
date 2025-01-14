use {
    crate::initialize_state_actor,
    eth_trie::{EthTrie, MemoryDB, Trie},
    moved::{
        genesis::config::GenesisConfig,
        types::{queries::ProofResponse, state::StateMessage},
    },
    moved_engine_api::schema::{
        ForkchoiceUpdatedResponseV1, GetBlockResponse, GetPayloadResponseV3, PayloadStatusV1,
        Status,
    },
    serde::de::DeserializeOwned,
    std::sync::Arc,
    tokio::sync::mpsc,
};

#[tokio::test]
async fn test_get_proof() -> anyhow::Result<()> {
    let (state_channel, rx) = mpsc::channel(10);
    let genesis_config = GenesisConfig::default();
    let state_actor = initialize_state_actor(genesis_config, rx);

    let state_task = state_actor.spawn();

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "eth_getBlockByNumber",
        "params": [
            "0x0",
            true
        ]
    });
    let genesis_block: GetBlockResponse = handle_request(request, &state_channel).await?;
    let genesis_hash = genesis_block.0.header.hash;

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "engine_forkchoiceUpdatedV3",
        "params": [
            {
                "headBlockHash": format!("{}", genesis_hash),
                "safeBlockHash": format!("{}", genesis_hash),
                "finalizedBlockHash": format!("{}", genesis_hash)
            },
            null
        ]
    });
    let response: ForkchoiceUpdatedResponseV1 = handle_request(request, &state_channel).await?;
    assert_eq!(response.payload_status.status, Status::Valid);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "engine_forkchoiceUpdatedV3",
        "params": [
            {
                "headBlockHash": format!("{}", genesis_hash),
                "safeBlockHash": format!("{}", genesis_hash),
                "finalizedBlockHash": format!("{}", genesis_hash)
            },
            {
                "timestamp": "0x6776ff9d",
                "prevRandao": "0x25a6a508a4516852626c6213354a3b01b4f482fa4c8b765ab7ef833bd1f77f72",
                "suggestedFeeRecipient": "0x4200000000000000000000000000000000000011",
                "withdrawals": [],
                "parentBeaconBlockRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "transactions": [
                    "0x7ef8f8a06e6097e888aa6423cc7114c38a41184f030498feb9f6807ee3861d7039c786ca94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000022950000c5f4f0000000000000000000000006776ff9d00000000000000210000000000000000000000000000000000000000000000000000000000bd33030000000000000000000000000000000000000000000000000000000000000001cb0216ad7562c7b4431d3ad76a8c6e9c7a72372ab98a932627bed559e9a8d17d0000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9"
                ],
                "gasLimit": "0x1c9c380"
            }
        ]
    });
    let response: ForkchoiceUpdatedResponseV1 = handle_request(request, &state_channel).await?;
    let payload_id = response.payload_id.unwrap();

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "engine_getPayloadV3",
        "params": [
           String::from(payload_id),
        ]
    });
    let response: GetPayloadResponseV3 = handle_request(request, &state_channel).await?;

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "engine_newPayloadV3",
        "params": [
           response.execution_payload,
           [],
           "0x0000000000000000000000000000000000000000000000000000000000000000"
        ]
    });
    let response: PayloadStatusV1 = handle_request(request, &state_channel).await?;
    assert_eq!(response.status, Status::Valid);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "eth_getBlockByNumber",
        "params": [
            "0x1",
            true
        ]
    });
    let block: GetBlockResponse = handle_request(request, &state_channel).await?;
    let state_root = block.0.header.state_root;

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "eth_getProof",
        "params": [
           "0x4200000000000000000000000000000000000016",
           [],
           format!("{}", response.latest_valid_hash.unwrap())
        ]
    });
    let response: ProofResponse = handle_request(request, &state_channel).await?;

    // Proof is verified successfully
    let trie = EthTrie::new(Arc::new(MemoryDB::new(false)));
    let key = alloy::primitives::keccak256(alloy::hex!("4200000000000000000000000000000000000016"));
    trie.verify_proof(
        state_root,
        key.as_slice(),
        response
            .account_proof
            .into_iter()
            .map(|x| x.to_vec())
            .collect(),
    )
    .unwrap()
    .unwrap();

    drop(state_channel);
    state_task.await.unwrap();
    Ok(())
}

async fn handle_request<T: DeserializeOwned>(
    request: serde_json::Value,
    state_channel: &mpsc::Sender<StateMessage>,
) -> anyhow::Result<T> {
    let response = moved_engine_api::request::handle(request.clone(), state_channel.clone()).await;

    if let Some(error) = response.error {
        anyhow::bail!("Error response from request {request:?}: {error:?}");
    }

    let result: T = serde_json::from_value(response.result.expect("If not error then has result"))?;
    Ok(result)
}
