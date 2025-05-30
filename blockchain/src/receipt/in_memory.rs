use {
    crate::receipt::{
        ExtendedReceipt, ReceiptQueries, TransactionReceipt, write::ReceiptRepository,
    },
    std::{
        convert::Infallible,
        hash::{Hash, Hasher},
        sync::Arc,
    },
    umi_shared::primitives::B256,
};

pub type ReadHandle = evmap::ReadHandle<B256, Arc<ExtendedReceipt>>;
pub type WriteHandle = evmap::WriteHandle<B256, Arc<ExtendedReceipt>>;

impl Hash for ExtendedReceipt {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_hash.hash(state);
        self.transaction_index.hash(state);
        self.to.hash(state);
        self.from.hash(state);
        self.gas_used.hash(state);
        self.l2_gas_price.hash(state);
        self.contract_address.hash(state);
        self.logs_offset.hash(state);
        self.block_hash.hash(state);
        self.block_number.hash(state);
        self.block_timestamp.hash(state);
    }
}

#[derive(Debug)]
pub struct ReceiptMemory {
    receipts: WriteHandle,
}

impl ReceiptMemory {
    pub fn new(receipts: WriteHandle) -> Self {
        Self { receipts }
    }

    pub fn extend(&mut self, receipts: impl IntoIterator<Item = ExtendedReceipt>) {
        self.receipts.extend(
            receipts
                .into_iter()
                .map(|receipt| (receipt.transaction_hash, Arc::new(receipt))),
        );
        self.receipts.refresh();
    }
}

impl AsRef<ReadHandle> for ReceiptMemory {
    fn as_ref(&self) -> &ReadHandle {
        &self.receipts
    }
}

#[derive(Debug, Clone)]
pub struct ReceiptMemoryReader {
    receipts: ReadHandle,
}

impl ReceiptMemoryReader {
    pub fn new(receipts: ReadHandle) -> Self {
        Self { receipts }
    }
}

impl AsRef<ReadHandle> for ReceiptMemoryReader {
    fn as_ref(&self) -> &ReadHandle {
        &self.receipts
    }
}

pub trait ReadReceiptMemory {
    fn contains(&self, transaction_hash: B256) -> bool;
    fn by_transaction_hash(&self, transaction_hash: B256) -> Option<ExtendedReceipt>;
}

impl<T: AsRef<ReadHandle>> ReadReceiptMemory for T {
    fn contains(&self, transaction_hash: B256) -> bool {
        self.as_ref().contains_key(&transaction_hash)
    }

    fn by_transaction_hash(&self, transaction_hash: B256) -> Option<ExtendedReceipt> {
        self.as_ref()
            .get_one(&transaction_hash)
            .map(|v| ExtendedReceipt::clone(&v))
    }
}

pub mod receipt_memory {
    use crate::receipt::{ReceiptMemory, ReceiptMemoryReader};

    pub fn new() -> (ReceiptMemoryReader, ReceiptMemory) {
        let (r, w) = evmap::new();

        (ReceiptMemoryReader::new(r), ReceiptMemory::new(w))
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryReceiptQueries;

impl Default for InMemoryReceiptQueries {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryReceiptQueries {
    pub fn new() -> Self {
        Self
    }
}

impl ReceiptQueries for InMemoryReceiptQueries {
    type Err = Infallible;
    type Storage = ReceiptMemoryReader;

    fn by_transaction_hash(
        &self,
        storage: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err> {
        Ok(storage
            .by_transaction_hash(transaction_hash)
            .map(TransactionReceipt::from))
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryReceiptRepository;

impl Default for InMemoryReceiptRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryReceiptRepository {
    pub fn new() -> Self {
        Self
    }
}

impl ReceiptRepository for InMemoryReceiptRepository {
    type Err = Infallible;
    type Storage = ReceiptMemory;

    fn contains(&self, storage: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err> {
        Ok(storage.contains(transaction_hash))
    }

    fn extend(
        &self,
        storage: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err> {
        storage.extend(receipts);
        Ok(())
    }
}
