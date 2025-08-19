use {
    crate::state::{
        BlockHeight, HeightToStateRootIndex, ProofResponse, StateQueries,
        proof_from_trie_and_resolver,
    },
    eth_trie::{DB, EthTrie},
    move_bytecode_utils::compiled_module_viewer::CompiledModuleView,
    move_core_types::{
        account_address::AccountAddress, identifier::Identifier, language_storage::StructTag,
    },
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    serde::{Serialize, de::DeserializeOwned},
    std::sync::Arc,
    umi_evm_ext::state::{self, StorageTrieRepository},
    umi_execution::transaction::{L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS},
    umi_shared::primitives::{B256, ToEthAddress, U256},
    umi_state::{EthTrieResolver, Listable},
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

    fn move_list_modules(
        &self,
        account: AccountAddress,
        height: BlockHeight,
        after: Option<&Identifier>,
        limit: u32,
    ) -> Result<Vec<Identifier>, state::Error> {
        let trie = self.trie_at(height)?;
        move_list_elements(&trie, account, after, limit)
    }

    fn move_list_resources(
        &self,
        account: AccountAddress,
        height: BlockHeight,
        after: Option<&StructTag>,
        limit: u32,
    ) -> Result<Vec<StructTag>, state::Error> {
        let trie = self.trie_at(height)?;
        move_list_elements(&trie, account, after, limit)
    }

    fn resolver_at(
        &self,
        height: BlockHeight,
    ) -> Result<impl MoveResolver + TableResolver + CompiledModuleView + '_, state::Error> {
        Ok(EthTrieResolver::new(self.trie_at(height)?))
    }
}

fn move_list_elements<T, D>(
    trie: &EthTrie<D>,
    account: AccountAddress,
    after: Option<&T>,
    limit: u32,
) -> Result<Vec<T>, state::Error>
where
    T: Listable + Clone + Ord + Listable + Serialize + DeserializeOwned + 'static,
    D: DB,
{
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut limit = limit as usize;
    let mut iter = umi_state::SkipListIterator::new(account, after, trie)?;
    let mut result = Vec::with_capacity(limit);

    // Check to make sure the first element in the iterator is different than `after`.
    // If `after` is contained in the Skip List then it will be returned as the first
    // element of the iterator, so it needs special handling.
    match iter.next().transpose()? {
        Some(id) if Some(&id) != after => {
            result.push(id);
            limit -= 1;
        }
        _ => (),
    }

    for id in iter.take(limit) {
        result.push(id?);
    }

    Ok(result)
}
