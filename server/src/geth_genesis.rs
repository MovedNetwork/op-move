//! A temporary module holding logic necessary for extracting the genesis block information
//! from op-geth. This module can be deleted once we are computing the genesis hash expected
//! by op-node ourselves.

use {
    moved_app::RpcBlock,
    moved_blockchain::block::{Block, ExtendedBlock, Header},
};

pub fn is_genesis_block_request(request: &serde_json::Value) -> Option<bool> {
    let obj = request.as_object()?;
    let method = obj.get("method")?.as_str()?;
    if method != "eth_getBlockByNumber" {
        return Some(false);
    }
    let first_param = obj.get("params")?.as_array()?.first()?.as_str()?;
    Some(first_param == "0x0")
}

pub fn extract_genesis_block(geth_response: &serde_json::Value) -> Option<ExtendedBlock> {
    let geth_block: RpcBlock =
        serde_json::from_value(geth_response.as_object()?.get("result")?.clone()).ok()?;
    let header = Header {
        parent_hash: geth_block.header.parent_hash,
        ommers_hash: geth_block.header.ommers_hash,
        beneficiary: geth_block.header.beneficiary,
        state_root: geth_block.header.state_root,
        transactions_root: geth_block.header.transactions_root,
        receipts_root: geth_block.header.receipts_root,
        logs_bloom: geth_block.header.logs_bloom,
        difficulty: geth_block.header.difficulty,
        number: geth_block.header.number,
        gas_limit: geth_block.header.gas_limit,
        gas_used: geth_block.header.gas_used,
        timestamp: geth_block.header.timestamp,
        extra_data: geth_block.header.extra_data.clone(),
        nonce: geth_block.header.nonce,
        base_fee_per_gas: geth_block.header.base_fee_per_gas,
        withdrawals_root: geth_block.header.withdrawals_root,
        blob_gas_used: geth_block.header.blob_gas_used,
        excess_blob_gas: geth_block.header.excess_blob_gas,
        parent_beacon_block_root: geth_block.header.parent_beacon_block_root,
        mix_hash: geth_block.header.mix_hash,
        requests_hash: geth_block.header.requests_hash,
    };
    let block = Block::new(header, Vec::new());
    let ext_block = ExtendedBlock::new(
        geth_block.header.hash,
        Default::default(),
        Default::default(),
        block,
    );
    Some(ext_block)
}
