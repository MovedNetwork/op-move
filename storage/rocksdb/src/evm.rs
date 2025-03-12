use {
    moved_evm_ext::storage::{StorageTrie, StorageTrieRepository, StorageTriesChanges},
    moved_shared::primitives::{Address, B256},
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

impl StorageTrieRepository for RocksDbStorageTrieRepository {
    fn for_account(&self, account: &Address) -> StorageTrie {
        todo!()
    }

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> StorageTrie {
        todo!()
    }

    fn apply(&mut self, changes: StorageTriesChanges) -> Result<(), moved_evm_ext::storage::Error> {
        todo!()
    }
}
