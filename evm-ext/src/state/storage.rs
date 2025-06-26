use {
    alloy::{primitives::keccak256, rlp},
    auto_impl::auto_impl,
    eth_trie::{DB, EthTrie, MemDBError, MemoryDB, RootWithTrieDiff, Trie, TrieError},
    move_binary_format::errors::PartialVMError,
    std::{
        collections::HashMap,
        fmt::Debug,
        ops::Add,
        result,
        sync::{Arc, RwLock},
    },
    thiserror::Error,
    umi_shared::primitives::{Address, B256, U256},
    umi_trie::StagingEthTrieDb,
};

/// [`result::Result`] with its `Err` variant set to [`Error`].
pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    EthTrie(#[from] TrieError),
    #[error(transparent)]
    Rlp(#[from] rlp::Error),
    #[error("Address {0} is outside L2 range")]
    AddressOutsideRange(Address),
    #[error("VM execution failed with: {0}")]
    PartialVMError(#[from] PartialVMError),
    #[error("Account with address {0} not found")]
    AccountNotFound(Address),
    #[error("Failed to map height {0} to a state root")]
    UnknownBlockHeight(u64),
}

impl From<Error> for umi_shared::error::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::EthTrie(_) => umi_shared::error::Error::DatabaseState,
            Error::Rlp(e) => {
                umi_shared::error::Error::User(umi_shared::error::UserError::RLPError(e))
            }
            Error::PartialVMError(e) => {
                umi_shared::error::Error::User(umi_shared::error::UserError::PartialVm(e))
            }
            Error::AccountNotFound(address) | Error::AddressOutsideRange(address) => {
                umi_shared::error::Error::User(umi_shared::error::UserError::InvalidAddress(
                    address,
                ))
            }
            Error::UnknownBlockHeight(height) => umi_shared::error::Error::User(
                umi_shared::error::UserError::InvalidBlockHeight(height),
            ),
        }
    }
}

impl From<MemDBError> for Error {
    fn from(value: MemDBError) -> Self {
        Self::EthTrie(TrieError::DB(value.to_string()))
    }
}

pub struct StorageTrie(pub EthTrie<StagingEthTrieDb<BoxedTrieDb>>);

#[auto_impl(Box)]
pub trait StorageTrieDb {
    fn db(&self, account: Address) -> Arc<StagingEthTrieDb<BoxedTrieDb>>;
}

pub trait StorageTrieRepository {
    fn for_account(&self, account: &Address) -> Result<StorageTrie>;

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> Result<StorageTrie>;

    // TODO: move this out of repository
    fn apply(&self, changes: StorageTriesChanges) -> Result<()>;
}

impl<T: StorageTrieDb> StorageTrieRepository for T {
    fn for_account(&self, account: &Address) -> Result<StorageTrie> {
        let db = self.db(*account);

        Ok(if let Some(root) = db.root()? {
            StorageTrie::from(db, root)?
        } else {
            StorageTrie::new(db)
        })
    }

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> Result<StorageTrie> {
        let db = self.db(*account);

        Ok(StorageTrie::from(db, *storage_root)?)
    }

    fn apply(&self, changes: StorageTriesChanges) -> Result<()> {
        for (account, changes) in changes {
            self.for_account(&account)?.apply(changes)?;
        }
        Ok(())
    }
}

pub struct BoxedTrieDb(pub Box<dyn DbWithRoot<Error = Error>>);

impl BoxedTrieDb {
    pub fn new(db: impl DbWithRoot<Error = Error> + 'static) -> Self {
        Self(Box::new(db))
    }
}

pub trait DbWithRoot: DB {
    fn root(&self) -> result::Result<Option<B256>, Self::Error>;

    fn put_root(&self, root: B256) -> result::Result<(), Self::Error>;
}

impl DbWithRoot for BoxedTrieDb {
    fn root(&self) -> result::Result<Option<B256>, Self::Error> {
        self.0.root()
    }

    fn put_root(&self, root: B256) -> result::Result<(), Self::Error> {
        self.0.put_root(root)
    }
}

impl DB for BoxedTrieDb {
    type Error = Error;

    fn get(&self, key: &[u8]) -> result::Result<Option<Vec<u8>>, Self::Error> {
        self.0.get(key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> result::Result<(), Self::Error> {
        self.0.insert(key, value)
    }

    fn insert_batch(
        &self,
        keys: Vec<Vec<u8>>,
        values: Vec<Vec<u8>>,
    ) -> result::Result<(), Self::Error> {
        self.0.insert_batch(keys, values)
    }

    fn remove(&self, key: &[u8]) -> result::Result<(), Self::Error> {
        self.0.remove(key)
    }

    fn flush(&self) -> result::Result<(), Self::Error> {
        self.0.flush()
    }
}

#[derive(Debug, Clone)]
pub struct StorageTriesChanges {
    pub tries: HashMap<Address, StorageTrieChanges>,
}

impl IntoIterator for StorageTriesChanges {
    type Item = (Address, StorageTrieChanges);
    type IntoIter = <HashMap<Address, StorageTrieChanges> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.tries.into_iter()
    }
}

impl StorageTriesChanges {
    pub fn empty() -> Self {
        Self {
            tries: HashMap::new(),
        }
    }

    pub fn with_trie_changes(mut self, address: Address, changes: StorageTrieChanges) -> Self {
        let changes = match self.tries.remove(&address) {
            Some(old) => old + changes,
            None => changes,
        };
        self.tries.insert(address, changes);
        self
    }
}

#[derive(Debug, Clone)]
pub struct StorageTrieChanges {
    pub root: B256,
    pub trie_diff: HashMap<B256, Vec<u8>>,
}

impl Add for StorageTrieChanges {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.root = rhs.root;
        self.trie_diff.extend(rhs.trie_diff);
        self
    }
}

impl From<RootWithTrieDiff> for StorageTrieChanges {
    fn from(value: RootWithTrieDiff) -> Self {
        Self {
            root: value.root,
            trie_diff: value.trie_diff.into_iter().collect(),
        }
    }
}

impl StorageTrie {
    pub fn new(db: Arc<StagingEthTrieDb<BoxedTrieDb>>) -> Self {
        Self(EthTrie::new(db))
    }

    pub fn from(
        db: Arc<StagingEthTrieDb<BoxedTrieDb>>,
        root: B256,
    ) -> result::Result<Self, TrieError> {
        Ok(Self(EthTrie::from(db, root)?))
    }

    pub fn root_hash(&mut self) -> Result<B256> {
        Ok(self.0.root_hash()?)
    }

    pub fn proof(&mut self, key: &[u8]) -> Result<Vec<Vec<u8>>> {
        Ok(self.0.get_proof(key)?)
    }

    pub fn get(&self, index: &U256) -> Result<Option<U256>> {
        let trie_key = keccak256::<[u8; 32]>(index.to_be_bytes());
        let Some(bytes) = self.0.get(trie_key.as_slice())? else {
            return Ok(None);
        };

        Ok(Some(rlp::decode_exact(&bytes)?))
    }

    pub fn insert(&mut self, index: &U256, value: &U256) -> Result<()> {
        let trie_key = keccak256::<[u8; 32]>(index.to_be_bytes());

        if value.is_zero() {
            self.0.remove(trie_key.as_slice())?;
        } else {
            let value = rlp::encode_fixed_size(value);
            self.0.insert(trie_key.as_slice(), &value)?;
        }

        Ok(())
    }

    pub fn commit(&mut self) -> Result<StorageTrieChanges> {
        Ok(self.0.root_hash_with_changed_nodes().map(Into::into)?)
    }

    pub fn apply(&self, changes: StorageTrieChanges) -> Result<()> {
        let mut keys = Vec::with_capacity(changes.trie_diff.len());
        let mut values = Vec::with_capacity(changes.trie_diff.len());
        for (k, v) in changes.trie_diff.into_iter() {
            keys.push(k.to_vec());
            values.push(v);
        }

        self.0
            .db
            .inner
            .insert_batch(keys, values)
            .map_err(|e| TrieError::DB(e.to_string()))?;

        self.0.db.inner.put_root(changes.root)?;

        Ok(())
    }
}

pub struct InMemoryDb {
    root: RwLock<Option<B256>>,
    db: MemoryDB,
}

impl InMemoryDb {
    pub fn empty() -> Self {
        Self {
            root: RwLock::new(None),
            db: MemoryDB::new(false),
        }
    }
}

impl DB for InMemoryDb {
    type Error = <MemoryDB as DB>::Error;

    fn get(&self, key: &[u8]) -> result::Result<Option<Vec<u8>>, Self::Error> {
        self.db.get(key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> result::Result<(), Self::Error> {
        self.db.insert(key, value)
    }

    fn remove(&self, key: &[u8]) -> result::Result<(), Self::Error> {
        self.db.remove(key)
    }

    fn flush(&self) -> result::Result<(), Self::Error> {
        self.db.flush()
    }
}

impl DbWithRoot for InMemoryDb {
    fn root(&self) -> result::Result<Option<B256>, Self::Error> {
        Ok(*self.root.read().unwrap())
    }

    fn put_root(&self, root: B256) -> result::Result<(), Self::Error> {
        self.root.write().unwrap().replace(root);
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct InMemoryStorageTrieRepository {
    accounts: Arc<RwLock<HashMap<Address, Arc<StagingEthTrieDb<BoxedTrieDb>>>>>,
}

impl InMemoryStorageTrieRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Arc<StagingEthTrieDb<BoxedTrieDb>> {
        Arc::new(StagingEthTrieDb::new(BoxedTrieDb::new(
            EthTrieDbWithLocalError::new(InMemoryDb::empty()),
        )))
    }
}

impl StorageTrieDb for InMemoryStorageTrieRepository {
    fn db(&self, account: Address) -> Arc<StagingEthTrieDb<BoxedTrieDb>> {
        self.accounts
            .write()
            .unwrap()
            .entry(account)
            .or_insert_with(Self::create)
            .clone()
    }
}

pub struct EthTrieDbWithLocalError<T>(pub T);

impl<T> EthTrieDbWithLocalError<T> {
    pub fn new(db: T) -> Self {
        Self(db)
    }
}

impl<E, T: DbWithRoot<Error = E>> DbWithRoot for EthTrieDbWithLocalError<T>
where
    Error: From<E>,
{
    fn root(&self) -> result::Result<Option<B256>, Self::Error> {
        Ok(self.0.root()?)
    }

    fn put_root(&self, root: B256) -> result::Result<(), Self::Error> {
        Ok(self.0.put_root(root)?)
    }
}

impl<E, T: DbWithRoot<Error = E>> DbWithRoot for StagingEthTrieDb<T>
where
    Error: From<E>,
{
    fn root(&self) -> result::Result<Option<B256>, Self::Error> {
        self.inner.root()
    }

    fn put_root(&self, root: B256) -> result::Result<(), Self::Error> {
        self.inner.put_root(root)
    }
}

impl<E, T: DB<Error = E>> DB for EthTrieDbWithLocalError<T>
where
    Error: From<E>,
{
    type Error = Error;

    fn get(&self, key: &[u8]) -> result::Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.0.get(key)?)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> result::Result<(), Self::Error> {
        Ok(self.0.insert(key, value)?)
    }

    fn insert_batch(
        &self,
        keys: Vec<Vec<u8>>,
        values: Vec<Vec<u8>>,
    ) -> result::Result<(), Self::Error> {
        Ok(self.0.insert_batch(keys, values)?)
    }

    fn remove(&self, key: &[u8]) -> result::Result<(), Self::Error> {
        Ok(self.0.remove(key)?)
    }

    fn flush(&self) -> result::Result<(), Self::Error> {
        Ok(self.0.flush()?)
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    struct NoopEthTrieDb;

    impl DB for NoopEthTrieDb {
        type Error = Error;

        fn get(&self, _: &[u8]) -> result::Result<Option<Vec<u8>>, Self::Error> {
            Ok(None)
        }

        fn insert(&self, _: &[u8], _: Vec<u8>) -> result::Result<(), Self::Error> {
            Ok(())
        }

        fn remove(&self, _: &[u8]) -> result::Result<(), Self::Error> {
            Ok(())
        }

        fn flush(&self) -> result::Result<(), Self::Error> {
            Ok(())
        }
    }

    impl DbWithRoot for NoopEthTrieDb {
        fn root(&self) -> result::Result<Option<B256>, Self::Error> {
            Ok(None)
        }

        fn put_root(&self, _: B256) -> result::Result<(), Self::Error> {
            Ok(())
        }
    }

    impl StorageTrieDb for () {
        fn db(&self, _: Address) -> Arc<StagingEthTrieDb<BoxedTrieDb>> {
            Arc::new(StagingEthTrieDb::new(BoxedTrieDb::new(NoopEthTrieDb)))
        }
    }
}
