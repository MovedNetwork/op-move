use {
    alloy::{primitives::keccak256, rlp},
    auto_impl::auto_impl,
    eth_trie::{EthTrie, MemDBError, MemoryDB, Trie, TrieError, DB},
    moved_shared::primitives::{Address, B256, U256},
    std::{collections::HashMap, convert::Infallible, error, fmt::Debug, result, sync::Arc},
    thiserror::Error,
};

/// [`result::Result`] with its `Err` variant set to [`Error`].
type Result<T> = result::Result<T, Error>;

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
}

#[derive(Default)]
pub struct InMemoryStorageTrieRepository {
    accounts: HashMap<Address, B256>,
    storages: HashMap<B256, Arc<BoxedTrieDb>>,
}

impl InMemoryStorageTrieRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StorageTrieRepository for InMemoryStorageTrieRepository {
    fn for_account(&self, account: &Address) -> StorageTrie {
        if let Some((db, storage_root)) = self
            .accounts
            .get(account)
            .and_then(|index| self.storages.get(index).cloned().map(|v| (v, *index)))
        {
            StorageTrie::from(db, storage_root).unwrap()
        } else {
            StorageTrie::new(Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                MemoryDB::new(false),
            ))))
        }
    }

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> StorageTrie {
        if let Some(db) = self.storages.get(storage_root).cloned() {
            StorageTrie::from(db, *storage_root).unwrap()
        } else {
            StorageTrie::new(Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                MemoryDB::new(false),
            ))))
        }
    }
}

impl InMemoryStorageTrieRepository {
    pub fn apply(&mut self, account: Address, storage_root: B256) {
        if let Some(root) = self.accounts.get_mut(&account) {
            if let Some(storage) = self.storages.get(root) {
                self.storages.insert(storage_root, storage.clone());
            }
            self.storages.remove(root);
            *root = storage_root;
        }
    }
}

pub struct EthTrieDbWithLocalError<T>(T);

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
}
