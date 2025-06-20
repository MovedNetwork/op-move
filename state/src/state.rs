use {
    crate::{Changes, EthTrieResolver, InsertChangeSetIntoMerkleTrie, State},
    eth_trie::{DB, EthTrie, TrieError},
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    std::sync::Arc,
    umi_evm_ext::state::DbWithRoot,
    umi_shared::primitives::B256,
    umi_trie::TryFromOptRoot,
};

/// A blockchain state implementation backed by [`eth_trie`].
#[derive(Debug)]
pub struct EthTrieState<D: DB> {
    resolver: EthTrieResolver<D>,
    state_root: Option<B256>,
}

impl<D: DbWithRoot> EthTrieState<D> {
    pub fn try_new(db: Arc<D>) -> Result<Self, TrieError> {
        let state_root = db.root().map_err(|e| TrieError::DB(e.to_string()))?;
        let trie = EthTrie::try_from_opt_root(db, state_root)?;

        Ok(Self {
            state_root,
            resolver: EthTrieResolver::new(trie),
        })
    }

    pub fn empty(db: Arc<D>) -> Self {
        Self {
            state_root: None,
            resolver: EthTrieResolver::new(EthTrie::new(db)),
        }
    }

    pub(crate) fn trie_mut(&mut self) -> &mut EthTrie<D> {
        self.resolver.trie_mut()
    }

    fn db(&self) -> &Arc<D> {
        &self.resolver.trie().db
    }
}

impl<D: DbWithRoot> State for EthTrieState<D> {
    type Err = TrieError;

    fn apply(&mut self, changes: Changes) -> Result<(), Self::Err> {
        let root = self
            .trie_mut()
            .insert_change_set_into_merkle_trie(&changes)?;
        self.state_root.replace(root);
        self.db()
            .put_root(root)
            .map_err(|e| TrieError::DB(e.to_string()))
    }

    fn resolver(&self) -> &(impl MoveResolver + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> B256 {
        self.state_root.unwrap_or_default()
    }
}
