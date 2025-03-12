use {
    crate::{
        all::HeedDb,
        generic::{EncodableAddress, EncodableB256, EncodableBytes},
    },
    eth_trie::DB,
    heed::RoTxn,
    moved_evm_ext::storage::DbWithRoot,
    moved_shared::primitives::{Address, B256},
};

pub type Key = EncodableBytes;
pub type Value = EncodableBytes;
pub type Db = heed::Database<Key, Value>;
pub type RootKey = EncodableAddress;
pub type RootValue = EncodableB256;
pub type RootDb = heed::Database<RootKey, RootValue>;
pub const DB: &str = "evm_storage_trie";
pub const ROOT_DB: &str = "evm_storage_trie_root";

pub struct HeedEthStorageTrieDb<'db> {
    env: &'db heed::Env,
    account: Address,
}

impl<'db> HeedEthStorageTrieDb<'db> {
    pub fn new(env: &'db heed::Env, account: Address) -> Self {
        Self { env, account }
    }

    fn unique_key(&self, key: &[u8]) -> Vec<u8> {
        [self.account.as_slice(), key].concat()
    }
}

impl<'db> DbWithRoot for HeedEthStorageTrieDb<'db> {
    fn root(&self) -> Result<Option<B256>, heed::Error> {
        let transaction = self.env.read_txn()?;

        let db = self.env.storage_root_database(&transaction)?;

        let root = db.get(&transaction, &self.account)?;

        transaction.commit()?;

        Ok(root)
    }

    fn put_root(&self, root: B256) -> Result<(), heed::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.env.storage_root_database(&transaction)?;

        db.put(&mut transaction, &self.account, &root)?;

        transaction.commit()
    }
}

impl<'db> DB for HeedEthStorageTrieDb<'db> {
    type Error = heed::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let transaction = self.env.read_txn()?;

        let db = self.env.storage_trie_database(&transaction)?;

        let key = &self.unique_key(key);
        let value = db.get(&transaction, key)?.map(<[u8]>::to_vec);

        transaction.commit()?;

        Ok(value)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let mut transaction = self.env.write_txn()?;

        let db = self.env.storage_trie_database(&transaction)?;

        let key = &self.unique_key(key);
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

pub trait HeedStorageTrieExt {
    fn storage_trie_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;

    fn storage_root_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<RootKey, RootValue>>;
}

impl HeedStorageTrieExt for heed::Env {
    fn storage_trie_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("Storage trie database should exist");

        Ok(HeedDb(db))
    }

    fn storage_root_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<RootKey, RootValue>> {
        let db: RootDb = self
            .open_database(rtxn, Some(ROOT_DB))?
            .expect("Storage root database should exist");

        Ok(HeedDb(db))
    }
}
