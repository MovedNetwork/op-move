pub mod nodes;

mod diff;
mod in_memory;
mod resolver;
mod skip_list;
mod state;

pub use {
    diff::Changes, in_memory::InMemoryState, resolver::EthTrieResolver, state::EthTrieState,
    umi_evm_ext::state::InMemoryDb as InMemoryTrieDb,
};

use {
    alloy::hex::FromHex,
    aptos_types::state_store::{state_key::StateKey, state_value::StateValue},
    bytes::Bytes,
    eth_trie::{DB, EthTrie, Trie, TrieError},
    move_binary_format::errors::{Location, VMResult},
    move_core_types::{
        account_address::AccountAddress,
        identifier::IdentStr,
        language_storage::{ModuleId, StructTag},
    },
    move_table_extension::TableResolver,
    move_vm_types::{code::ModuleBytesStorage, resolver::MoveResolver},
    nodes::{TreeKey, TreeValue},
    std::{collections::HashMap, fmt::Debug},
    umi_evm_ext::{EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE, type_utils::ACCOUNT_INFO_PREFIX},
    umi_shared::primitives::{Address, B256, KeyHashable},
};

/// A global blockchain state trait.
///
/// This trait is defined by these operations:
/// * [`resolver`]: Creates [`MoveResolver`] that can resolve both resources and modules.
/// * [`state_root`]: Returns current state root.
/// * [`apply`]: Applies changes produced by a transaction on the state trie.
///
/// [`resolver`]: Self::resolver
/// [`state_root`]: Self::state_root
/// [`apply`]: Self::apply
pub trait State {
    /// The associated error that can occur on storage operations.
    type Err: Debug;

    /// Applies the `changes` to the blockchain state.
    fn apply(&mut self, changes: Changes) -> Result<(), Self::Err>;

    /// Returns a reference to a [`MoveResolver`] that can resolve both resources and modules on
    /// the current blockchain state.
    fn resolver(&self) -> &(impl MoveResolver + TableResolver);

    /// Retrieves the value of the root node of the merkle trie that holds the blockchain state.
    fn state_root(&self) -> B256;
}

pub trait InsertChangeSetIntoMerkleTrie {
    type Err: Debug;

    fn insert_change_set_into_merkle_trie(
        &mut self,
        change_set: &Changes,
    ) -> Result<B256, Self::Err>;
}

impl<D: DB> InsertChangeSetIntoMerkleTrie for EthTrie<D> {
    type Err = TrieError;

    fn insert_change_set_into_merkle_trie(
        &mut self,
        change_set: &Changes,
    ) -> Result<B256, Self::Err> {
        let values = change_set.to_tree_values();

        for (k, v) in values {
            let key_bytes = k.key_hash();
            let value_bytes = v.serialize();
            self.insert(key_bytes.0.as_slice(), &value_bytes)?;
        }

        self.root_hash()
    }
}

/// Converts itself to a set of updates for a merkle patricia trie.
///
/// This trait is defined by a single operation called [`Self::to_tree_values`].
pub trait ToTreeValues {
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

impl ToTreeValues for Changes {
    fn to_tree_values(&self) -> HashMap<TreeKey, TreeValue> {
        self.accounts
            .accounts()
            .iter()
            .flat_map(|(address, changes)| {
                changes
                    .modules()
                    .iter()
                    .map(move |(k, v)| {
                        let value = v.clone().ok().map(StateValue::new_legacy);
                        let key = StateKey::module(address, k.as_ident_str());

                        (
                            TreeKey::StateKey(key),
                            value
                                .map(TreeValue::StateValue)
                                .unwrap_or(TreeValue::Deleted),
                        )
                    })
                    .chain(changes.resources().iter().map(move |(k, v)| {
                        let value = if is_evm_storage_or_account_key(k) {
                            v.clone()
                                .ok()
                                .map(TreeValue::Evm)
                                .unwrap_or(TreeValue::Deleted)
                        } else {
                            v.clone()
                                .ok()
                                .map(StateValue::new_legacy)
                                .map(TreeValue::StateValue)
                                .unwrap_or(TreeValue::Deleted)
                        };
                        let key = if let Some(address) = evm_key_address(k) {
                            TreeKey::Evm(address)
                        } else {
                            TreeKey::StateKey(
                                StateKey::resource(address, k)
                                    .expect("Creating a resource state key is infallible"),
                            )
                        };

                        (key, value)
                    }))
            })
            .chain(self.tables.changes.iter().flat_map(|(handle, changes)| {
                let handle = handle.into();
                changes.entries.iter().map(move |(id, op)| {
                    let key = StateKey::table_item(&handle, id);
                    let value = op
                        .clone()
                        .ok()
                        .map(|(bytes, _)| StateValue::new_legacy(bytes));
                    (
                        TreeKey::StateKey(key),
                        value
                            .map(TreeValue::StateValue)
                            .unwrap_or(TreeValue::Deleted),
                    )
                })
            }))
            .collect::<HashMap<_, _>>()
    }
}

pub fn evm_key_address(k: &StructTag) -> Option<Address> {
    if k.address == EVM_NATIVE_ADDRESS && k.module.as_ident_str() == EVM_NATIVE_MODULE {
        k.name
            .as_str()
            .strip_prefix(ACCOUNT_INFO_PREFIX)
            .and_then(|h| Address::from_hex(h).ok())
    } else {
        None
    }
}

pub fn is_evm_storage_or_account_key(k: &StructTag) -> bool {
    k.address == EVM_NATIVE_ADDRESS
        && k.module.as_ident_str() == EVM_NATIVE_MODULE
        && k.name.as_str().starts_with(ACCOUNT_INFO_PREFIX)
}

pub struct ResolverBasedModuleBytesStorage<'a, R> {
    resolver: &'a R,
}

impl<'a, R: MoveResolver> ResolverBasedModuleBytesStorage<'a, R> {
    pub fn new(resolver: &'a R) -> Self {
        Self { resolver }
    }
}

impl<R: MoveResolver> ModuleBytesStorage for ResolverBasedModuleBytesStorage<'_, R> {
    fn fetch_module_bytes(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> VMResult<Option<Bytes>> {
        let module_id = ModuleId::new(*address, module_name.to_owned());
        self.resolver
            .get_module(&module_id)
            .map_err(|e| e.finish(Location::Module(module_id)))
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
        let actual_root = InMemoryState::default().state_root();
        let expected_root = B256::ZERO;

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_insert_to_empty_tree_produces_new_state_root() {
        let mut state = InMemoryState::default();
        let mut change_set = Changes::empty();

        change_set
            .accounts
            .add_account_changeset(AccountAddress::new([0; 32]), AccountChanges::new())
            .unwrap();

        state.apply(change_set).unwrap();
        let empty_state_root = B256::ZERO;
        let actual_state_root = state.state_root();

        assert_ne!(actual_state_root, empty_state_root);
    }

    #[test]
    fn test_state_root_is_different_after_update_changes_trie() {
        let mut state = InMemoryState::default();
        let mut change_set = Changes::empty();

        change_set
            .accounts
            .add_account_changeset(AccountAddress::new([0; 32]), AccountChanges::new())
            .unwrap();
        state.apply(change_set).unwrap();
        let old_state_root = state.state_root();

        let mut change_set = Changes::empty();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();
        change_set
            .accounts
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state.apply(change_set).unwrap();
        let new_state_root = state.state_root();

        assert_ne!(old_state_root, new_state_root);
    }

    #[test]
    fn test_state_root_remains_the_same_when_update_does_not_change_trie() {
        let mut state = InMemoryState::default();
        let mut change_set = Changes::empty();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();

        change_set
            .accounts
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state
            .trie_mut()
            .insert_change_set_into_merkle_trie(&change_set)
            .unwrap();
        let expected_state_root = state.state_root();

        let mut change_set = Changes::empty();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();
        change_set
            .accounts
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state
            .trie_mut()
            .insert_change_set_into_merkle_trie(&change_set)
            .unwrap();
        let actual_state_root = state.state_root();

        assert_eq!(actual_state_root, expected_state_root);
    }
}
