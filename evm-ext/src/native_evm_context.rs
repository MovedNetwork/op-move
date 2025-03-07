use {
    super::{
        trie_types,
        type_utils::{account_info_struct_tag, code_hash_struct_tag},
        CODE_LAYOUT, EVM_NATIVE_ADDRESS,
    },
    crate::{storage, storage::StorageTrieRepository},
    alloy::primitives::map::HashMap,
    aptos_types::vm_status::StatusCode,
    better_any::{Tid, TidAble},
    eth_trie::DB,
    move_binary_format::errors::PartialVMError,
    move_core_types::{account_address::AccountAddress, resolver::MoveResolver},
    move_vm_types::values::{VMValueCast, Value},
    revm::{
        db::{CacheDB, DatabaseRef},
        primitives::{
            utilities::KECCAK_EMPTY, Account, AccountInfo, Address, Bytecode, B256, U256,
        },
    },
    std::{error::Error, sync::RwLock},
};

pub const FRAMEWORK_ADDRESS: AccountAddress = AccountAddress::ONE;

/// A subset of the `Header` fields that are available while the transactions
/// in the block are being executed.
#[derive(Debug, Clone, Default)]
pub struct HeaderForExecution {
    pub number: u64,
    pub timestamp: u64,
    pub prev_randao: B256,
}

#[derive(Tid)]
pub struct NativeEVMContext<'a> {
    pub resolver: &'a dyn MoveResolver<PartialVMError>,
    pub storage_trie: Box<dyn StorageTrieRepository>,
    // pub storage_trie: &'a dyn StorageTrieRepository<Storage = D>,
    pub state_changes: Vec<HashMap<Address, Account>>,
    pub block_header: HeaderForExecution,
}

impl<'a> NativeEVMContext<'a> {
    pub fn new(
        state: &'a impl MoveResolver<PartialVMError>,
        storage_trie: Box<dyn StorageTrieRepository>,
        block_header: HeaderForExecution,
    ) -> Self {
        Self {
            resolver: state,
            storage_trie,
            state_changes: Vec::new(),
            block_header,
        }
    }
}

pub struct ResolverBackedDB<'a> {
    storage_trie: &'a dyn StorageTrieRepository,
    resolver: &'a dyn MoveResolver<PartialVMError>,
}

impl<'a> ResolverBackedDB<'a> {
    pub fn new(
        storage_trie: &'a impl StorageTrieRepository,
        resolver: &'a dyn MoveResolver<PartialVMError>,
    ) -> Self {
        Self {
            storage_trie,
            resolver,
        }
    }

    pub fn get_account(
        &self,
        address: &Address,
    ) -> Result<Option<trie_types::Account>, PartialVMError> {
        let struct_tag = account_info_struct_tag(address);
        let resource = self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?;
        let value = resource.map(|bytes| {
            trie_types::Account::try_deserialize(&bytes)
                .expect("EVM account info must deserialize correctly.")
        });
        Ok(value)
    }
}

impl<'a> DatabaseRef for ResolverBackedDB<'a> {
    type Error = PartialVMError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let value = self.get_account(&address)?;
        let info = value.map(Into::into);
        Ok(info)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if code_hash == KECCAK_EMPTY {
            return Ok(Bytecode::new_legacy(Vec::new().into()));
        }

        let struct_tag = code_hash_struct_tag(&code_hash);
        let resource = self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::MISSING_DATA).with_message(format!(
                    "Missing EVM code corresponding to code hash {}",
                    struct_tag.name
                ))
            })?;
        let value = Value::simple_deserialize(&resource, &CODE_LAYOUT)
            .expect("EVM account info must deserialize correctly.");
        let bytes: Vec<u8> = value.cast()?;
        Ok(Bytecode::new_legacy(bytes.into()))
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let storage = self.storage_trie.for_account(&address);
        let value = storage.get(&index).unwrap();
        Ok(value.unwrap_or_default())
    }

    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        // Complication: Move doesn't support this API out of the box.
        // We could build it out ourselves, but maybe it's not needed
        // for the contracts we want to support?

        unimplemented!("EVM block hash API not implemented")
    }
}
