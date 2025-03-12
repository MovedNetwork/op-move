use {
    crate::evm_storage_trie::RocksEthStorageTrieDb,
    eth_trie::{TrieError, DB},
    moved_evm_ext::{
        storage,
        storage::{BoxedTrieDb, DbWithRoot, EthTrieDbWithLocalError, StorageTrieDb},
    },
    moved_shared::primitives::{Address, B256},
    std::{
        error,
        fmt::{Display, Formatter},
        sync::Arc,
    },
};

pub struct RocksDbStorageTrieRepository {
    db: &'static rocksdb::DB,
}

impl RocksDbStorageTrieRepository {
    pub fn new(db: &'static rocksdb::DB) -> Self {
        Self { db }
    }
}

impl StorageTrieDb for RocksDbStorageTrieRepository {
    fn db(&self, account: Address) -> Arc<BoxedTrieDb> {
        let db = RocksEthStorageTrieDb::new(self.db, account);

        Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
            EthTrieWithRocksDbError::new(db),
        )))
    }
}

#[derive(Debug)]
pub struct Error(rocksdb::Error);

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl error::Error for Error {}

impl From<Error> for storage::Error {
    fn from(value: Error) -> Self {
        storage::Error::EthTrie(TrieError::DB(value.0.to_string()))
    }
}

impl From<rocksdb::Error> for Error {
    fn from(value: rocksdb::Error) -> Self {
        Self(value)
    }
}

pub struct EthTrieWithRocksDbError<T: DB>(pub T);

impl<T: DB> EthTrieWithRocksDbError<T> {
    pub fn new(db: T) -> Self {
        Self(db)
    }
}

impl<E, T: DB<Error = E> + DbWithRoot> DbWithRoot for EthTrieWithRocksDbError<T>
where
    Error: From<E>,
{
    fn root(&self) -> Result<Option<B256>, Self::Error> {
        Ok(self.0.root()?)
    }

    fn put_root(&self, root: B256) -> Result<(), Self::Error> {
        Ok(self.0.put_root(root)?)
    }
}

impl<E, T: DB<Error = E>> DB for EthTrieWithRocksDbError<T>
where
    Error: From<E>,
{
    type Error = Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.0.get(key)?)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        Ok(self.0.insert(key, value)?)
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        Ok(self.0.remove(key)?)
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(self.0.flush()?)
    }
}
