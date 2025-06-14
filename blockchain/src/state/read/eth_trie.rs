use {
    crate::state::{
        BlockHeight, HeightToStateRootIndex, ProofResponse, StateQueries,
        proof_from_trie_and_resolver,
    },
    eth_trie::{DB, EthTrie},
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    std::sync::Arc,
    umi_evm_ext::state::{self, StorageTrieRepository},
    umi_execution::transaction::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
    umi_shared::primitives::{B256, ToEthAddress, U256},
    umi_state::EthTrieResolver,
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
    pub fn push_state_root(&self, state_root: B256) -> Result<(), R::Err> {
        self.index.push_state_root(state_root)
    }

    fn root_by_height(&self, height: BlockHeight) -> Result<B256, state::Error> {
        match height {
            0 => Ok(self.genesis_state_root),
            _ => self
                .index
                .root_by_height(height)
                .map_err(|e| state::Error::EthTrie(eth_trie::TrieError::DB(format!("{e:?}"))))?
                .ok_or(state::Error::UnknownBlockHeight(height)),
        }
    }

    fn trie_at(&self, height: BlockHeight) -> Result<EthTrie<D>, state::Error> {
        let root = self.root_by_height(height)?;
        EthTrie::from(self.db.clone(), root).map_err(state::Error::EthTrie)
    }
}

impl<R: HeightToStateRootIndex, D: DB> StateQueries for EthTrieStateQueries<R, D> {
    fn proof_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Result<ProofResponse, state::Error> {
        let address = account.to_eth_address();

        // Only L2 contract addresses supported at this time
        if address < L2_LOWEST_ADDRESS || L2_HIGHEST_ADDRESS < address {
            return Err(state::Error::AddressOutsideRange(address));
        }

        let resolver = self.resolver_at(height)?;
        let mut tree = self.trie_at(height)?;

        proof_from_trie_and_resolver(address, storage_slots, &mut tree, &resolver, evm_storage)
    }

    fn resolver_at(
        &self,
        height: BlockHeight,
    ) -> Result<impl MoveResolver + TableResolver + '_, state::Error> {
        Ok(EthTrieResolver::new(self.trie_at(height)?))
    }
}
