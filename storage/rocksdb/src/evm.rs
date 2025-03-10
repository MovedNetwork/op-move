use {
    moved_evm_ext::storage::{StorageTrie, StorageTrieRepository},
    moved_shared::primitives::{Address, B256},
};

pub struct RocksDbStorageTrieRepository;

impl RocksDbStorageTrieRepository {
    pub fn new() -> Self {
        Self
    }
}

impl StorageTrieRepository for RocksDbStorageTrieRepository {
    fn for_account(&self, account: &Address) -> StorageTrie {
        todo!()
    }

    fn by_root(&self, storage_root: &B256) -> StorageTrie {
        todo!()
    }
}
