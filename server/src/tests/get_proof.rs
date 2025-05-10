use {
    crate::tests::test_context::{handle_request, TestContext},
    eth_trie::{EthTrie, MemoryDB, Trie},
    moved_blockchain::state::ProofResponse,
    moved_evm_ext::state,
    std::sync::Arc,
};

#[tokio::test]
async fn test_get_proof() -> anyhow::Result<()> {
    let mut ctx = TestContext::new().await?;

    let block_hash = ctx.produce_block().await?;
    let block = ctx.get_block_by_number(1).await?;
    let state_root = block.0.header.state_root;

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "eth_getProof",
        "params": [
           "0x4200000000000000000000000000000000000016",
           [],
           format!("{block_hash}")
        ]
    });
    let response: ProofResponse = handle_request(request, &ctx.queue, ctx.app.clone()).await?;

    // Proof is verified successfully
    let trie = EthTrie::new(Arc::new(MemoryDB::new(false)));
    let key = alloy::primitives::keccak256(alloy::hex!("4200000000000000000000000000000000000016"));
    trie.verify_proof(
        state_root,
        key.as_slice(),
        response.account_proof.iter().map(|x| x.to_vec()).collect(),
    )?
    .unwrap();

    // Proof contains the right account data
    let account = state::Account::new(
        response.nonce,
        response.balance,
        response.code_hash,
        response.storage_hash,
    );
    let leaf = response.account_proof.last().unwrap();
    assert!(
        hex::encode(leaf).contains(hex::encode(account.serialize()).as_str()),
        "Proof leaf contains account data"
    );

    ctx.shutdown().await;
    Ok(())
}
