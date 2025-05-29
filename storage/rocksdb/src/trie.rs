use {
    eth_trie::{DB, EthTrie, TrieError},
    rocksdb::{AsColumnFamilyRef, DB as RocksDb, WriteBatchWithTransaction},
    std::sync::Arc,
    umi_evm_ext::state::DbWithRoot,
    umi_shared::primitives::B256,
};

pub const TRIE_COLUMN_FAMILY: &str = "trie";
pub const ROOT_COLUMN_FAMILY: &str = "trie_root";
pub const ROOT_KEY: &str = "trie_root";

pub struct RocksEthTrieDb<'db> {
    db: &'db RocksDb,
}

impl<'db> RocksEthTrieDb<'db> {
    pub fn new(db: &'db RocksDb) -> Self {
        Self { db }
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

impl DbWithRoot for RocksEthTrieDb<'_> {
    fn root(&self) -> Result<Option<B256>, rocksdb::Error> {
        Ok(self
            .db
            .get_cf(self.root_cf(), ROOT_KEY)?
            .map(|v| B256::new(v.try_into().unwrap())))
    }

    fn put_root(&self, root: B256) -> Result<(), rocksdb::Error> {
        self.db.put_cf(self.root_cf(), ROOT_KEY, root.as_slice())
    }
}

impl DB for RocksEthTrieDb<'_> {
    type Error = rocksdb::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        self.db.get_cf(self.cf(), key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        self.db.put_cf(self.cf(), key, value)
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        let cf = self.cf();

        self.db.write(keys.into_iter().zip(values).fold(
            WriteBatchWithTransaction::<false>::default(),
            |mut batch, (key, value)| {
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

pub trait TryFromOptRoot<D> {
    fn try_from_opt_root(db: Arc<D>, root: Option<B256>) -> Result<Self, TrieError>
    where
        Self: Sized;
}

impl<D: DB> TryFromOptRoot<D> for EthTrie<D> {
    fn try_from_opt_root(db: Arc<D>, root: Option<B256>) -> Result<EthTrie<D>, TrieError> {
        match root {
            None => Ok(EthTrie::new(db)),
            Some(root) => EthTrie::from(db, root),
        }
    }
}

pub trait FromOptRoot<D> {
    fn from_opt_root(db: Arc<D>, root: Option<B256>) -> Self
    where
        Self: Sized;
}

impl<D, T: TryFromOptRoot<D>> FromOptRoot<D> for T {
    fn from_opt_root(db: Arc<D>, root: Option<B256>) -> Self {
        Self::try_from_opt_root(db, root).expect("Root node should exist")
    }
}
