use {
    super::{
        CODE_LAYOUT, EVM_NATIVE_ADDRESS,
        type_utils::{account_info_struct_tag, code_hash_struct_tag},
    },
    crate::{
        events::EthTransferLog,
        state::{self, StorageTrieRepository},
    },
    alloy::primitives::map::HashMap,
    aptos_types::vm_status::StatusCode,
    better_any::{Tid, TidAble},
    move_binary_format::errors::PartialVMError,
    move_core_types::account_address::AccountAddress,
    move_vm_types::{resolver::MoveResolver, value_serde::ValueSerDeContext, values::VMValueCast},
    revm::{
        DatabaseRef,
        database::CacheDB,
        primitives::{Address, B256, KECCAK_EMPTY, U256},
        state::{Account, AccountInfo, Bytecode},
    },
    std::fmt,
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
    pub resolver: &'a dyn MoveResolver,
    pub storage_trie: &'a dyn StorageTrieRepository,
    pub transfer_logs: &'a dyn EthTransferLog,
    // Keep the DB in `NativeEVMContext` so that the EVM storage is consistent
    // for the whole MoveVM session; even if the EVM is invoked multiple times
    // (this could happen if a Move script calls the EVM multiple times for example).
    pub db: CacheDB<ResolverBackedDB<'a>>,
    pub state_changes: Vec<HashMap<Address, Account>>,
    pub block_header: HeaderForExecution,
}

impl<'a> NativeEVMContext<'a> {
    pub fn new(
        state: &'a impl MoveResolver,
        storage_trie: &'a impl StorageTrieRepository,
        transfer_logs: &'a dyn EthTransferLog,
        block_header: HeaderForExecution,
    ) -> Self {
        Self {
            resolver: state,
            storage_trie,
            transfer_logs,
            db: CacheDB::new(ResolverBackedDB::new(storage_trie, state)),
            state_changes: Vec::new(),
            block_header,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ResolverBackedDB<'a> {
    storage_trie: &'a dyn StorageTrieRepository,
    resolver: &'a dyn MoveResolver,
}

impl<'a> ResolverBackedDB<'a> {
    pub fn new(
        storage_trie: &'a dyn StorageTrieRepository,
        resolver: &'a dyn MoveResolver,
    ) -> Self {
        Self {
            storage_trie,
            resolver,
        }
    }

    pub fn get_account(&self, address: &Address) -> Result<Option<state::Account>, PartialVMError> {
        let struct_tag = account_info_struct_tag(address);
        let meta_data = self.resolver.get_module_metadata(&struct_tag.module_id());
        let resource = self
            .resolver
            .get_resource_bytes_with_metadata_and_layout(
                &EVM_NATIVE_ADDRESS,
                &struct_tag,
                &meta_data,
                None,
            )?
            .0;
        let value = resource.map(|bytes| {
            state::Account::try_deserialize(&bytes)
                .expect("EVM account info must deserialize correctly.")
        });
        Ok(value)
    }
}

#[derive(Debug, Clone)]
pub struct DbError {
    pub inner: PartialVMError,
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for DbError {}

impl revm::database::DBErrorMarker for DbError {}

impl From<PartialVMError> for DbError {
    fn from(value: PartialVMError) -> Self {
        Self { inner: value }
    }
}

impl From<state::Error> for DbError {
    fn from(e: state::Error) -> Self {
        let inner = PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(format!("{e:?}"));
        Self { inner }
    }
}

impl DatabaseRef for ResolverBackedDB<'_> {
    type Error = DbError;

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
        let meta_data = self.resolver.get_module_metadata(&struct_tag.module_id());
        let resource = self
            .resolver
            .get_resource_bytes_with_metadata_and_layout(
                &EVM_NATIVE_ADDRESS,
                &struct_tag,
                &meta_data,
                None,
            )?
            .0
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::MISSING_DATA).with_message(format!(
                    "Missing EVM code corresponding to code hash {}",
                    struct_tag.name
                ))
            })?;
        let value = ValueSerDeContext::new()
            .deserialize(&resource, &CODE_LAYOUT)
            .expect("EVM account info must deserialize correctly.");
        let bytes: Vec<u8> = value.cast()?;
        Ok(Bytecode::new_legacy(bytes.into()))
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let storage = self.storage_trie.for_account(&address)?;
        let value = storage.get(&index)?;
        Ok(value.unwrap_or_default())
    }

    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        // Complication: Move doesn't support this API out of the box.
        // We could build it out ourselves, but maybe it's not needed
        // for the contracts we want to support?

        unimplemented!("EVM block hash API not implemented")
    }
}
