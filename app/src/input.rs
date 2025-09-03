use {
    alloy::{primitives::Bloom, rlp::Decodable},
    op_alloy::consensus::OpTxEnvelope,
    umi_blockchain::{
        block::Header,
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
    #[cfg(feature = "op-upgrade")]
    pub eip1559_params: Option<U64>,
    pub no_tx_pool: Option<bool>,
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
    #[cfg(feature = "op-upgrade")]
    pub eip1559_params: Option<umi_blockchain::block::BaseFeeParameters>,
    pub no_tx_pool: Option<bool>,
}

impl TryFrom<Payload> for PayloadForExecution {
    type Error = umi_shared::error::Error;

    fn try_from(value: Payload) -> Result<Self, Self::Error> {
        let mut transactions = Vec::with_capacity(value.transactions.len());

        for raw_tx in value.transactions {
            let mut slice: &[u8] = raw_tx.as_ref();
            let op_tx = OpTxEnvelope::decode(&mut slice)?;
            transactions.push(op_tx.try_into()?);
        }

        #[cfg(feature = "op-upgrade")]
        let parsed_params = value
            .eip1559_params
            .map(|params| {
                // The first [0, 4) bytes are base fee denominator
                let denominator = params.wrapping_shr(32).saturating_to::<u32>();
                // The bottom 4 bytes reserved for elasticity
                let elasticity = (params.bitand(U64::from(0xFFFF_FFFFu64))).saturating_to::<u32>();

                if elasticity != 0 && denominator == 0 {
                    return Err(Self::Error::fee_denom_invariant_violation());
                }
                Ok(umi_blockchain::block::BaseFeeParameters {
                    denominator,
                    elasticity,
                })
            })
            .transpose()?;

        Ok(Self {
            timestamp: value.timestamp,
            prev_randao: value.prev_randao,
            suggested_fee_recipient: value.suggested_fee_recipient,
            withdrawals: value.withdrawals,
            parent_beacon_block_root: value.parent_beacon_block_root,
            transactions,
            gas_limit: value.gas_limit,
            #[cfg(feature = "op-upgrade")]
            eip1559_params: parsed_params,
            no_tx_pool: value.no_tx_pool,
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
        #[cfg_attr(not(feature = "op-upgrade"), allow(unused_mut))]
        let mut input = NewPayloadIdInput::new_v3(
            head,
            self.timestamp.into_limbs()[0],
            &self.prev_randao,
            &self.suggested_fee_recipient,
            self.gas_limit.into_limbs()[0],
        )
        .with_beacon_root(&self.parent_beacon_block_root)
        .with_withdrawals(
            self.withdrawals
                .iter()
                .map(ToWithdrawal::to_withdrawal)
                .collect::<Vec<_>>(),
        )
        .with_transaction_hashes(self.transactions.iter().map(|tx| tx.tx_hash()));
        #[cfg(feature = "op-upgrade")]
        {
            if let Some(eip1559_params) = &self.eip1559_params {
                input = input.with_eip1559_params(eip1559_params);
            }
        }

        input
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
