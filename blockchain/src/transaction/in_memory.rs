use {
    crate::transaction::ExtendedTransaction,
    moved_shared::primitives::B256,
    std::{ops::Deref, sync::Arc},
};

pub type ReadHandle = evmap::ReadHandle<B256, Arc<ExtendedTransaction>>;
pub type WriteHandle = evmap::WriteHandle<B256, Arc<ExtendedTransaction>>;

#[derive(Debug, Clone)]
pub struct TransactionMemoryReader {
    transactions: ReadHandle,
}

impl TransactionMemoryReader {
    pub fn new(transactions: ReadHandle) -> Self {
        Self { transactions }
    }
}

impl AsRef<ReadHandle> for TransactionMemoryReader {
    fn as_ref(&self) -> &ReadHandle {
        &self.transactions
    }
}

#[derive(Debug)]
pub struct TransactionMemory {
    transactions: WriteHandle,
}

impl TransactionMemory {
    pub fn new(transactions: WriteHandle) -> Self {
        Self { transactions }
    }

    pub fn extend(&mut self, tx: impl IntoIterator<Item = ExtendedTransaction>) {
        self.transactions
            .extend(tx.into_iter().map(|tx| (tx.hash(), Arc::new(tx))));
    }
}

impl AsRef<ReadHandle> for TransactionMemory {
    fn as_ref(&self) -> &ReadHandle {
        &self.transactions
    }
}

pub trait ReadTransactionMemory {
    fn by_hash(&self, hash: B256) -> Option<ExtendedTransaction>;
    fn by_hashes(&self, hashes: impl IntoIterator<Item = B256>) -> Vec<ExtendedTransaction>;
}

impl<T: AsRef<ReadHandle>> ReadTransactionMemory for T {
    fn by_hash(&self, hash: B256) -> Option<ExtendedTransaction> {
        self.as_ref()
            .get_one(&hash)
            .map(|v| ExtendedTransaction::clone(v.deref()))
    }

    fn by_hashes(&self, hashes: impl IntoIterator<Item = B256>) -> Vec<ExtendedTransaction> {
        hashes
            .into_iter()
            .filter_map(|hash| self.by_hash(hash))
            .collect()
    }
}
