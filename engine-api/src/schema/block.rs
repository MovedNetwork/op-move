use {
    moved::{
        primitives::{Address, Bytes, B2048, B256},
        types::state::{BlockResponse, TransactionResponse},
    },
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBlockResponse {
    /// the block number. null when its pending block.
    number: u64,
    /// hash of the block. null when its pending block.
    hash: B256,
    /// hash of the parent block.
    parent_hash: B256,
    /// hash of the generated proof-of-work. null when its pending block.
    nonce: Option<u64>,
    /// SHA3 of the uncles data in the block.
    sha3_uncles: B256,
    /// the bloom filter for the logs of the block. null when its pending block.
    logs_bloom: B2048,
    /// the root of the transaction trie of the block.
    transactions_root: B256,
    /// the root of the final state trie of the block.
    state_root: B256,
    /// the root of the receipts trie of the block.
    receipts_root: B256,
    /// the address of the beneficiary to whom the mining rewards were given.
    miner: Address,
    /// integer of the difficulty for this block.
    difficulty: u64,
    /// integer of the total difficulty of the chain until this block.
    total_difficulty: u64,
    /// the "extra data" field of this block.
    extra_data: Bytes,
    /// integer the size of this block in bytes.
    size: u64,
    /// the maximum gas allowed in this block.
    gas_limit: u64,
    /// the total used gas by all transactions in this block.
    gas_used: u64,
    /// the unix timestamp for when the block was collated.
    timestamp: u64,
    /// Array of transaction objects, or 32 Bytes transaction hashes depending on the last given parameter.
    transactions: Vec<TransactionInfo>,
    /// Array of uncle hashes.
    uncles: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionInfo {
    Body(B256), // todo transaction body
    Hash(B256),
}

impl From<TransactionResponse> for TransactionInfo {
    fn from(value: TransactionResponse) -> Self {
        match value {
            TransactionResponse::Hash(hash) => Self::Hash(hash),
            TransactionResponse::Body(_body) => Self::Body(B256::ZERO),
        }
    }
}

impl From<BlockResponse> for GetBlockResponse {
    fn from(value: BlockResponse) -> Self {
        Self {
            number: value.number.into(),
            hash: value.hash.into(),
            parent_hash: value.parent_hash.into(),
            nonce: value.nonce.into(),
            sha3_uncles: value.sha3_uncles.into(),
            logs_bloom: value.logs_bloom.into(),
            transactions_root: value.transactions_root.into(),
            state_root: value.state_root.into(),
            receipts_root: value.receipts_root.into(),
            miner: value.miner.into(),
            difficulty: value.difficulty.into(),
            total_difficulty: value.total_difficulty.into(),
            extra_data: value.extra_data.into(),
            size: value.size.into(),
            gas_limit: value.gas_limit.into(),
            gas_used: value.gas_used.into(),
            timestamp: value.timestamp.into(),
            transactions: value.transactions.into_iter().map(Into::into).collect(),
            uncles: value.uncles.into(),
        }
    }
}
