pub use alloy::eips::BlockNumberOrTag;

use {
    moved_app::{RpcBlock, RpcTransaction},
    moved_blockchain::{block::BlockResponse, transaction::TransactionResponse},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetBlockResponse(pub RpcBlock);

impl From<BlockResponse> for GetBlockResponse {
    fn from(value: BlockResponse) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetTransactionResponse(pub RpcTransaction);

impl From<TransactionResponse> for GetTransactionResponse {
    fn from(value: TransactionResponse) -> Self {
        Self(value)
    }
}
