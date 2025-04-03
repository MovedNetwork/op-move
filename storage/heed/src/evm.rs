use {
    crate::evm_storage_trie::HeedEthStorageTrieDb,
    eth_trie::{DB, TrieError},
    moved_evm_ext::state::{self, BoxedTrieDb, DbWithRoot, EthTrieDbWithLocalError, StorageTrieDb},
    moved_shared::primitives::{Address, B256},
    moved_trie::StagingEthTrieDb,
    std::{
        error,
        fmt::{Display, Formatter},
        sync::Arc,
    },
};

#[derive(Debug)]
pub struct Error(heed::Error);

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl error::Error for Error {}

impl From<Error> for state::Error {
    fn from(value: Error) -> Self {
        state::Error::EthTrie(TrieError::DB(value.0.to_string()))
    }
}

impl From<heed::Error> for Error {
    fn from(value: heed::Error) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub struct HeedStorageTrieRepository {
    env: &'static heed::Env,
}

impl HeedStorageTrieRepository {
    pub fn new(env: &'static heed::Env) -> Self {
        Self { env }
    }
}

impl StorageTrieDb for HeedStorageTrieRepository {
    fn db(&self, account: Address) -> Arc<StagingEthTrieDb<BoxedTrieDb>> {
        let db = HeedEthStorageTrieDb::new(self.env, account);

        Arc::new(StagingEthTrieDb::new(BoxedTrieDb::new(
            EthTrieDbWithLocalError::new(EthTrieDbWithHeedError::new(db)),
        )))
    }
}

pub struct EthTrieDbWithHeedError<T: DB>(pub T);

impl<T: DB> EthTrieDbWithHeedError<T> {
    pub fn new(db: T) -> Self {
        Self(db)
    }
}

impl<E, T: DB<Error = E> + DbWithRoot> DbWithRoot for EthTrieDbWithHeedError<T>
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

impl<E, T: DB<Error = E>> DB for EthTrieDbWithHeedError<T>
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

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        Ok(self.0.insert_batch(keys, values)?)
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        Ok(self.0.remove(key)?)
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(self.0.flush()?)
    }
}
