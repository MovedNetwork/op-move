use {
    crate::state::{
        BlockHeight, HeightToStateRootIndex, ProofResponse, StateQueries,
        proof_from_trie_and_resolver,
    },
    eth_trie::{DB, EthTrie},
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    moved_evm_ext::state::StorageTrieRepository,
    moved_execution::transaction::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
    moved_shared::primitives::{B256, ToEthAddress, U256},
    moved_state::EthTrieResolver,
    std::sync::Arc,
};

#[derive(Debug)]
pub struct EthTrieStateQueries<R, D: DB> {
    index: R,
    db: Arc<D>,
    genesis_state_root: B256,
}

impl<R: Clone, D: DB> Clone for EthTrieStateQueries<R, D> {
    fn clone(&self) -> Self {
        Self::new(self.index.clone(), self.db.clone(), self.genesis_state_root)
    }
}

impl<R, D: DB> EthTrieStateQueries<R, D> {
    pub fn new(index: R, db: Arc<D>, genesis_state_root: B256) -> Self {
        Self {
            index,
            db,
            genesis_state_root,
        }
    }
}

impl<R: HeightToStateRootIndex, D: DB> EthTrieStateQueries<R, D> {
    fn root_by_height(&self, height: BlockHeight) -> Option<B256> {
        match height {
            0 => Some(self.genesis_state_root),
            _ => self.index.root_by_height(height),
        }
    }

    fn trie_at(&self, height: BlockHeight) -> Option<EthTrie<D>> {
        Some(
            EthTrie::from(self.db.clone(), self.root_by_height(height)?)
                .expect("State root should be in sync with block height"),
        )
    }
}

impl<R: HeightToStateRootIndex, D: DB> StateQueries for EthTrieStateQueries<R, D> {
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

        let resolver = self.resolver_at(height);
        let mut tree = self.trie_at(height).unwrap();

        proof_from_trie_and_resolver(address, storage_slots, &mut tree, &resolver, evm_storage)
    }

    fn resolver_at(&self, height: BlockHeight) -> impl MoveResolver + TableResolver + '_ {
        EthTrieResolver::new(self.trie_at(height).unwrap())
    }
}
