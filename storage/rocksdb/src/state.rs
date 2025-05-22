use {
    crate::{
        RocksDb, RocksEthTrieDb,
        generic::{FromKey, ToKey},
    },
    eth_trie::EthTrie,
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
    rocksdb::{AsColumnFamilyRef, WriteBatchWithTransaction},
    std::sync::Arc,
};

pub const COLUMN_FAMILY: &str = "state";
pub const HEIGHT_COLUMN_FAMILY: &str = "state_height";
pub const HEIGHT_KEY: &str = "state_height";

#[derive(Clone)]
pub struct RocksDbStateQueries<'db> {
    db: &'db RocksDb,
    trie_db: Arc<RocksEthTrieDb<'db>>,
    genesis_state_root: B256,
}

impl<'db> RocksDbStateQueries<'db> {
    pub fn new(
        db: &'db RocksDb,
        trie_db: Arc<RocksEthTrieDb<'db>>,
        genesis_state_root: B256,
    ) -> Self {
        Self {
            db,
            trie_db,
            genesis_state_root,
        }
    }

    pub fn push_state_root(&self, state_root: B256) -> Result<(), rocksdb::Error> {
        let height = self.height()? + 1;
        let mut batch = WriteBatchWithTransaction::<false>::default();

        batch.put_cf(&self.cf(), height.to_key(), state_root);
        batch.put_cf(&self.height_cf(), HEIGHT_KEY, height.to_key());

        self.db.write(batch)
    }

    fn height(&self) -> Result<u64, rocksdb::Error> {
        Ok(self
            .db
            .get_pinned_cf(&self.height_cf(), HEIGHT_KEY)?
            .map(|v| u64::from_key(v.as_ref()))
            .unwrap_or(0))
    }

    fn root_by_height(&self, height: u64) -> Result<Option<B256>, rocksdb::Error> {
        if height == 0 {
            return Ok(Some(self.genesis_state_root));
        }

        Ok(self
            .db
            .get_pinned_cf(&self.cf(), height.to_key())?
            .map(|v| B256::new(v.as_ref().try_into().unwrap())))
    }

    fn tree(&self, height: u64) -> Result<EthTrie<RocksEthTrieDb<'db>>, rocksdb::Error> {
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
    ) -> Result<impl MoveResolver + TableResolver, rocksdb::Error> {
        Ok(EthTrieResolver::new(self.tree(height)?))
    }

    fn height_cf(&self) -> impl AsColumnFamilyRef + use<'_> {
        self.db
            .cf_handle(HEIGHT_COLUMN_FAMILY)
            .expect("Column family should exist")
    }

    fn cf(&self) -> impl AsColumnFamilyRef + use<'_> {
        self.db
            .cf_handle(COLUMN_FAMILY)
            .expect("Column family should exist")
    }
}

impl StateQueries for RocksDbStateQueries<'_> {
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
