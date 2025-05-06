use {
    crate::receipt::{
        ExtendedReceipt, ReceiptQueries, TransactionReceipt, write::ReceiptRepository,
    },
    moved_shared::primitives::B256,
    std::convert::Infallible,
};

#[derive(Debug)]
pub struct ReceiptMemory {
    receipts: evmap::WriteHandle<B256, Box<ExtendedReceipt>>,
}

impl ReceiptMemory {
    pub fn new(receipts: evmap::WriteHandle<B256, Box<ExtendedReceipt>>) -> Self {
        Self { receipts }
    }

    pub fn contains(&self, transaction_hash: B256) -> bool {
        self.receipts.contains_key(&transaction_hash)
    }

    pub fn extend(&mut self, receipts: impl IntoIterator<Item = ExtendedReceipt>) {
        self.receipts.extend(
            receipts
                .into_iter()
                .map(|receipt| (receipt.transaction_hash, Box::new(receipt))),
        );
    }

    pub fn by_transaction_hash(&self, transaction_hash: B256) -> Option<ExtendedReceipt> {
        self.receipts.get_one(&transaction_hash).map(|v| *v.clone())
    }
}

#[derive(Debug)]
pub struct ReceiptMemoryReader {
    receipts: evmap::ReadHandle<B256, Box<ExtendedReceipt>>,
}

impl ReceiptMemoryReader {
    pub fn new(receipts: evmap::ReadHandle<B256, Box<ExtendedReceipt>>) -> Self {
        Self { receipts }
    }

    pub fn contains(&self, transaction_hash: B256) -> bool {
        self.receipts.contains_key(&transaction_hash)
    }

    pub fn by_transaction_hash(&self, transaction_hash: B256) -> Option<ExtendedReceipt> {
        self.receipts.get_one(&transaction_hash).map(|v| *v.clone())
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
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
