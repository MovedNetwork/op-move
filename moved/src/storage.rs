use {
    crate::{
        iter::GroupIterator,
        primitives::{ToB256, B256},
    },
    aptos_crypto::{hash::CryptoHash, HashValue},
    aptos_jellyfish_merkle::{
        mock_tree_store::MockTreeStore, node_type::Node, JellyfishMerkleTree, TreeReader,
        TreeUpdateBatch,
    },
    aptos_storage_interface::AptosDbError,
    aptos_types::{
        state_store::{state_key::StateKey, state_value::StateValue},
        transaction::Version,
    },
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    move_vm_test_utils::InMemoryStorage,
    std::{collections::HashMap, fmt::Debug},
};

/// A global blockchain state trait.
///
/// This trait is defined by these operations:
/// * [`resolver`]: Creates [`MoveResolver`] that can resolve both resources and modules.
/// * [`state_root`]: Returns current state root.
/// * [`apply`]: Applies changes produced by a transaction on the state trie.
/// * [`apply_with_tables`]: Same as [`apply`] but includes changes to tables from
///   [`move_table_extension`].
///
/// [`resolver`]: Self::resolver
/// [`state_root`]: Self::state_root
/// [`apply`]: Self::apply
/// [`apply_with_tables`]: Self::apply_with_tables
pub trait State {
    /// The associated error that can occur on storage operations.
    type Err: Debug;

    /// Applies the `changes` to the underlying storage state.
    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err>;

    /// Applies the `changes` to the underlying storage state. In addition, applies `table_changes`
    /// using the [`move_table_extension`].
    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        table_changes: TableChangeSet,
    ) -> Result<(), Self::Err>;

    fn db(&self) -> &(impl TreeReader<StateKey> + Sync);

    /// Returns a reference to a [`MoveResolver`] that can resolve both resources and modules.
    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver);

    /// Retrieves the current state root.
    fn state_root(&self) -> B256;
}

pub struct InMemoryState {
    resolver: InMemoryStorage,
    db: MockTreeStore<StateKey>,
    version: Version,
}

impl Default for InMemoryState {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryState {
    const ALLOW_OVERWRITE: bool = false;

    pub fn new() -> Self {
        Self {
            resolver: InMemoryStorage::new(),
            db: MockTreeStore::new(Self::ALLOW_OVERWRITE),
            version: 0,
        }
    }

    fn tree(&self) -> JellyfishMerkleTree<impl TreeReader<StateKey>, StateKey> {
        JellyfishMerkleTree::new(&self.db)
    }
}

impl State for InMemoryState {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        self.insert_change_set_into_merkle_trie(&changes);
        self.resolver.apply(changes)?;
        Ok(())
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.insert_change_set_into_merkle_trie(&changes);
        self.resolver.apply_extended(changes, table_changes)?;
        Ok(())
    }

    fn db(&self) -> &(impl TreeReader<StateKey> + Sync) {
        &self.db
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> B256 {
        self.tree()
            .get_root_hash(self.version)
            .map(ToB256::to_h256)
            .unwrap()
    }
}

impl InMemoryState {
    fn next_version(&mut self) -> Version {
        self.version += 1;
        self.version
    }

    fn insert_change_set_into_merkle_trie(&mut self, change_set: &ChangeSet) -> B256 {
        let version = self.next_version();
        let values = change_set.to_tree_values();
        let values_per_shard = values
            .iter()
            .map(|(&k, v)| (k, v.as_ref()))
            .group_by(|(k, _)| k.nibble(0));
        let (root_hash, tree_update_batch) = self
            .tree()
            .create_update_batch(values_per_shard, version)
            .expect("Fails on duplicate key or storage read. In memory storage cannot fail.");

        self.db
            .write_tree_update_batch(tree_update_batch)
            .expect("Fails on duplicate key or storage write. In memory storage cannot fail.");

        root_hash.to_h256()
    }
}

/// The jellyfish merkle trie key is the hash of the actual key.
type TreeKey = HashValue;

/// The jellyfish merkle trie value consists of a hash of the actual value and the actual key.
type TreeValue = Option<(HashValue, StateKey)>;

/// A reference to [`TreeValue`].
type TreeValueRef<'r> = Option<&'r (HashValue, StateKey)>;

/// Converts itself to a set of updates for a jellyfish merkle trie.
///
/// This trait is defined by a single operation called [`Self::to_tree_values`].
trait ToTreeValues {
    /// Extracts modules and resources and generates a set of merkle trie keys and values applicable
    /// to a trie for the purpose of updating it resulting in a new root hash.
    ///
    /// The [`TreeValue`] is optional where:
    /// * The [`Some`] variant creates new or replaces existing value.
    /// * The [`None`] variant marks a deletion.
    ///
    /// The [`TreeKey`] is a hashed values always based on the account's address and further based
    /// on module name or resource type.
    ///
    /// # Move language context
    ///
    /// The purpose of Move programs is to read from and write to tree-shaped persistent global
    /// storage. Programs cannot access the filesystem, network, or any other data outside of this
    /// tree.
    ///
    /// In pseudocode, the global storage looks something like:
    ///
    /// ```move
    /// module 0x42::example {
    ///   struct GlobalStorage {
    ///     resources: Map<(address, ResourceType), ResourceValue>,
    ///     modules: Map<(address, ModuleName), ModuleBytecode>
    ///   }
    /// }
    /// ```
    ///
    /// Structurally, global storage is a forest consisting of trees rooted at an account address.
    /// Each address can store both resource data values and module code values. As the pseudocode
    /// above indicates, each address can store at most one resource value of a given type and at
    /// most one module with a given name.
    fn to_tree_values(&self) -> HashMap<TreeKey, TreeValue>;
}

impl ToTreeValues for ChangeSet {
    fn to_tree_values(&self) -> HashMap<TreeKey, TreeValue> {
        self.accounts()
            .iter()
            .flat_map(|(address, changes)| {
                changes
                    .modules()
                    .iter()
                    .map(move |(k, v)| {
                        let value = v.clone().ok().map(StateValue::new_legacy);
                        let key = StateKey::module(address, k.as_ident_str());

                        (key, value)
                    })
                    .chain(changes.resources().iter().map(move |(k, v)| {
                        let value = v.clone().ok().map(StateValue::new_legacy);
                        let key = StateKey::resource(address, k).unwrap();

                        (key, value)
                    }))
            })
            .map(|(k, v)| (k.hash(), v.map(|v| (v.hash(), k))))
            .collect::<HashMap<_, _>>()
    }
}

/// Creates update batches for mutating the merkle trie.
///
/// This trait is defined by a single operation called [`Self::create_update_batch`].
trait CreateUpdateBatch {
    /// Creates a [`TreeUpdateBatch`] from `values_per_shard` and a `version`. Returns the update
    /// batch and a root hash of the trie after applying the update batch.
    ///
    /// This function does not write any updates to the tree itself, making this work with an
    /// immutable reference to `self`. It only creates an update batch that can be written using
    /// a compatible storage backend.
    fn create_update_batch(
        &self,
        values_per_shard: HashMap<u8, Vec<(TreeKey, TreeValueRef)>>,
        version: Version,
    ) -> Result<(HashValue, TreeUpdateBatch<StateKey>), AptosDbError>;
}

impl<'a, R> CreateUpdateBatch for JellyfishMerkleTree<'a, R, StateKey>
where
    R: 'a + TreeReader<StateKey> + Sync,
{
    fn create_update_batch(
        &self,
        values_per_shard: HashMap<u8, Vec<(TreeKey, TreeValueRef)>>,
        version: Version,
    ) -> Result<(HashValue, TreeUpdateBatch<StateKey>), AptosDbError> {
        let mut tree_update_batch = TreeUpdateBatch::new();
        const NIL: Node<StateKey> = Node::Null;
        let mut shard_root_nodes = [NIL; 16];
        let persisted_versions = self.get_shard_persisted_versions(None)?;

        for (shard_id, values) in values_per_shard {
            let (shard_root_node, batch) = self.batch_put_value_set_for_shard(
                shard_id,
                values,
                None,
                persisted_versions[shard_id as usize],
                version,
            )?;

            tree_update_batch.combine(batch);
            shard_root_nodes[shard_id as usize] = shard_root_node;
        }

        self.put_top_levels_nodes(shard_root_nodes.to_vec(), version.checked_sub(1), version)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        bytes::Bytes,
        move_core_types::{
            account_address::AccountAddress,
            effects::{AccountChanges, Op},
            identifier::Identifier,
        },
    };

    #[test]
    #[should_panic]
    fn test_state_root_from_empty_tree_fails() {
        InMemoryState::new().state_root();
    }

    #[test]
    fn test_insert_to_empty_tree_produces_new_state_root() {
        let mut state = InMemoryState::new();
        let mut change_set = ChangeSet::new();

        change_set
            .add_account_changeset(AccountAddress::new([0; 32]), AccountChanges::new())
            .unwrap();

        let expected_root_hash = state.insert_change_set_into_merkle_trie(&change_set);
        let actual_state_root = state.state_root();

        assert_eq!(actual_state_root, expected_root_hash);
    }

    #[test]
    fn test_state_root_is_different_after_update_changes_trie() {
        let mut state = InMemoryState::new();
        let mut change_set = ChangeSet::new();

        change_set
            .add_account_changeset(AccountAddress::new([0; 32]), AccountChanges::new())
            .unwrap();
        state.insert_change_set_into_merkle_trie(&change_set);
        let old_state_root = state.state_root();

        let mut change_set = ChangeSet::new();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();
        change_set
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state.insert_change_set_into_merkle_trie(&change_set);
        let new_state_root = state.state_root();

        assert_ne!(old_state_root, new_state_root);
    }

    #[test]
    fn test_state_root_remains_the_same_when_update_does_not_change_trie() {
        let mut state = InMemoryState::new();
        let mut change_set = ChangeSet::new();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();

        change_set
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state.insert_change_set_into_merkle_trie(&change_set);
        let expected_state_root = state.state_root();

        let mut change_set = ChangeSet::new();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();
        change_set
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state.insert_change_set_into_merkle_trie(&change_set);
        let actual_state_root = state.state_root();

        assert_eq!(actual_state_root, expected_state_root);
    }
}
