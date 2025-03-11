use {
    alloy::{primitives::keccak256, rlp},
    auto_impl::auto_impl,
    eth_trie::{EthTrie, MemDBError, MemoryDB, RootWithTrieDiff, Trie, TrieError, DB},
    moved_shared::primitives::{Address, B256, U256},
    std::{collections::HashMap, fmt::Debug, ops::Add, result, sync::Arc},
    thiserror::Error,
};

/// [`result::Result`] with its `Err` variant set to [`Error`].
pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    EthTrie(#[from] TrieError),
    #[error("{0}")]
    Rlp(#[from] rlp::Error),
}

impl From<MemDBError> for Error {
    fn from(value: MemDBError) -> Self {
        Self::EthTrie(TrieError::DB(value.to_string()))
    }
}

pub struct StorageTrie(pub EthTrie<BoxedTrieDb>);

#[auto_impl(Box)]
pub trait StorageTrieRepository {
    fn for_account(&self, account: &Address) -> StorageTrie;

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> StorageTrie;

    // todo: do not include here
    fn apply(&mut self, changes: StorageTriesChanges) -> Result<()>;
}

pub struct BoxedTrieDb(pub Box<dyn DB<Error = Error>>);

impl BoxedTrieDb {
    pub fn new(db: impl DB<Error = Error> + 'static) -> Self {
        Self(Box::new(db))
    }
}

impl DB for BoxedTrieDb {
    type Error = Error;

    fn get(&self, key: &[u8]) -> result::Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.0.get(key)?)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> result::Result<(), Self::Error> {
        Ok(self.0.insert(key, value)?)
    }

    fn remove(&self, key: &[u8]) -> result::Result<(), Self::Error> {
        Ok(self.0.remove(key)?)
    }

    fn flush(&self) -> result::Result<(), Self::Error> {
        Ok(self.0.flush()?)
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
        self.trie_diff.extend(rhs.trie_diff.into_iter());
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
    pub fn new(db: Arc<BoxedTrieDb>) -> Self {
        Self(EthTrie::new(db))
    }

    pub fn from(db: Arc<BoxedTrieDb>, root: B256) -> result::Result<Self, TrieError> {
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
            .insert_batch(keys, values)
            .map_err(|e| TrieError::DB(e.to_string()))?;

        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryStorageTrieRepository {
    accounts: HashMap<Address, (Arc<BoxedTrieDb>, B256)>,
}

impl InMemoryStorageTrieRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StorageTrieRepository for InMemoryStorageTrieRepository {
    fn for_account(&self, account: &Address) -> StorageTrie {
        if let Some((db, storage_root)) = self.accounts.get(account).cloned() {
            StorageTrie::from(db, storage_root).unwrap()
        } else {
            StorageTrie::new(Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                MemoryDB::new(false),
            ))))
        }
    }

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> StorageTrie {
        if let Some(db) = self.accounts.get(account).map(|(db, _)| db).cloned() {
            StorageTrie::from(db, *storage_root).unwrap()
        } else {
            StorageTrie::new(Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                MemoryDB::new(false),
            ))))
        }
    }

    fn apply(&mut self, changes: StorageTriesChanges) -> Result<()> {
        for (account, changes) in changes {
            let storage_root = changes.root;
            let storage_trie = self.for_account(&account);
            storage_trie.apply(changes)?;
            self.replace(account, storage_root, storage_trie.0.db);
        }
        Ok(())
    }
}

impl InMemoryStorageTrieRepository {
    pub fn replace(&mut self, account: Address, storage_root: B256, storage: Arc<BoxedTrieDb>) {
        self.accounts.insert(account, (storage, storage_root));
    }
}

pub struct EthTrieDbWithLocalError<T>(pub T);

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

    fn remove(&self, key: &[u8]) -> result::Result<(), Self::Error> {
        Ok(self.0.remove(key)?)
    }

    fn flush(&self) -> result::Result<(), Self::Error> {
        Ok(self.0.flush()?)
    }
}

impl StorageTrieRepository for () {
    fn for_account(&self, _: &Address) -> StorageTrie {
        todo!()
    }

    fn for_account_with_root(&self, _: &Address, _: &B256) -> StorageTrie {
        todo!()
    }

    fn apply(&mut self, _: StorageTriesChanges) -> Result<()> {
        todo!()
    }
}
