use {
    moved_evm_ext::storage::{StorageTrie, StorageTrieRepository},
    moved_shared::primitives::{Address, B256},
};

pub struct HeedStorageTrieRepository;

impl HeedStorageTrieRepository {
    pub fn new() -> Self {
        Self
    }
}

impl StorageTrieRepository for HeedStorageTrieRepository {
    fn for_account(&self, account: &Address) -> StorageTrie {
        todo!()
    }

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> StorageTrie {
        todo!()
    }
}
