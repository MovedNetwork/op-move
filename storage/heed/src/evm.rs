use {
    crate::evm_storage_trie::HeedEthStorageTrieDb,
    eth_trie::{TrieError, DB},
    moved_evm_ext::{
        storage,
        storage::{
            BoxedTrieDb, EthTrieDbWithLocalError, StorageTrie, StorageTrieRepository,
            StorageTriesChanges,
        },
    },
    moved_shared::primitives::{Address, B256},
    std::{
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

impl std::error::Error for Error {}

impl From<Error> for storage::Error {
    fn from(value: Error) -> Self {
        storage::Error::EthTrie(TrieError::DB(value.0.to_string()))
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

    pub fn replace(&self, account: Address, storage_root: B256) -> Result<(), Error> {
        let db = HeedEthStorageTrieDb::new(self.env, account);
        db.put_root(storage_root)?;
        Ok(())
    }
}

impl StorageTrieRepository for HeedStorageTrieRepository {
    fn for_account(&self, account: &Address) -> StorageTrie {
        let db = HeedEthStorageTrieDb::new(self.env, *account);

        if let Some(storage_root) = db.root().unwrap() {
            StorageTrie::from(
                Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                    EthTrieDbWithHeedError::new(db),
                ))),
                storage_root,
            )
            .unwrap()
        } else {
            StorageTrie::new(Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                EthTrieDbWithHeedError::new(db),
            ))))
        }
    }

    fn for_account_with_root(&self, account: &Address, storage_root: &B256) -> StorageTrie {
        let db = HeedEthStorageTrieDb::new(self.env, *account);

        if db.root().unwrap().is_some() {
            StorageTrie::from(
                Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                    EthTrieDbWithHeedError::new(db),
                ))),
                *storage_root,
            )
            .unwrap()
        } else {
            StorageTrie::new(Arc::new(BoxedTrieDb::new(EthTrieDbWithLocalError(
                EthTrieDbWithHeedError::new(db),
            ))))
        }
    }

    fn apply(&mut self, changes: StorageTriesChanges) -> storage::Result<()> {
        for (account, changes) in changes {
            let storage_root = changes.root;
            let storage_trie = self.for_account(&account);
            storage_trie.apply(changes)?;
            self.replace(account, storage_root)?;
        }
        Ok(())
    }
}

pub struct EthTrieDbWithHeedError<T: DB>(pub T);

impl<T: DB> EthTrieDbWithHeedError<T> {
    pub fn new(db: T) -> Self {
        Self(db)
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

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        Ok(self.0.remove(key)?)
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(self.0.flush()?)
    }
}
