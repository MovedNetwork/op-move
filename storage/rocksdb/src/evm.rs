use {
    moved_evm_ext::storage::{BoxedTrieDb, StorageTrieDb},
    moved_shared::primitives::Address,
    std::sync::Arc,
};

pub struct RocksDbStorageTrieRepository;

impl Default for RocksDbStorageTrieRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl RocksDbStorageTrieRepository {
    pub fn new() -> Self {
        Self
    }
}

impl StorageTrieDb for RocksDbStorageTrieRepository {
    fn db(&self, _account: Address) -> Arc<BoxedTrieDb> {
        todo!()
    }
}
