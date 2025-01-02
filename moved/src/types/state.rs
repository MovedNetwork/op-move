//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    crate::{
        block::{ExtendedBlock, Header},
        primitives::{Address, Bytes, ToU64, B2048, B256, U256, U64},
        state_actor::NewPayloadIdInput,
        types::transactions::NormalizedExtendedTxEnvelope,
    },
    alloy::{
        consensus::transaction::TxEnvelope,
        eips::{eip2718::Encodable2718, BlockNumberOrTag},
        primitives::Bloom,
        rpc::types::{BlockTransactions, FeeHistory, TransactionRequest},
    },
    op_alloy::{
        consensus::{OpReceiptEnvelope, OpTxEnvelope},
        rpc_types::L1BlockInfo,
    },
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
}

impl From<Query> for StateMessage {
    fn from(value: Query) -> Self {
        Self::Query(value)
    }
}

pub type RpcBlock = alloy::rpc::types::Block<op_alloy::rpc_types::Transaction>;

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
                    .map(|tx| match tx {
                        OpTxEnvelope::Legacy(tx) => *tx.hash(),
                        OpTxEnvelope::Eip1559(tx) => *tx.hash(),
                        OpTxEnvelope::Eip2930(tx) => *tx.hash(),
                        OpTxEnvelope::Eip7702(tx) => *tx.hash(),
                        OpTxEnvelope::Deposit(tx) => tx.hash(),
                        _ => unreachable!("Tx type not supported"),
                    })
                    .collect(),
            ),
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header,
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
                    .map(|(i, inner)| {
                        let tx = alloy::rpc::types::Transaction {
                            block_hash: Some(value.hash),
                            block_number: Some(value.block.header.number),
                            transaction_index: Some(i as u64),
                            // TODO: Gassing it up requires either modifying supported variants of
                            // `OpTxEnvelope` or storing a different type in block transactions
                            // altogether
                            effective_gas_price: None,
                            from: compute_from(&inner)
                                .expect("Block transactions should contain valid signature"),
                            inner,
                        };
                        op_alloy::rpc_types::Transaction {
                            inner: tx,
                            // TODO: what are these fields?
                            deposit_nonce: None,
                            deposit_receipt_version: None,
                        }
                    })
                    .collect(),
            ),
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header,
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

fn compute_from(tx: &OpTxEnvelope) -> Result<Address, alloy::primitives::SignatureError> {
    match tx {
        OpTxEnvelope::Legacy(tx) => tx.recover_signer(),
        OpTxEnvelope::Eip1559(tx) => tx.recover_signer(),
        OpTxEnvelope::Eip2930(tx) => tx.recover_signer(),
        OpTxEnvelope::Eip7702(tx) => tx.recover_signer(),
        OpTxEnvelope::Deposit(tx) => Ok(tx.from),
        _ => unreachable!("Tx type not supported"),
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
    pub tx: OpTxEnvelope,
    pub tx_index: u64,
    pub normalized_tx: NormalizedExtendedTxEnvelope,
    pub receipt: OpReceiptEnvelope,
    pub l1_block_info: Option<L1BlockInfo>,
    pub gas_used: u64,
    pub l2_gas_price: U256,
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
