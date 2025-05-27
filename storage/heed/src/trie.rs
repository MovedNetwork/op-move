use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, EncodableBytes, EncodableU64},
    },
    eth_trie::{DB, EthTrie, TrieError},
    heed::RoTxn,
    moved_evm_ext::state::DbWithRoot,
    moved_shared::primitives::B256,
    std::sync::Arc,
};

pub type Key = EncodableBytes;
pub type Value = EncodableBytes;
pub type Db = heed::Database<Key, Value>;
pub type RootKey = EncodableU64;
pub type RootValue = EncodableB256;
pub type RootDb = heed::Database<RootKey, RootValue>;
pub const DB: &str = "trie";
pub const ROOT_DB: &str = "trie_root";
pub const ROOT_KEY: u64 = 0u64;

pub struct HeedEthTrieDb<'db> {
    env: &'db heed::Env,
}

impl<'db> HeedEthTrieDb<'db> {
    pub fn new(env: &'db heed::Env) -> Self {
        Self { env }
    }
}

impl DbWithRoot for HeedEthTrieDb<'_> {
    fn root(&self) -> Result<Option<B256>, heed::Error> {
        let transaction = self.env.read_txn()?;

        let db = self.env.trie_root_database(&transaction)?;

        let root = db.get(&transaction, &ROOT_KEY)?;

        transaction.commit()?;

        Ok(root)
    }

    fn put_root(&self, root: B256) -> Result<(), heed::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.env.trie_root_database(&transaction)?;

        db.put(&mut transaction, &ROOT_KEY, &root)?;

        transaction.commit()
    }
}

impl DB for HeedEthTrieDb<'_> {
    type Error = heed::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let transaction = self.env.read_txn()?;

        let db = self.env.trie_database(&transaction)?;

        let value = db.get(&transaction, key)?.map(<[u8]>::to_vec);

        transaction.commit()?;

        Ok(value)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.env.trie_database(&transaction)?;

        db.put(&mut transaction, key, value.as_slice())?;

        transaction.commit()
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.env.trie_database(&transaction)?;

        for (key, value) in keys.into_iter().zip(values) {
            db.put(&mut transaction, key.as_slice(), value.as_slice())?;
        }

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

pub trait HeedTrieExt {
    fn trie_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;

    fn trie_root_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<RootKey, RootValue>>;
}

impl HeedTrieExt for heed::Env {
    fn trie_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("Trie database should exist");

        Ok(HeedDb(db))
    }

    fn trie_root_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<RootKey, RootValue>> {
        let db: RootDb = self
            .open_database(rtxn, Some(ROOT_DB))?
            .expect("Trie root database should exist");

        Ok(HeedDb(db))
    }
}
