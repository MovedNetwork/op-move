use {
    alloy::{primitives::Bloom, rlp::Decodable},
    umi_blockchain::{
        block::{ExtendedBlock, Header},
        payload::{NewPayloadIdInput, PayloadId},
    },
    umi_execution::transaction::{NormalizedEthTransaction, NormalizedExtendedTxEnvelope},
    umi_shared::primitives::{Address, B256, B2048, Bytes, ToU64, U64, U256},
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

/// Internal representation of [`Payload`] that has its `transactions`
/// field parsed and normalized.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PayloadForExecution {
    pub timestamp: U64,
    pub prev_randao: B256,
    pub suggested_fee_recipient: Address,
    pub withdrawals: Vec<Withdrawal>,
    pub parent_beacon_block_root: B256,
    pub transactions: Vec<NormalizedExtendedTxEnvelope>,
    pub gas_limit: U64,
}

impl TryFrom<Payload> for PayloadForExecution {
    type Error = umi_shared::error::Error;

    fn try_from(value: Payload) -> Result<Self, Self::Error> {
        let mut transactions = Vec::new();

        for raw_tx in value.transactions {
            let mut slice: &[u8] = raw_tx.as_ref();
            let tx = NormalizedExtendedTxEnvelope::decode(&mut slice)?;
            transactions.push(tx);
        }

        Ok(Self {
            timestamp: value.timestamp,
            prev_randao: value.prev_randao,
            suggested_fee_recipient: value.suggested_fee_recipient,
            withdrawals: value.withdrawals,
            parent_beacon_block_root: value.parent_beacon_block_root,
            transactions,
            gas_limit: value.gas_limit,
        })
    }
}

pub type Withdrawal = alloy::rpc::types::Withdrawal;

#[derive(Debug)]
pub enum Command {
    StartBlockBuild {
        payload_attributes: PayloadForExecution,
        payload_id: PayloadId,
    },
    AddTransaction {
        tx: NormalizedEthTransaction,
    },
    GenesisUpdate {
        block: ExtendedBlock,
    },
}

pub type RpcBlock = alloy::rpc::types::Block<RpcTransaction>;
pub type RpcTransaction = op_alloy::rpc_types::Transaction;

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

pub trait ToPayloadIdInput<'a> {
    fn to_payload_id_input(&'a self, head: &'a B256) -> NewPayloadIdInput<'a>;
}

impl<'a> ToPayloadIdInput<'a> for PayloadForExecution {
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
    fn with_payload_attributes(self, payload: PayloadForExecution) -> Self;
}

impl WithPayloadAttributes for Header {
    fn with_payload_attributes(self, payload: PayloadForExecution) -> Self {
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
