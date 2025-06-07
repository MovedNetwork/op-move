use {
    eth_trie::DB,
    rocksdb::{AsColumnFamilyRef, DB as RocksDb, WriteBatchWithTransaction},
    std::sync::Arc,
    umi_evm_ext::state::DbWithRoot,
    umi_shared::primitives::{Address, B256},
};

pub const TRIE_COLUMN_FAMILY: &str = "evm_storage_trie";
pub const ROOT_COLUMN_FAMILY: &str = "evm_storage_trie_root";

pub struct RocksEthStorageTrieDb {
    db: Arc<RocksDb>,
    account: Address,
}

impl RocksEthStorageTrieDb {
    pub fn new(db: Arc<RocksDb>, account: Address) -> Self {
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

impl DbWithRoot for RocksEthStorageTrieDb {
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

impl DB for RocksEthStorageTrieDb {
    type Error = rocksdb::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let key = self.unique_key(key);
        self.db.get_cf(self.cf(), key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let key = self.unique_key(key);
        self.db.put_cf(self.cf(), key, value)
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        let cf = self.cf();

        self.db.write(keys.into_iter().zip(values).fold(
            WriteBatchWithTransaction::<false>::default(),
            |mut batch, (key, value)| {
                let key = self.unique_key(key.as_slice());
                batch.put_cf(cf, key, value);
                batch
            },
        ))
    }

    fn remove(&self, _key: &[u8]) -> Result<(), Self::Error> {
        // Intentionally ignored to not remove historical trie nodes
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        self.db.flush_cf(self.cf())
    }
}
