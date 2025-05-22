use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, EncodableU64},
        trie::HeedEthTrieDb,
    },
    eth_trie::EthTrie,
    heed::RoTxn,
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    moved_blockchain::state::{
        Balance, BlockHeight, EthTrieResolver, Nonce, ProofResponse, StateQueries,
        proof_from_trie_and_resolver,
    },
    moved_evm_ext::state::StorageTrieRepository,
    moved_execution::{
        quick_get_eth_balance, quick_get_nonce,
        transaction::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
    },
    moved_shared::primitives::{B256, ToEthAddress, U256},
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

#[derive(Clone)]
pub struct HeedStateQueries<'db> {
    env: &'db heed::Env,
    trie_db: Arc<HeedEthTrieDb<'db>>,
    genesis_state_root: B256,
}

impl<'db> HeedStateQueries<'db> {
    pub fn new(
        env: &'db heed::Env,
        trie_db: Arc<HeedEthTrieDb<'db>>,
        genesis_state_root: B256,
    ) -> Self {
        Self {
            env,
            trie_db,
            genesis_state_root,
        }
    }

    pub fn push_state_root(&self, state_root: B256) -> Result<(), heed::Error> {
        let height = self.height()? + 1;
        let mut transaction = self.env.write_txn()?;

        let db = self.env.state_database(&transaction)?;

        db.put(&mut transaction, &height, &state_root)?;

        let db = self.env.state_height_database(&transaction)?;

        db.put(&mut transaction, &HEIGHT_KEY, &height)?;

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
        if height == 0 {
            return Ok(Some(self.genesis_state_root));
        }

        let transaction = self.env.read_txn()?;

        let db = self.env.state_database(&transaction)?;

        let root = db.get(&transaction, &height);

        transaction.commit()?;

        root
    }

    fn tree(&self, height: u64) -> Result<EthTrie<HeedEthTrieDb<'db>>, heed::Error> {
        Ok(match self.root_by_height(height)? {
            Some(root) => {
                EthTrie::from(self.trie_db.clone(), root).expect("State root should be valid")
            }
            None => EthTrie::new(self.trie_db.clone()),
        })
    }

    fn resolver(
        &self,
        height: BlockHeight,
    ) -> Result<impl MoveResolver + TableResolver, heed::Error> {
        Ok(EthTrieResolver::new(self.tree(height)?))
    }
}

impl StateQueries for HeedStateQueries<'_> {
    fn balance_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance> {
        let resolver = self.resolver(height).ok()?;

        Some(quick_get_eth_balance(&account, &resolver, evm_storage))
    }

    fn nonce_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce> {
        let resolver = self.resolver(height).ok()?;

        Some(quick_get_nonce(&account, &resolver, evm_storage))
    }

    fn proof_at(
        &self,
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

        let mut tree = self.tree(height).ok()?;
        let resolver = self.resolver(height).ok()?;

        proof_from_trie_and_resolver(address, storage_slots, &mut tree, &resolver, evm_storage)
    }

    fn resolver_at(&self, height: BlockHeight) -> impl MoveResolver + TableResolver + '_ {
        self.resolver(height).unwrap()
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
