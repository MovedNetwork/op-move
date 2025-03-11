use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, EncodableBytes, EncodableU64},
        trie::ROOT_DB,
    },
    eth_trie::{EthTrie, TrieError, DB},
    heed::{RoTxn, RwTxn},
    moved_shared::primitives::{Address, B256},
    std::sync::Arc,
};

pub type Key = EncodableBytes;
pub type Value = EncodableBytes;
pub type Db = heed::Database<Key, Value>;
pub type RootKey = EncodableU64;
pub type RootValue = EncodableB256;
pub type RootDb = heed::Database<RootKey, RootValue>;
pub const ROOT_KEY: u64 = 0u64;

pub struct HeedEthTrie2Db<'db> {
    env: &'db heed::Env,
    db: String,
    root_db: String,
}

impl<'db> HeedEthTrie2Db<'db> {
    pub fn new(env: &'db heed::Env, account: Address) -> Self {
        Self {
            env,
            db: format!("evm_storage_{:x}", account.0),
            root_db: format!("evm_storage_root_{:x}", account.0),
        }
    }

    pub fn root(&self) -> Result<Option<B256>, heed::Error> {
        let transaction = self.env.read_txn()?;

        let Some(db) = self.trie_root_database(&transaction)? else {
            transaction.commit()?;
            return Ok(None);
        };

        let root = db.get(&transaction, &ROOT_KEY)?;

        transaction.commit()?;

        Ok(root)
    }

    pub fn put_root(&self, root: B256) -> Result<(), heed::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.trie_root_database_mut(&mut transaction)?;

        db.put(&mut transaction, &ROOT_KEY, &root)?;

        transaction.commit()
    }

    fn trie_database_mut(&self, wtxn: &mut RwTxn) -> heed::Result<HeedDb<Key, Value>> {
        self.env.trie_database_mut(wtxn, self.db.as_str())
    }

    fn trie_database(&self, rotx: &RoTxn) -> heed::Result<Option<HeedDb<Key, Value>>> {
        self.env.trie_database(rotx, self.db.as_str())
    }

    fn trie_root_database_mut(&self, wtxn: &mut RwTxn) -> heed::Result<HeedDb<RootKey, RootValue>> {
        self.env.trie_root_database_mut(wtxn, self.root_db.as_str())
    }

    fn trie_root_database(&self, rtxn: &RoTxn) -> heed::Result<Option<HeedDb<RootKey, RootValue>>> {
        self.env.trie_root_database(rtxn, self.root_db.as_str())
    }
}

impl<'db> DB for HeedEthTrie2Db<'db> {
    type Error = heed::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let transaction = self.env.read_txn()?;

        let Some(db) = self.trie_database(&transaction)? else {
            transaction.commit()?;
            return Ok(None);
        };

        let value = db.get(&transaction, key)?.map(<[u8]>::to_vec);

        transaction.commit()?;

        Ok(value)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.trie_database_mut(&mut transaction)?;

        db.put(&mut transaction, key, value.as_slice())?;

        transaction.commit()
    }

    fn remove(&self, _key: &[u8]) -> Result<(), Self::Error> {
        // Intentionally ignored to not remove historical trie nodes
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        // Intentionally ignored as cache management is delegated to the database
        Ok(())
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

pub trait HeedTrie2Ext {
    fn trie_database_mut(&self, wtxn: &mut RwTxn, name: &str) -> heed::Result<HeedDb<Key, Value>>;

    fn trie_database(&self, rotx: &RoTxn, name: &str) -> heed::Result<Option<HeedDb<Key, Value>>>;

    fn trie_root_database_mut(
        &self,
        wtxn: &mut RwTxn,
        name: &str,
    ) -> heed::Result<HeedDb<RootKey, RootValue>>;

    fn trie_root_database(
        &self,
        rotx: &RoTxn,
        name: &str,
    ) -> heed::Result<Option<HeedDb<RootKey, RootValue>>>;
}

impl HeedTrie2Ext for heed::Env {
    fn trie_database_mut(&self, wtxn: &mut RwTxn, name: &str) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self.create_database(wtxn, Some(name))?;

        Ok(HeedDb(db))
    }

    fn trie_database(&self, rotx: &RoTxn, name: &str) -> heed::Result<Option<HeedDb<Key, Value>>> {
        let db: Option<Db> = self.open_database(rotx, Some(name))?;

        Ok(db.map(HeedDb))
    }

    fn trie_root_database_mut(
        &self,
        wtxn: &mut RwTxn,
        name: &str,
    ) -> heed::Result<HeedDb<RootKey, RootValue>> {
        let db: RootDb = self.create_database(wtxn, Some(name))?;

        Ok(HeedDb(db))
    }

    fn trie_root_database(
        &self,
        rotx: &RoTxn,
        name: &str,
    ) -> heed::Result<Option<HeedDb<RootKey, RootValue>>> {
        let db: Option<RootDb> = self.open_database(rotx, Some(name))?;

        Ok(db.map(HeedDb))
    }
}
