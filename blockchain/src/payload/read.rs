use {
    crate::{block::ExtendedBlock, payload::id::PayloadId},
    alloy::eips::eip2718::Encodable2718,
    op_alloy::consensus::OpTxEnvelope,
    std::{
        collections::hash_map::{Entry, HashMap},
        fmt::Debug,
        sync::{Arc, RwLock},
    },
    tokio::sync::broadcast,
    umi_shared::primitives::{Address, B256, B2048, Bytes, U64, U256},
};

pub type Withdrawal = alloy::rpc::types::Withdrawal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadResponse {
    pub execution_payload: ExecutionPayload,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundle,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: B256,
}

impl PayloadResponse {
    pub fn from_block_with_transactions(
        block: ExtendedBlock,
        transactions: impl IntoIterator<Item = OpTxEnvelope>,
    ) -> Self {
        Self {
            parent_beacon_block_root: block
                .block
                .header
                .parent_beacon_block_root
                .unwrap_or_default(),
            block_value: block.value,
            execution_payload: ExecutionPayload::from_block_with_transactions(block, transactions),
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
    pub fn from_block_with_transactions(
        block: ExtendedBlock,
        transactions: impl IntoIterator<Item = OpTxEnvelope>,
    ) -> Self {
        let transactions = transactions
            .into_iter()
            .map(|tx| {
                let capacity = tx.eip2718_encoded_length();
                let mut bytes = Vec::with_capacity(capacity);
                tx.encode_2718(&mut bytes);
                bytes.into()
            })
            .collect();

        Self {
            block_hash: block.hash,
            parent_hash: block.block.header.parent_hash,
            fee_recipient: block.block.header.beneficiary,
            state_root: block.block.header.state_root,
            receipts_root: block.block.header.receipts_root,
            logs_bloom: block.block.header.logs_bloom.0,
            prev_randao: block.block.header.mix_hash,
            block_number: U64::from(block.block.header.number),
            gas_limit: U64::from(block.block.header.gas_limit),
            gas_used: U64::from(block.block.header.gas_used),
            timestamp: U64::from(block.block.header.timestamp),
            extra_data: block.block.header.extra_data,
            base_fee_per_gas: U256::from(block.block.header.base_fee_per_gas.unwrap_or_default()),
            transactions,
            withdrawals: Vec::new(), // TODO: withdrawals
            blob_gas_used: U64::from(block.block.header.blob_gas_used.unwrap_or_default()),
            excess_blob_gas: U64::from(block.block.header.excess_blob_gas.unwrap_or_default()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlobsBundle {
    pub commitments: Vec<Bytes>,
    pub proofs: Vec<Bytes>,
    pub blobs: Vec<Bytes>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlreadyStarted;

pub enum MaybePayloadResponse {
    Unknown,
    Some(PayloadResponse),
    Delayed(broadcast::Receiver<PayloadResponse>),
}

impl MaybePayloadResponse {
    pub fn is_some(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    pub fn unwrap(self) -> PayloadResponse {
        match self {
            Self::Some(response) => response,
            Self::Unknown | Self::Delayed(_) => {
                panic!("Unwrap unknown or delayed payload response")
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct InProgressPayloads {
    inner: Arc<RwLock<HashMap<PayloadId, broadcast::Sender<PayloadResponse>>>>,
}

impl InProgressPayloads {
    pub fn get_delayed(&self, id: &PayloadId) -> Option<broadcast::Receiver<PayloadResponse>> {
        self.inner
            .read()
            .expect("Lock is not poisoned")
            .get(id)
            .map(|sender| sender.subscribe())
    }

    pub fn start_id(&self, id: PayloadId) -> Result<(), AlreadyStarted> {
        let mut in_progress = self.inner.write().expect("Lock is not poisoned");
        match in_progress.entry(id) {
            Entry::Vacant(entry) => {
                let (sender, _) = broadcast::channel(1);
                entry.insert(sender);
                Ok(())
            }
            Entry::Occupied(_) => Err(AlreadyStarted),
        }
    }

    pub fn finish_id(
        &self,
        block: ExtendedBlock,
        transactions: impl IntoIterator<Item = op_alloy::consensus::OpTxEnvelope>,
    ) {
        let id = block.payload_id;
        let response = PayloadResponse::from_block_with_transactions(block, transactions);
        if let Some(sender) = self
            .inner
            .write()
            .expect("Lock is not poisoned")
            .remove(&id)
        {
            sender.send(response).ok();
        }
    }
}

pub trait PayloadQueries {
    type Err: Debug;
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        block_hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err>;

    fn by_id(
        &self,
        storage: &Self::Storage,
        id: PayloadId,
    ) -> Result<MaybePayloadResponse, Self::Err>;

    fn get_in_progress(&self) -> InProgressPayloads;
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {super::*, std::convert::Infallible};

    impl PayloadQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_hash(
            &self,
            _: &Self::Storage,
            _: B256,
        ) -> Result<Option<PayloadResponse>, Self::Err> {
            Ok(None)
        }

        fn by_id(
            &self,
            _: &Self::Storage,
            _: PayloadId,
        ) -> Result<MaybePayloadResponse, Self::Err> {
            Ok(MaybePayloadResponse::Unknown)
        }

        fn get_in_progress(&self) -> InProgressPayloads {
            InProgressPayloads::default()
        }
    }
}
