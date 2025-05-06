use {crate::transaction::ExtendedTransaction, moved_shared::primitives::B256};

#[derive(Debug, Clone)]
pub struct TransactionMemoryReader {
    transactions: evmap::ReadHandle<B256, Box<ExtendedTransaction>>,
}

impl TransactionMemoryReader {
    pub fn new(transactions: evmap::ReadHandle<B256, Box<ExtendedTransaction>>) -> Self {
        Self { transactions }
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedTransaction> {
        self.transactions
            .get(&hash)
            .and_then(|v| v.iter().next().map(|v| *v.clone()))
    }

    pub fn by_hashes(&self, hashes: impl IntoIterator<Item = B256>) -> Vec<ExtendedTransaction> {
        hashes
            .into_iter()
            .filter_map(|hash| self.by_hash(hash))
            .collect()
    }
}

#[derive(Debug)]
pub struct TransactionMemory {
    transactions: evmap::WriteHandle<B256, Box<ExtendedTransaction>>,
}

impl TransactionMemory {
    pub fn new(transactions: evmap::WriteHandle<B256, Box<ExtendedTransaction>>) -> Self {
        Self { transactions }
    }

    pub fn extend(&mut self, tx: impl IntoIterator<Item = ExtendedTransaction>) {
        self.transactions
            .extend(tx.into_iter().map(|tx| (tx.hash(), Box::new(tx))));
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedTransaction> {
        self.transactions
            .get(&hash)
            .and_then(|v| v.iter().next().map(|v| *v.clone()))
    }

    pub fn by_hashes(&self, hashes: impl IntoIterator<Item = B256>) -> Vec<ExtendedTransaction> {
        hashes
            .into_iter()
            .filter_map(|hash| self.by_hash(hash))
            .collect()
    }
}
