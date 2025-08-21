use {
    super::{
        CODE_LAYOUT, EVM_NATIVE_ADDRESS,
        type_utils::{account_info_struct_tag, code_hash_struct_tag},
    },
    crate::{
        events::EthTransferLog,
        state::{self, BlockHashLookup, StorageTrieRepository},
        type_utils::get_move_account_nonce,
    },
    alloy::primitives::map::HashMap,
    aptos_types::vm_status::StatusCode,
    better_any::{Tid, TidAble},
    move_binary_format::errors::PartialVMError,
    move_core_types::account_address::AccountAddress,
    move_vm_types::{resolver::MoveResolver, value_serde::ValueSerDeContext, values::VMValueCast},
    revm::{
        DatabaseRef,
        context::BlockEnv,
        context_interface::block::BlobExcessGasAndPrice,
        database::CacheDB,
        primitives::{Address, B256, KECCAK_EMPTY, U256},
        state::{Account, AccountInfo, Bytecode},
    },
    std::fmt,
    umi_shared::primitives::ToMoveAddress,
};

pub const FRAMEWORK_ADDRESS: AccountAddress = AccountAddress::ONE;

/// A subset of the `Header` fields that are available while the transactions
/// in the block are being executed.
#[derive(Debug, Clone, Default)]
pub struct HeaderForExecution {
    pub number: u64,
    pub timestamp: u64,
    pub prev_randao: B256,
    pub chain_id: u64,
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
        block_hash_lookup: &'a dyn BlockHashLookup,
    ) -> Self {
        Self {
            resolver: state,
            storage_trie,
            transfer_logs,
            db: CacheDB::new(ResolverBackedDB::new(
                storage_trie,
                state,
                block_hash_lookup,
                block_header.number,
            )),
            state_changes: Vec::new(),
            block_header,
        }
    }

    pub fn block_env(&self) -> BlockEnv {
        BlockEnv {
            number: self.block_header.number,
            beneficiary: Address::ZERO,
            timestamp: self.block_header.timestamp,
            gas_limit: u64::MAX,
            basefee: 0,
            difficulty: U256::ZERO,
            prevrandao: Some(self.block_header.prev_randao),
            blob_excess_gas_and_price: Some(BlobExcessGasAndPrice {
                excess_blob_gas: 0,
                blob_gasprice: 0,
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ResolverBackedDB<'a> {
    storage_trie: &'a dyn StorageTrieRepository,
    resolver: &'a dyn MoveResolver,
    block_hash_lookup: &'a dyn BlockHashLookup,
    current_block_number: u64,
}

impl<'a> ResolverBackedDB<'a> {
    pub fn new(
        storage_trie: &'a dyn StorageTrieRepository,
        resolver: &'a dyn MoveResolver,
        block_hash_lookup: &'a dyn BlockHashLookup,
        current_block_number: u64,
    ) -> Self {
        Self {
            storage_trie,
            resolver,
            block_hash_lookup,
            current_block_number,
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

        let move_address = address.to_move_address();
        let maybe_nonce = get_move_account_nonce(&move_address, self.resolver);
        let account = match (value, maybe_nonce) {
            (Some(mut account), Some(nonce)) => {
                account.inner.nonce = nonce;
                Some(account)
            }
            (Some(account), None) => Some(account),
            (None, Some(nonce)) => {
                let inner = alloy::consensus::TrieAccount {
                    nonce,
                    ..Default::default()
                };
                Some(state::Account { inner })
            }
            (None, None) => None,
        };

        Ok(account)
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
        let Some(account) = self.get_account(&address)? else {
            return Ok(U256::ZERO);
        };
        let storage = self
            .storage_trie
            .for_account_with_root(&address, &account.inner.storage_root)?;
        let value = storage.get(&index)?;
        Ok(value.unwrap_or_default())
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        // `number` must be in the range [self.current_block_number - 256, self.current_block_number).
        let lower_bound = self.current_block_number.saturating_sub(256);
        let upper_bound = self.current_block_number;

        if lower_bound <= number && number < upper_bound {
            return Ok(self
                .block_hash_lookup
                .hash_by_number(number)
                .unwrap_or(B256::ZERO));
        }

        Ok(B256::ZERO)
    }
}
