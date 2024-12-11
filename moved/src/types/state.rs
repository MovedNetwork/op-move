//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    crate::{
        block::{ExtendedBlock, Header},
        primitives::{Address, Bytes, ToSaturatedU64, ToU64, B2048, B256, U256, U64},
        state_actor::NewPayloadIdInput,
        types::transactions::{ExtendedTxEnvelope, NormalizedExtendedTxEnvelope},
    },
    alloy::{
        consensus::transaction::TxEnvelope,
        eips::BlockNumberOrTag,
        rpc::types::{BlockTransactions, FeeHistory, TransactionRequest},
    },
    alloy_rlp::Encodable,
    op_alloy::{consensus::OpReceiptEnvelope, rpc_types::L1BlockInfo},
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Withdrawal {
    pub index: U64,
    pub validator_index: U64,
    pub address: Address,
    pub amount: U64,
}

pub type PayloadId = U64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadResponse {
    pub execution_payload: ExecutionPayload,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundle,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: B256,
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
    GetPayload {
        id: PayloadId,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    GetPayloadByBlockHash {
        block_hash: B256,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    AddTransaction {
        tx: TxEnvelope,
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
        response_channel: oneshot::Sender<Vec<u8>>,
    },
    TransactionReceipt {
        tx_hash: B256,
        response_channel: oneshot::Sender<Option<TransactionReceipt>>,
    },
}

impl From<Query> for StateMessage {
    fn from(value: Query) -> Self {
        Self::Query(value)
    }
}

pub type RpcBlock = alloy::rpc::types::Block<alloy::rpc::types::Transaction<ExtendedTxEnvelope>>;

#[derive(Debug)]
pub struct BlockResponse(pub RpcBlock);

impl BlockResponse {
    fn new(value: RpcBlock) -> Self {
        Self(value)
    }

    pub fn from_block_with_transaction_hashes(value: ExtendedBlock) -> Self {
        Self::new(RpcBlock {
            transactions: BlockTransactions::Hashes(
                value
                    .block
                    .transactions
                    .iter()
                    .map(ExtendedTxEnvelope::compute_hash)
                    .collect(),
            ),
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header.into(),
                // TODO: review fields below
                total_difficulty: None,
                size: None,
            },
            // TODO: review fields below
            uncles: Vec::new(),
            withdrawals: None,
        })
    }

    pub fn from_block_with_transactions(value: ExtendedBlock) -> Self {
        Self::new(RpcBlock {
            transactions: BlockTransactions::Full(
                value
                    .block
                    .transactions
                    .into_iter()
                    .enumerate()
                    .map(|(i, inner)| alloy::rpc::types::Transaction {
                        block_hash: Some(value.hash),
                        block_number: Some(value.block.header.number),
                        transaction_index: Some(i as u64),
                        effective_gas_price: None, // TODO: Gas it up #160
                        from: inner
                            .sender()
                            .expect("Block transactions should contain valid signature"),
                        inner,
                    })
                    .collect(),
            ),
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header.into(),
                // TODO: review fields below
                total_difficulty: None,
                size: None,
            },
            // TODO: review fields below
            uncles: Vec::new(),
            withdrawals: None,
        })
    }
}

impl From<Header> for alloy::consensus::Header {
    fn from(value: Header) -> Self {
        Self {
            number: value.number,
            parent_hash: value.parent_hash,
            ommers_hash: value.ommers_hash,
            nonce: value.nonce.into(),
            base_fee_per_gas: Some(value.base_fee_per_gas.to_saturated_u64()),
            blob_gas_used: Some(value.blob_gas_used),
            excess_blob_gas: Some(value.excess_blob_gas),
            parent_beacon_block_root: Some(value.parent_beacon_block_root),
            logs_bloom: value.logs_bloom.into(),
            transactions_root: value.transactions_root,
            state_root: value.state_root,
            receipts_root: value.receipts_root,
            difficulty: value.difficulty,
            extra_data: value.extra_data,
            gas_limit: value.gas_limit,
            gas_used: value.gas_used,
            timestamp: value.timestamp,
            beneficiary: value.beneficiary,
            // TODO: review fields below
            mix_hash: Default::default(),
            requests_hash: None,
            withdrawals_root: None,
        }
    }
}

pub type TransactionReceipt = op_alloy::rpc_types::OpTransactionReceipt;

#[derive(Debug)]
pub struct ExecutionOutcome {
    pub receipts_root: B256,
    pub state_root: B256,
    pub logs_bloom: B2048,
    pub gas_used: U64,
    pub total_tip: U256,
}

#[derive(Debug, Clone)]
pub struct TransactionWithReceipt {
    pub tx_hash: B256,
    pub tx: ExtendedTxEnvelope,
    pub tx_index: u64,
    pub normalized_tx: NormalizedExtendedTxEnvelope,
    pub receipt: OpReceiptEnvelope,
    pub l1_block_info: Option<L1BlockInfo>,
    pub gas_used: u64,
    /// If the transaction deployed a new contract, gives the address.
    ///
    /// In Move contracts are identified by AccountAddress + ModuleID,
    /// so this field cannot capture all the detail of a new deployment,
    /// however we cannot extend the field because it is here for Ethereum
    /// compatibility. As a compromise, we will put the AccountAddress here
    /// and the user would need to look up the ModuleID by inspecting the
    /// transaction object itself.
    pub contract_address: Option<Address>,
    /// Counts the number of logs that exist in transactions appearing earlier
    /// in the same block.
    ///
    /// This allows computing the log index for each log in this transaction.
    pub logs_offset: u64,
}

pub(crate) trait WithExecutionOutcome {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self;
}

impl WithExecutionOutcome for Header {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self {
        Self {
            state_root: outcome.state_root,
            receipts_root: outcome.receipts_root,
            logs_bloom: outcome.logs_bloom,
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
            index: self.index.into_limbs()[0],
            validator_index: self.validator_index.into_limbs()[0],
            address: self.address,
            amount: self.amount.into_limbs()[0],
        }
    }
}

impl From<ExtendedBlock> for PayloadResponse {
    fn from(value: ExtendedBlock) -> Self {
        PayloadResponse {
            parent_beacon_block_root: value.block.header.parent_beacon_block_root,
            block_value: value.value,
            execution_payload: ExecutionPayload::from(value),
            blobs_bundle: Default::default(),
            should_override_builder: false,
        }
    }
}

impl From<ExtendedBlock> for ExecutionPayload {
    fn from(value: ExtendedBlock) -> Self {
        let transactions = value
            .block
            .transactions
            .into_iter()
            .map(|tx| {
                let capacity = tx.length();
                let mut bytes = Vec::with_capacity(capacity);
                tx.encode(&mut bytes);
                bytes.into()
            })
            .collect();

        Self {
            block_hash: value.hash,
            parent_hash: value.block.header.parent_hash,
            fee_recipient: value.block.header.beneficiary,
            state_root: value.block.header.state_root,
            receipts_root: value.block.header.receipts_root,
            logs_bloom: value.block.header.logs_bloom,
            prev_randao: value.block.header.prev_randao,
            block_number: U64::from(value.block.header.number),
            gas_limit: U64::from(value.block.header.gas_limit),
            gas_used: U64::from(value.block.header.gas_used),
            timestamp: U64::from(value.block.header.timestamp),
            extra_data: value.block.header.extra_data,
            base_fee_per_gas: value.block.header.base_fee_per_gas,
            transactions,
            withdrawals: Vec::new(), // TODO: withdrawals
            blob_gas_used: U64::from(value.block.header.blob_gas_used),
            excess_blob_gas: U64::from(value.block.header.excess_blob_gas),
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
            prev_randao: payload.prev_randao,
            parent_beacon_block_root: payload.parent_beacon_block_root,
            ..self
        }
    }
}
