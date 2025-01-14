use {
    crate::primitives::{KeyHashable, B256},
    aptos_types::state_store::{state_key::StateKey, state_value::StateValue},
    eth_trie::{EthTrie, MemoryDB, Trie, DB},
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    move_vm_test_utils::InMemoryStorage,
    std::{collections::HashMap, fmt::Debug, sync::Arc},
};

// TODO: Should change `State` interface to return `Result`.
pub const IN_MEMORY_EXPECT_MSG: &str = "In-memory storage cannot fail";

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

    fn db(&self) -> Arc<impl DB>;

    /// Returns a reference to a [`MoveResolver`] that can resolve both resources and modules.
    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver);

    /// Retrieves the current state root.
    fn state_root(&self) -> B256;
}

pub struct InMemoryState {
    resolver: InMemoryStorage,
    db: Arc<MemoryDB>,
    current_state_root: Option<B256>,
}

impl Default for InMemoryState {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryState {
    // Per `eth-trie` docs: If "light" is true, the data is deleted from the database
    // at the time of submission.
    const IS_LIGHT: bool = false;

    pub fn new() -> Self {
        Self {
            resolver: InMemoryStorage::new(),
            db: Arc::new(MemoryDB::new(Self::IS_LIGHT)),
            current_state_root: None,
        }
    }

    fn tree(&self) -> EthTrie<MemoryDB> {
        let db = self.db.clone();
        match self.current_state_root {
            None => EthTrie::new(db),
            Some(root) => EthTrie::from(db, root).expect(IN_MEMORY_EXPECT_MSG),
        }
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

    fn db(&self) -> Arc<impl DB> {
        self.db.clone()
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> B256 {
        self.current_state_root.unwrap_or_default()
    }
}

impl InMemoryState {
    fn insert_change_set_into_merkle_trie(&mut self, change_set: &ChangeSet) -> B256 {
        let values = change_set.to_tree_values();

        let mut trie = self.tree();
        for (k, v) in values {
            let key_bytes = k.key_hash();
            let value_bytes = v
                .as_ref()
                .map(|x| bcs::to_bytes(x).expect("Value must serialize"));
            trie.insert(
                key_bytes.0.as_slice(),
                value_bytes.as_deref().unwrap_or(&[]),
            )
            .expect(IN_MEMORY_EXPECT_MSG);
        }
        let root = trie.root_hash().expect(IN_MEMORY_EXPECT_MSG);
        self.current_state_root = Some(root);
        root
    }
}

/// The merkle patricia trie key is the hash of the actual key.
type TreeKey = StateKey;

/// The merkle patricia trie value. None indicates the value was deleted.
type TreeValue = Option<StateValue>;

/// Converts itself to a set of updates for a merkle patricia trie.
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
            .collect::<HashMap<_, _>>()
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
    fn test_state_root_from_empty_tree_is_zero() {
        let actual_root = InMemoryState::new().state_root();
        let expected_root = B256::ZERO;

        assert_eq!(actual_root, expected_root);
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
