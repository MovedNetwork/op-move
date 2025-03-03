use {
    super::{
        trie_types,
        type_utils::{account_info_struct_tag, account_storage_struct_tag, code_hash_struct_tag},
        CODE_LAYOUT, EVM_NATIVE_ADDRESS,
    },
    alloy::primitives::map::HashMap,
    aptos_types::vm_status::StatusCode,
    better_any::{Tid, TidAble},
    move_binary_format::errors::PartialVMError,
    move_core_types::{account_address::AccountAddress, resolver::MoveResolver},
    move_vm_types::values::{VMValueCast, Value},
    revm::{
        db::{CacheDB, DatabaseRef},
        primitives::{
            utilities::KECCAK_EMPTY, Account, AccountInfo, Address, Bytecode, B256, U256,
        },
    },
    std::sync::RwLock,
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
    pub db: CacheDB<ResolverBackedDB<'a>>,
    pub state_changes: Vec<HashMap<Address, Account>>,
    pub block_header: HeaderForExecution,
}

impl<'a> NativeEVMContext<'a> {
    pub fn new(
        state: &'a impl MoveResolver<PartialVMError>,
        block_header: HeaderForExecution,
    ) -> Self {
        let inner_db = ResolverBackedDB::new(state);
        let db = CacheDB::new(inner_db);
        Self {
            resolver: state,
            db,
            state_changes: Vec::new(),
            block_header,
        }
    }
}

pub struct ResolverBackedDB<'a> {
    resolver: &'a dyn MoveResolver<PartialVMError>,
    // This cache is used because each EVM account has a single resource for all
    // its storage slots and therefore may be a large amount of data that takes
    // a non-trivial amount of time to deserialize. By caching the storage representation
    // in memory we only have to do the deserialization once per transaction. This
    // optimization is likely helpful because it is common to access more than one
    // storage slot for an EVM smart contract even in a single transaction. In the
    // future if we choose to split storage across multiple resources to limit the
    // size of a single resource, a cache will still be useful since reconstructing
    // the storage from all the resource pieces will also be non-trivial.
    //
    // The cache must be wrapped in a type that allows interior mutability
    // because the `DatabaseRef` interface uses immutable references. Since this
    // cache is only used for a single transaction execution it is unlikely that
    // it will ever need to multi-threaded access, so a thread-unsafe type like
    // `RefCell` would be sufficient. But at the same time, I don't think the overhead
    // of `RwLock` is that large, so I think it is ok to use.
    storage_cache: RwLock<HashMap<Address, trie_types::AccountStorage>>,
}

impl<'a> ResolverBackedDB<'a> {
    pub fn new(resolver: &'a impl MoveResolver<PartialVMError>) -> Self {
        Self {
            resolver,
            storage_cache: RwLock::new(HashMap::default()),
        }
    }

    pub fn storage_for(
        &self,
        address: &Address,
    ) -> Result<trie_types::AccountStorage, PartialVMError> {
        let struct_tag = account_storage_struct_tag(address);
        match self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?
        {
            Some(bytes) => {
                let storage = trie_types::AccountStorage::try_deserialize(&bytes)
                    .expect("EVM account storage must deserialize correctly");
                Ok(storage)
            }
            None => Ok(trie_types::AccountStorage::default()),
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
        let mut cache_lock = self
            .storage_cache
            .write()
            .expect("ResolverBackedDB::storage_cache not poisoned");

        if let Some(storage) = cache_lock.get(&address) {
            return Ok(storage.get(index));
        }

        let storage = self.storage_for(&address)?;
        let value = storage.get(index);
        cache_lock.insert(address, storage);
        Ok(value)
    }

    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        // Complication: Move doesn't support this API out of the box.
        // We could build it out ourselves, but maybe it's not needed
        // for the contracts we want to support?

        unimplemented!("EVM block hash API not implemented")
    }
}
