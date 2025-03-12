use {
    eth_trie::DB,
    moved_evm_ext::storage::DbWithRoot,
    moved_shared::primitives::{Address, B256},
    rocksdb::{AsColumnFamilyRef, DB as RocksDb},
};

pub const TRIE_COLUMN_FAMILY: &str = "evm_storage_trie";
pub const ROOT_COLUMN_FAMILY: &str = "evm_storage_trie_root";

pub struct RocksEthStorageTrieDb<'db> {
    db: &'db RocksDb,
    account: Address,
}

impl<'db> RocksEthStorageTrieDb<'db> {
    pub fn new(db: &'db RocksDb, account: Address) -> Self {
        Self { db, account }
    }

    fn unique_key(&self, key: &[u8]) -> Vec<u8> {
        [self.account.as_slice(), key].concat()
    }

    fn cf(&self) -> &impl AsColumnFamilyRef {
        self.db
            .cf_handle(TRIE_COLUMN_FAMILY)
            .expect("Column family should exist")
    }

    fn root_cf(&self) -> &impl AsColumnFamilyRef {
        self.db
            .cf_handle(ROOT_COLUMN_FAMILY)
            .expect("Column family should exist")
    }
}

impl<'db> DbWithRoot for RocksEthStorageTrieDb<'db> {
    fn root(&self) -> Result<Option<B256>, rocksdb::Error> {
        Ok(self
            .db
            .get_cf(self.root_cf(), self.account.as_slice())?
            .map(|v| B256::new(v.try_into().unwrap())))
    }

    fn put_root(&self, root: B256) -> Result<(), rocksdb::Error> {
        self.db
            .put_cf(self.root_cf(), self.account.as_slice(), root.as_slice())
    }
}

impl<'db> DB for RocksEthStorageTrieDb<'db> {
    type Error = rocksdb::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let key = self.unique_key(key);
        self.db.get_cf(self.cf(), key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let key = self.unique_key(key);
        self.db.put_cf(self.cf(), key, value)
    }

    fn remove(&self, _key: &[u8]) -> Result<(), Self::Error> {
        // Intentionally ignored to not remove historical trie nodes
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        self.db.flush_cf(self.cf())
    }
}
