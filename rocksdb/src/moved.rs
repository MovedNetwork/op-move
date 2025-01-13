use {
    crate::RocksEthTrieDb,
    eth_trie::{EthTrie, Trie, DB},
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    moved::{
        primitives::{KeyHashable, B256},
        state_actor::HistoricResolver,
        storage::{State, ToTreeValues},
    },
    std::sync::Arc,
};

/// A blockchain state implementation backed by [`rocksdb`] as its persistent storage engine.
pub struct RocksDbState<'db> {
    db: Arc<RocksEthTrieDb<'db>>,
    resolver: HistoricResolver<RocksEthTrieDb<'db>>,
    state_root: B256,
}

impl<'db> RocksDbState<'db> {
    const STATE_ROOT_KEY: &'static str = "state_root";

    pub fn new(db: Arc<RocksEthTrieDb<'db>>) -> Self {
        let state_root = db
            .db()
            .get(Self::STATE_ROOT_KEY)
            .unwrap()
            .map(|v| B256::from_slice(&v))
            .unwrap_or(B256::ZERO);

        Self {
            resolver: HistoricResolver::new(db.clone(), state_root),
            state_root,
            db,
        }
    }

    fn persist_state_root(&self) -> Result<(), rocksdb::Error> {
        self.db.db().put(Self::STATE_ROOT_KEY, self.state_root.0)
    }

    fn tree(&self) -> EthTrie<RocksEthTrieDb<'db>> {
        let db = self.db.clone();

        match self.state_root {
            B256::ZERO => EthTrie::new(db),
            root => EthTrie::from(db, root).unwrap(),
        }
    }

    fn insert_change_set_into_merkle_trie(
        &mut self,
        change_set: &ChangeSet,
    ) -> Result<B256, eth_trie::TrieError> {
        let values = change_set.to_tree_values();
        let mut trie = self.tree();

        for (k, v) in values {
            let key_bytes = k.key_hash();
            let value_bytes = v
                .as_ref()
                .map(|x| bcs::to_bytes(x).expect("Value should serialize"));

            trie.insert(
                key_bytes.0.as_slice(),
                value_bytes.as_deref().unwrap_or(&[]),
            )?;
        }

        trie.root_hash()
    }
}

impl<'db> State for RocksDbState<'db> {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        self.state_root = self.insert_change_set_into_merkle_trie(&changes).unwrap();
        self.resolver = HistoricResolver::new(self.db.clone(), self.state_root);
        self.persist_state_root().unwrap();
        Ok(())
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        _table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.state_root = self.insert_change_set_into_merkle_trie(&changes).unwrap();
        self.resolver = HistoricResolver::new(self.db.clone(), self.state_root);
        self.persist_state_root().unwrap();
        Ok(())
    }

    fn db(&self) -> Arc<impl DB> {
        self.db.clone()
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> B256 {
        self.state_root
    }
}
