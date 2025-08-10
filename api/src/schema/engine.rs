//! See https://github.com/ethereum/execution-apis/blob/main/src/engine/
//! for specification of types.

use {
    serde::{Deserialize, Serialize},
    std::{
        fmt::{Debug, Display, Formatter},
        str::FromStr,
    },
    umi_app::Payload,
    umi_blockchain::payload::{BlobsBundle, ExecutionPayload, PayloadResponse, Withdrawal},
    umi_shared::primitives::{Address, B256, B2048, Bytes, U64, U256},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalV1 {
    pub index: U64,
    pub validator_index: U64,
    pub address: Address,
    pub amount: U64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV3 {
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
    pub withdrawals: Vec<WithdrawalV1>,
    pub blob_gas_used: U64,
    pub excess_blob_gas: U64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ForkchoiceStateV1 {
    pub head_block_hash: B256,
    pub safe_block_hash: B256,
    pub finalized_block_hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PayloadAttributesV3 {
    pub timestamp: U64,
    pub prev_randao: B256,
    pub suggested_fee_recipient: Address,
    pub withdrawals: Vec<WithdrawalV1>,
    pub parent_beacon_block_root: B256,
    pub transactions: Vec<Bytes>,
    pub gas_limit: U64,
    // TODO: (#201) allowed to be non-null only post-Holocene:
    // <https://specs.optimism.io/protocol/holocene/exec-engine.html#dynamic-eip-1559-parameters>.
    // Currently not propagated.
    pub eip1559_params: Option<U64>,
    pub no_tx_pool: Option<bool>,
}

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String")]
pub struct PayloadId(pub U64);

impl PayloadId {
    pub const fn new(id: u64) -> Self {
        Self(U64::from_limbs([id]))
    }
}

impl Display for PayloadId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0.as_limbs()[0])
    }
}

impl Debug for PayloadId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0.as_limbs()[0])
    }
}

impl FromStr for PayloadId {
    type Err = <U64 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(U64::from_str(s)?))
    }
}

impl<U: Into<u64>> From<U> for PayloadId {
    fn from(value: U) -> Self {
        Self(U64::from_limbs([value.into()]))
    }
}

impl From<PayloadId> for U64 {
    fn from(value: PayloadId) -> Self {
        value.0
    }
}

impl From<PayloadId> for String {
    fn from(value: PayloadId) -> Self {
        let inner: u64 = value.0.into_limbs()[0];
        format!("{inner:#018x}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadStatusV1 {
    pub status: Status,
    pub latest_valid_hash: Option<B256>,
    #[serde(default)]
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Valid,
    Invalid,
    Syncing,
    Accepted,
    InvalidBlockHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BlobsBundleV1 {
    pub commitments: Vec<Bytes>,
    pub proofs: Vec<Bytes>,
    pub blobs: Vec<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkchoiceUpdatedResponseV1 {
    pub payload_status: PayloadStatusV1,
    pub payload_id: Option<PayloadId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPayloadResponseV3 {
    pub execution_payload: ExecutionPayloadV3,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundleV1,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: B256,
}

impl From<GetPayloadResponseV3> for PayloadResponse {
    fn from(value: GetPayloadResponseV3) -> Self {
        Self {
            execution_payload: value.execution_payload.into(),
            block_value: value.block_value,
            blobs_bundle: value.blobs_bundle.into(),
            should_override_builder: value.should_override_builder,
            parent_beacon_block_root: value.parent_beacon_block_root,
        }
    }
}

impl From<PayloadResponse> for GetPayloadResponseV3 {
    fn from(value: PayloadResponse) -> Self {
        Self {
            execution_payload: value.execution_payload.into(),
            block_value: value.block_value,
            blobs_bundle: value.blobs_bundle.into(),
            should_override_builder: value.should_override_builder,
            parent_beacon_block_root: value.parent_beacon_block_root,
        }
    }
}

impl From<BlobsBundleV1> for BlobsBundle {
    fn from(value: BlobsBundleV1) -> Self {
        Self {
            commitments: value.commitments,
            proofs: value.proofs,
            blobs: value.blobs,
        }
    }
}

impl From<BlobsBundle> for BlobsBundleV1 {
    fn from(value: BlobsBundle) -> Self {
        Self {
            commitments: value.commitments,
            proofs: value.proofs,
            blobs: value.blobs,
        }
    }
}

impl From<ExecutionPayloadV3> for ExecutionPayload {
    fn from(value: ExecutionPayloadV3) -> Self {
        Self {
            parent_hash: value.parent_hash,
            fee_recipient: value.fee_recipient,
            state_root: value.state_root,
            receipts_root: value.receipts_root,
            logs_bloom: value.logs_bloom,
            prev_randao: value.prev_randao,
            block_number: value.block_number,
            gas_limit: value.gas_limit,
            gas_used: value.gas_used,
            timestamp: value.timestamp,
            extra_data: value.extra_data,
            base_fee_per_gas: value.base_fee_per_gas,
            block_hash: value.block_hash,
            transactions: value.transactions,
            withdrawals: value.withdrawals.into_iter().map(Into::into).collect(),
            blob_gas_used: value.blob_gas_used,
            excess_blob_gas: value.excess_blob_gas,
        }
    }
}

impl From<ExecutionPayload> for ExecutionPayloadV3 {
    fn from(value: ExecutionPayload) -> Self {
        Self {
            parent_hash: value.parent_hash,
            fee_recipient: value.fee_recipient,
            state_root: value.state_root,
            receipts_root: value.receipts_root,
            logs_bloom: value.logs_bloom,
            prev_randao: value.prev_randao,
            block_number: value.block_number,
            gas_limit: value.gas_limit,
            gas_used: value.gas_used,
            timestamp: value.timestamp,
            extra_data: value.extra_data,
            base_fee_per_gas: value.base_fee_per_gas,
            block_hash: value.block_hash,
            transactions: value.transactions,
            withdrawals: value.withdrawals.into_iter().map(Into::into).collect(),
            blob_gas_used: value.blob_gas_used,
            excess_blob_gas: value.excess_blob_gas,
        }
    }
}

impl From<WithdrawalV1> for Withdrawal {
    fn from(value: WithdrawalV1) -> Self {
        Self {
            index: value.index.saturating_to(),
            validator_index: value.validator_index.saturating_to(),
            address: value.address,
            amount: value.amount.saturating_to(),
        }
    }
}

impl From<Withdrawal> for WithdrawalV1 {
    fn from(value: Withdrawal) -> Self {
        Self {
            index: U64::from(value.index),
            validator_index: U64::from(value.validator_index),
            address: value.address,
            amount: U64::from(value.amount),
        }
    }
}

impl From<PayloadAttributesV3> for Payload {
    fn from(value: PayloadAttributesV3) -> Self {
        Self {
            timestamp: value.timestamp,
            prev_randao: value.prev_randao,
            suggested_fee_recipient: value.suggested_fee_recipient,
            withdrawals: value.withdrawals.into_iter().map(Into::into).collect(),
            parent_beacon_block_root: value.parent_beacon_block_root,
            transactions: value.transactions,
            gas_limit: value.gas_limit,
            no_tx_pool: value.no_tx_pool,
        }
    }
}
