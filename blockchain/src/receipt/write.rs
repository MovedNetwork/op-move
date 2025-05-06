use {
    moved_shared::primitives::{Address, B256, U256},
    op_alloy::{consensus::OpReceiptEnvelope, rpc_types::L1BlockInfo},
    std::fmt::Debug,
};

pub trait ReceiptRepository {
    type Err: Debug;
    type Storage;

    fn contains(&self, storage: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err>;

    fn extend(
        &self,
        storage: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err>;
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExtendedReceipt {
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub to: Option<Address>,
    pub from: Address,
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
    pub block_hash: B256,
    pub block_number: u64,
    pub block_timestamp: u64,
}

impl ExtendedReceipt {
    pub fn with_block_hash(mut self, block_hash: B256) -> Self {
        self.block_hash = block_hash;
        self
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {super::*, std::convert::Infallible};

    impl ReceiptRepository for () {
        type Err = Infallible;
        type Storage = ();

        fn contains(&self, _: &Self::Storage, _: B256) -> Result<bool, Self::Err> {
            Ok(false)
        }

        fn extend(
            &self,
            _: &mut Self::Storage,
            _: impl IntoIterator<Item = ExtendedReceipt>,
        ) -> Result<(), Self::Err> {
            Ok(())
        }
    }
}
