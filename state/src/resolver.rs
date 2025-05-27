use {
    crate::{evm_key_address, is_evm_storage_or_account_key, nodes::TreeKey},
    aptos_types::state_store::{state_key::StateKey, state_value::StateValue},
    bytes::Bytes,
    eth_trie::{DB, EthTrie, Trie, TrieError},
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        language_storage::{ModuleId, StructTag},
        metadata::Metadata,
        value::MoveTypeLayout,
        vm_status::StatusCode,
    },
    move_table_extension::{TableHandle, TableResolver},
    move_vm_types::resolver::{ModuleResolver, ResourceResolver},
    moved_shared::primitives::KeyHashable,
};

/// This is a [`MoveResolver`] that accesses blockchain state via [`EthTrie`].
///
/// If you pass it an [`EthTrie`] initialized at state root corresponding to certain older block
/// height, it will read from the blockchain state version at that block.
///
/// [`MoveResolver`]: move_vm_types::resolver::MoveResolver
#[derive(Debug)]
pub struct EthTrieResolver<D: DB> {
    tree: EthTrie<D>,
}

impl<D: DB> EthTrieResolver<D> {
    pub const fn new(tree: EthTrie<D>) -> Self {
        Self { tree }
    }

    pub const fn trie(&self) -> &EthTrie<D> {
        &self.tree
    }

    pub const fn trie_mut(&mut self) -> &mut EthTrie<D> {
        &mut self.tree
    }
}

impl<D: DB> ModuleResolver for EthTrieResolver<D> {
    fn get_module_metadata(&self, _module_id: &ModuleId) -> Vec<Metadata> {
        Vec::new()
    }

    fn get_module(&self, id: &ModuleId) -> Result<Option<Bytes>, PartialVMError> {
        let state_key = StateKey::module(id.address(), id.name());
        let key_hash = TreeKey::StateKey(state_key).key_hash();
        let value = self.tree.get(key_hash.0.as_slice()).map_err(trie_err)?;

        Ok(deserialize_state_value(value))
    }
}

impl<D: DB> ResourceResolver for EthTrieResolver<D> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> Result<(Option<Bytes>, usize), PartialVMError> {
        let tree_key = if let Some(address) = evm_key_address(struct_tag) {
            TreeKey::Evm(address)
        } else {
            let state_key = StateKey::resource(address, struct_tag)
                .inspect_err(|e| print!("{e:?}"))
                .map_err(|_| PartialVMError::new(StatusCode::DATA_FORMAT_ERROR))?;
            TreeKey::StateKey(state_key)
        };
        let key_hash = tree_key.key_hash();
        let value = self.tree.get(key_hash.0.as_slice()).map_err(trie_err)?;
        let value = if is_evm_storage_or_account_key(struct_tag) {
            // In the case of EVM there is no additional serialization
            value.map(Into::into)
        } else {
            deserialize_state_value(value)
        };
        let len = value.as_ref().map(|v| v.len()).unwrap_or_default();

        Ok((value, len))
    }
}

impl<D: DB> TableResolver for EthTrieResolver<D> {
    fn resolve_table_entry_bytes_with_layout(
        &self,
        handle: &TableHandle,
        key: &[u8],
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<Option<Bytes>, PartialVMError> {
        let state_key =
            StateKey::table_item(&aptos_types::state_store::table::TableHandle(handle.0), key);
        let tree_key = TreeKey::StateKey(state_key);
        let key_hash = tree_key.key_hash();
        let value = self.tree.get(key_hash.0.as_slice()).map_err(trie_err)?;
        let value = deserialize_state_value(value);

        Ok(value)
    }
}

fn deserialize_state_value(bytes: Option<Vec<u8>>) -> Option<Bytes> {
    let value: StateValue = bcs::from_bytes(&bytes?).expect("Bytes must be serialized StateValue");
    let (_, inner) = value.unpack();
    Some(inner)
}

fn trie_err(e: TrieError) -> PartialVMError {
    PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(format!("{e:?}"))
}
