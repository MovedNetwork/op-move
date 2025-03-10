use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, EncodableU64},
        trie::{FromOptRoot, HeedEthTrieDb},
    },
    eth_trie::{EthTrie, TrieError, DB},
    heed::RoTxn,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress, effects::ChangeSet, resolver::MoveResolver,
    },
    move_table_extension::{TableChangeSet, TableResolver},
    moved_blockchain::state::{
        proof_from_trie_and_resolver, Balance, BlockHeight, EthTrieResolver, Nonce, ProofResponse,
        StateQueries,
    },
    moved_evm_ext::storage::StorageTrieRepository,
    moved_execution::{
        quick_get_eth_balance, quick_get_nonce,
        transaction::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
    },
    moved_shared::primitives::{ToEthAddress, B256, U256},
    moved_state::{InsertChangeSetIntoMerkleTrie, State},
    std::sync::Arc,
};

pub type Key = EncodableU64;
pub type Value = EncodableB256;
pub type Db = heed::Database<Key, Value>;
pub type HeightKey = EncodableU64;
pub type HeightValue = EncodableU64;
pub type HeightDb = heed::Database<HeightKey, HeightValue>;

pub const DB: &str = "state";
pub const HEIGHT_DB: &str = "state_height";
pub const HEIGHT_KEY: u64 = 0;

/// A blockchain state implementation backed by [`heed`] as its persistent storage engine.
pub struct HeedState<'db> {
    db: Arc<HeedEthTrieDb<'db>>,
    resolver: EthTrieResolver<HeedEthTrieDb<'db>>,
    state_root: Option<B256>,
}

impl<'db> HeedState<'db> {
    pub fn new(db: Arc<HeedEthTrieDb<'db>>) -> Self {
        let state_root = db
            .root()
            .expect("Database should be able to fetch state root");

        Self {
            resolver: EthTrieResolver::new(EthTrie::from_opt_root(db.clone(), state_root)),
            state_root,
            db,
        }
    }

    fn persist_state_root(&self) -> Result<(), heed::Error> {
        self.state_root
            .map(|root| self.db.put_root(root))
            .unwrap_or(Ok(()))
    }

    fn tree(&self) -> EthTrie<HeedEthTrieDb<'db>> {
        EthTrie::from_opt_root(self.db.clone(), self.state_root)
    }
}

impl<'db> State for HeedState<'db> {
    type Err = TrieError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        let mut tree = self.tree();
        let root = tree.insert_change_set_into_merkle_trie(&changes)?;
        self.state_root.replace(root);
        self.resolver = EthTrieResolver::new(tree);
        self.persist_state_root().unwrap();
        Ok(())
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        _table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.apply(changes)
    }

    fn db(&self) -> Arc<impl DB> {
        self.db.clone()
    }

    fn resolver(&self) -> &(impl MoveResolver<PartialVMError> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> B256 {
        self.state_root.unwrap_or_default()
    }
}

#[derive(Debug)]
pub struct HeedStateQueries<'db> {
    env: &'db heed::Env,
}

impl<'db> HeedStateQueries<'db> {
    pub fn new(env: &'db heed::Env) -> Self {
        Self { env }
    }

    pub fn from_genesis(env: &'db heed::Env, genesis_state_root: B256) -> Self {
        let this = Self { env };
        this.push_state_root(genesis_state_root).unwrap();
        this
    }

    pub fn push_state_root(&self, state_root: B256) -> Result<(), heed::Error> {
        let height = self.height()?;
        let mut transaction = self.env.write_txn()?;

        let db = self.env.state_database(&transaction)?;

        db.put(&mut transaction, &height, &state_root)?;

        let db = self.env.state_height_database(&transaction)?;

        db.put(&mut transaction, &HEIGHT_KEY, &(height + 1))?;

        transaction.commit()
    }

    fn height(&self) -> Result<u64, heed::Error> {
        let transaction = self.env.read_txn()?;

        let db = self.env.state_height_database(&transaction)?;

        let height = db.get(&transaction, &HEIGHT_KEY);

        transaction.commit()?;

        Ok(height?.unwrap_or(0))
    }

    fn root_by_height(&self, height: u64) -> Result<Option<B256>, heed::Error> {
        let transaction = self.env.read_txn()?;

        let db = self.env.state_database(&transaction)?;

        let root = db.get(&transaction, &height);

        transaction.commit()?;

        root
    }

    fn tree<D: DB>(&self, db: Arc<D>, height: u64) -> Result<EthTrie<D>, heed::Error> {
        Ok(match self.root_by_height(height)? {
            Some(root) => EthTrie::from(db, root).expect("State root should be valid"),
            None => EthTrie::new(db),
        })
    }

    fn resolver<'a>(
        &self,
        db: Arc<impl DB + 'a>,
        height: BlockHeight,
    ) -> Result<impl MoveResolver<PartialVMError> + TableResolver + 'a, heed::Error> {
        Ok(EthTrieResolver::new(self.tree(db, height)?))
    }
}

impl<'db> StateQueries for HeedStateQueries<'db> {
    fn balance_at(
        &self,
        db: Arc<impl DB>,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance> {
        let resolver = self.resolver(db, height).ok()?;

        Some(quick_get_eth_balance(&account, &resolver, evm_storage))
    }

    fn nonce_at(
        &self,
        db: Arc<impl DB>,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce> {
        let resolver = self.resolver(db, height).ok()?;

        Some(quick_get_nonce(&account, &resolver, evm_storage))
    }

    fn proof_at(
        &self,
        db: Arc<impl DB>,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Option<ProofResponse> {
        let address = account.to_eth_address();

        // Only L2 contract addresses supported at this time
        if address < L2_LOWEST_ADDRESS || L2_HIGHEST_ADDRESS < address {
            return None;
        }

        let mut tree = self.tree(db.clone(), height).ok()?;
        let resolver = self.resolver(db, height).ok()?;

        proof_from_trie_and_resolver(address, storage_slots, &mut tree, &resolver, evm_storage)
    }
}

pub trait HeedStateExt {
    fn state_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;

    fn state_height_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<HeightKey, HeightValue>>;
}

impl HeedStateExt for heed::Env {
    fn state_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("State root database should exist");

        Ok(HeedDb(db))
    }

    fn state_height_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<HeightKey, HeightValue>> {
        let db: HeightDb = self
            .open_database(rtxn, Some(HEIGHT_DB))?
            .expect("State height database should exist");

        Ok(HeedDb(db))
    }
}
