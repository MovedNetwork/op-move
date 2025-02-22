//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    super::queries::ProofResponse,
    crate::{
        block::{ExtendedBlock, Header},
        payload::NewPayloadIdInput,
        receipt::TransactionReceipt,
        transaction::{ExtendedTransaction, TransactionResponse},
    },
    alloy::{
        consensus::transaction::TxEnvelope,
        eips::{eip2718::Encodable2718, BlockId, BlockNumberOrTag},
        primitives::Bloom,
        rpc::types::{BlockTransactions, FeeHistory, TransactionRequest, Withdrawals},
    },
    moved_shared::primitives::{Address, Bytes, ToU64, B2048, B256, U256, U64},
    op_alloy::consensus::OpTxEnvelope,
    tokio::sync::oneshot,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Payload {
    pub timestamp: U64,
    pub prev_randao: B256,
    pub suggested_fee_recipient: Address,
    pub withdrawals: Vec<Withdrawal>,
    pub parent_beacon_block_root: B256,
    pub transactions: Vec<Bytes>,
    pub gas_limit: U64,
}

pub type Withdrawal = alloy::rpc::types::Withdrawal;

pub type PayloadId = U64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadResponse {
    pub execution_payload: ExecutionPayload,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundle,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: B256,
}

impl PayloadResponse {
    pub fn from_block(value: ExtendedBlock) -> Self {
        Self {
            parent_beacon_block_root: value
                .block
                .header
                .parent_beacon_block_root
                .unwrap_or_default(),
            block_value: value.value,
            execution_payload: ExecutionPayload::from_block(value),
            blobs_bundle: Default::default(),
            should_override_builder: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionPayload {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: B2048,
    pub prev_randao: B256,
    pub block_number: U64,
    pub gas_limit: U64,
    pub gas_used: U64,
    pub timestamp: U64,
    pub extra_data: Bytes,
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: Vec<Bytes>,
    pub withdrawals: Vec<Withdrawal>,
    pub blob_gas_used: U64,
    pub excess_blob_gas: U64,
}

impl ExecutionPayload {
    pub fn from_block(value: ExtendedBlock) -> Self {
        let transactions = value
            .block
            .transactions
            .into_iter()
            .map(|tx| {
                let capacity = tx.eip2718_encoded_length();
                let mut bytes = Vec::with_capacity(capacity);
                tx.encode_2718(&mut bytes);
                bytes.into()
            })
            .collect();

        Self {
            block_hash: value.hash,
            parent_hash: value.block.header.parent_hash,
            fee_recipient: value.block.header.beneficiary,
            state_root: value.block.header.state_root,
            receipts_root: value.block.header.receipts_root,
            logs_bloom: value.block.header.logs_bloom.0,
            prev_randao: value.block.header.mix_hash,
            block_number: U64::from(value.block.header.number),
            gas_limit: U64::from(value.block.header.gas_limit),
            gas_used: U64::from(value.block.header.gas_used),
            timestamp: U64::from(value.block.header.timestamp),
            extra_data: value.block.header.extra_data,
            base_fee_per_gas: U256::from(value.block.header.base_fee_per_gas.unwrap_or_default()),
            transactions,
            withdrawals: Vec::new(), // TODO: withdrawals
            blob_gas_used: U64::from(value.block.header.blob_gas_used.unwrap_or_default()),
            excess_blob_gas: U64::from(value.block.header.excess_blob_gas.unwrap_or_default()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlobsBundle {
    pub commitments: Vec<Bytes>,
    pub proofs: Vec<Bytes>,
    pub blobs: Vec<Bytes>,
}

#[derive(Debug)]
pub enum StateMessage {
    Command(Command),
    Query(Query),
}

#[derive(Debug)]
pub enum Command {
    UpdateHead {
        block_hash: B256,
    },
    StartBlockBuild {
        payload_attributes: Payload,
        response_channel: oneshot::Sender<PayloadId>,
    },
    AddTransaction {
        tx: TxEnvelope,
    },
    GenesisUpdate {
        block: ExtendedBlock,
    },
}

impl From<Command> for StateMessage {
    fn from(value: Command) -> Self {
        Self::Command(value)
    }
}

#[derive(Debug)]
pub enum Query {
    ChainId {
        response_channel: oneshot::Sender<u64>,
    },
    BalanceByHeight {
        address: Address,
        height: BlockNumberOrTag,
        response_channel: oneshot::Sender<Option<U256>>,
    },
    NonceByHeight {
        address: Address,
        height: BlockNumberOrTag,
        response_channel: oneshot::Sender<Option<u64>>,
    },
    BlockByHash {
        hash: B256,
        include_transactions: bool,
        response_channel: oneshot::Sender<Option<BlockResponse>>,
    },
    BlockByHeight {
        height: BlockNumberOrTag,
        include_transactions: bool,
        response_channel: oneshot::Sender<Option<BlockResponse>>,
    },
    BlockNumber {
        response_channel: oneshot::Sender<u64>,
    },
    FeeHistory {
        block_count: u64,
        block_number: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
        response_channel: oneshot::Sender<FeeHistory>,
    },
    EstimateGas {
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
        response_channel: oneshot::Sender<crate::Result<u64>>,
    },
    Call {
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
        response_channel: oneshot::Sender<crate::Result<Vec<u8>>>,
    },
    TransactionReceipt {
        tx_hash: B256,
        response_channel: oneshot::Sender<Option<TransactionReceipt>>,
    },
    TransactionByHash {
        tx_hash: B256,
        response_channel: oneshot::Sender<Option<TransactionResponse>>,
    },
    GetProof {
        address: Address,
        storage_slots: Vec<U256>,
        height: BlockId,
        response_channel: oneshot::Sender<Option<ProofResponse>>,
    },
    GetPayload {
        id: PayloadId,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    GetPayloadByBlockHash {
        block_hash: B256,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
}

impl From<Query> for StateMessage {
    fn from(value: Query) -> Self {
        Self::Query(value)
    }
}

pub type RpcBlock = alloy::rpc::types::Block<RpcTransaction>;
pub type RpcTransaction = op_alloy::rpc_types::Transaction;

#[derive(Debug)]
pub struct BlockResponse(pub RpcBlock);

impl BlockResponse {
    fn new(transactions: BlockTransactions<RpcTransaction>, value: ExtendedBlock) -> Self {
        Self(RpcBlock {
            transactions,
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header,
                // TODO: review fields below
                total_difficulty: None,
                size: None,
            },
            uncles: Vec::new(),
            withdrawals: Some(Withdrawals(Vec::new())),
        })
    }

    pub fn from_block_with_transaction_hashes(block: ExtendedBlock) -> Self {
        Self::new(
            BlockTransactions::Hashes(
                block
                    .block
                    .transactions
                    .iter()
                    .map(OpTxEnvelope::trie_hash)
                    .collect(),
            ),
            block,
        )
    }

    pub fn from_block_with_transactions(
        block: ExtendedBlock,
        transactions: Vec<ExtendedTransaction>,
    ) -> Self {
        Self::new(
            BlockTransactions::Full(transactions.into_iter().map(RpcTransaction::from).collect()),
            block,
        )
    }
}

#[derive(Debug)]
pub struct ExecutionOutcome {
    pub receipts_root: B256,
    pub state_root: B256,
    pub logs_bloom: B2048,
    pub gas_used: U64,
    pub total_tip: U256,
}

pub(crate) trait WithExecutionOutcome {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self;
}

impl WithExecutionOutcome for Header {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self {
        Self {
            state_root: outcome.state_root,
            receipts_root: outcome.receipts_root,
            logs_bloom: Bloom::new(outcome.logs_bloom.0),
            gas_used: outcome.gas_used.to_u64(),
            ..self
        }
    }
}

pub(crate) trait ToPayloadIdInput<'a> {
    fn to_payload_id_input(&'a self, head: &'a B256) -> NewPayloadIdInput<'a>;
}

impl<'a> ToPayloadIdInput<'a> for Payload {
    fn to_payload_id_input(&'a self, head: &'a B256) -> NewPayloadIdInput<'a> {
        NewPayloadIdInput::new_v3(
            head,
            self.timestamp.into_limbs()[0],
            &self.prev_randao,
            &self.suggested_fee_recipient,
        )
        .with_beacon_root(&self.parent_beacon_block_root)
        .with_withdrawals(
            self.withdrawals
                .iter()
                .map(ToWithdrawal::to_withdrawal)
                .collect::<Vec<_>>(),
        )
    }
}

trait ToWithdrawal {
    fn to_withdrawal(&self) -> alloy::eips::eip4895::Withdrawal;
}

impl ToWithdrawal for Withdrawal {
    fn to_withdrawal(&self) -> alloy::eips::eip4895::Withdrawal {
        alloy::eips::eip4895::Withdrawal {
            index: self.index,
            validator_index: self.validator_index,
            address: self.address,
            amount: self.amount,
        }
    }
}

pub(crate) trait WithPayloadAttributes {
    fn with_payload_attributes(self, payload: Payload) -> Self;
}

impl WithPayloadAttributes for Header {
    fn with_payload_attributes(self, payload: Payload) -> Self {
        Self {
            beneficiary: payload.suggested_fee_recipient,
            gas_limit: payload.gas_limit.to_u64(),
            timestamp: payload.timestamp.to_u64(),
            parent_beacon_block_root: Some(payload.parent_beacon_block_root),
            mix_hash: payload.prev_randao,
            ..self
        }
    }
}
