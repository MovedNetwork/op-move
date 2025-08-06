use {
    crate::state::{MoveModule, MoveModuleResponse},
    alloy::{
        consensus::constants::KECCAK_EMPTY,
        primitives::keccak256,
        rpc::types::{EIP1186AccountProofResponse, EIP1186StorageProof},
    },
    eth_trie::{DB, EthTrie, Trie},
    move_binary_format::{CompiledModule, errors::PartialVMError},
    move_core_types::{
        account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
        vm_status::StatusCode,
    },
    move_table_extension::TableResolver,
    move_vm_types::{
        resolver::{ModuleResolver, MoveResolver, ResourceResolver},
        value_serde::ValueSerDeContext,
        values::VMValueCast,
    },
    std::error,
    umi_evm_ext::{
        CODE_LAYOUT, EVM_NATIVE_ADDRESS, ResolverBackedDB,
        state::{self, StorageTrieRepository},
        type_utils::{account_info_struct_tag, code_hash_struct_tag},
    },
    umi_execution::{quick_get_eth_balance, quick_get_nonce},
    umi_shared::primitives::{Address, B256, Bytes, KeyHashable, ToEthAddress, U256},
    umi_state::nodes::TreeKey,
};

pub type ProofResponse = EIP1186AccountProofResponse;
pub type StorageProof = EIP1186StorageProof;

/// A non-negative integer for indicating the amount of base token on an account.
pub type Balance = U256;

/// A non-negative integer for indicating the nonce used for sending transactions by an account.
pub type Nonce = u64;

/// A non-negative integer for indicating the order of a block in the blockchain, used as a tag for
/// [`Version`].
pub type BlockHeight = u64;

/// A non-negative integer for versioning a set of changes in a historical order.
///
/// Typically, each version matches one transaction, but there is an exception for changes generated
/// on genesis.
pub type Version = u64;

/// Accesses blockchain state in any particular point in history to fetch some account values.
///
/// It is defined by these operations:
/// * [`Self::balance_at`] - To fetch an amount of base token in an account read in its smallest
///   denomination at given block height.
/// * [`Self::nonce_at`] - To fetch the nonce value set for an account at given block height.
/// * [`Self::proof_at`] - To fetch the account and storage values of the specified account
///   including the Merkle proof.
pub trait StateQueries {
    /// Queries the blockchain state version corresponding with block `height` for the amount of
    /// base token associated with `account`.
    fn balance_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> umi_shared::error::Result<Balance> {
        let resolver = self.resolver_at(height)?;

        quick_get_eth_balance(&account, &resolver, evm_storage)
    }

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Result<Nonce, state::Error> {
        let resolver = self.resolver_at(height)?;

        Ok(quick_get_nonce(&account, &resolver, evm_storage))
    }

    /// Queries the blockchain state version corresponding with block `height` for the
    /// account and storage proofs associated with `account`.
    fn proof_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Result<ProofResponse, state::Error>;

    /// Queries the blockchain state version corresponding with block `height` for the
    /// `account` EVM bytecode.
    fn evm_bytecode_at(
        &self,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Result<Option<Bytes>, state::Error> {
        let address = account.to_eth_address();
        let resolver = self.resolver_at(height)?;
        let struct_tag = account_info_struct_tag(&address);

        let meta_data = resolver.get_module_metadata(&struct_tag.module_id());
        let (Some(resource), _) = resolver.get_resource_bytes_with_metadata_and_layout(
            &EVM_NATIVE_ADDRESS,
            &struct_tag,
            &meta_data,
            None,
        )?
        else {
            return Ok(None);
        };

        let account = state::Account::try_deserialize(&resource)
            .expect("EVM account info should be deserializable");

        let code_hash = account.inner.code_hash;

        if code_hash == KECCAK_EMPTY {
            return Ok(Some(Bytes::new()));
        }

        let struct_tag = code_hash_struct_tag(&code_hash);
        let meta_data = resolver.get_module_metadata(&struct_tag.module_id());
        let resource = resolver
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
            .expect("EVM bytecode should be deserializable");
        let bytes: Vec<u8> = value.cast()?;

        Ok(Some(bytes.into()))
    }

    fn move_module_at(
        &self,
        account: AccountAddress,
        name: &str,
        height: BlockHeight,
    ) -> Result<Option<MoveModuleResponse>, state::Error> {
        let Ok(ident) = Identifier::new(name) else {
            return Ok(None);
        };
        let module_id = ModuleId::new(account, ident);
        let resolver = self.resolver_at(height)?;
        let Some(bytes) = resolver.get_module(&module_id)? else {
            return Ok(None);
        };

        // A transaction module payload can contain invalid bytecode.
        // Ignore the error in that case and omit ABI in the response.
        let abi = CompiledModule::deserialize(bytes.as_ref())
            .ok()
            .map(MoveModule::from);

        Ok(Some(MoveModuleResponse {
            bytecode: bytes.to_vec().into(),
            abi,
        }))
    }

    /// Queries the blockchain state version corresponding with block `height` for the value of a
    /// single EVM storage slot `index` at `account`.
    fn evm_storage_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: Address,
        index: U256,
        height: BlockHeight,
    ) -> Result<U256, state::Error> {
        let resolver = self.resolver_at(height)?;

        // Read account info to get the storage root
        let evm_db = ResolverBackedDB::new(evm_storage, &resolver, &(), height);
        let Some(account_info) = evm_db.get_account(&account)? else {
            return Ok(U256::ZERO);
        };

        // Read the slot from the account's storage trie
        let storage =
            evm_storage.for_account_with_root(&account, &account_info.inner.storage_root)?;
        Ok(storage.get(&index)?.unwrap_or_default())
    }

    fn move_list_modules(
        &self,
        account: AccountAddress,
        height: BlockHeight,
        after: Option<&Identifier>,
        limit: u32,
    ) -> Result<Vec<Identifier>, state::Error>;

    fn resolver_at(
        &self,
        height: BlockHeight,
    ) -> Result<impl MoveResolver + TableResolver + '_, state::Error>;
}

pub trait HeightToStateRootIndex {
    type Err: error::Error;
    fn root_by_height(&self, height: BlockHeight) -> Result<Option<B256>, Self::Err>;
    fn height(&self) -> Result<BlockHeight, Self::Err>;
    fn push_state_root(&self, state_root: B256) -> Result<(), Self::Err>;
}

pub fn proof_from_trie_and_resolver(
    address: Address,
    storage_slots: &[U256],
    tree: &mut EthTrie<impl DB>,
    resolver: &impl MoveResolver,
    storage_trie: &impl StorageTrieRepository,
) -> Result<ProofResponse, state::Error> {
    let evm_db = ResolverBackedDB::new(storage_trie, resolver, &(), 0);

    // All L2 contract account data is part of the EVM state
    let account_info = evm_db
        .get_account(&address)?
        .ok_or(state::Error::AccountNotFound(address))?;

    let account_key = TreeKey::Evm(address);
    let account_proof = tree
        .get_proof(account_key.key_hash().0.as_slice())?
        .into_iter()
        .map(Into::into)
        .collect();

    let storage_proof = if storage_slots.is_empty() {
        Vec::new()
    } else {
        let mut storage =
            storage_trie.for_account_with_root(&address, &account_info.inner.storage_root)?;

        storage_slots
            .iter()
            .map(|index| {
                let key = keccak256::<[u8; 32]>(index.to_be_bytes());
                let proof = storage.proof(key.as_slice())?;
                let value = storage.get(index)?.unwrap_or_default();

                Ok(StorageProof {
                    key: (*index).into(),
                    value,
                    proof: proof.into_iter().map(Into::into).collect(),
                })
            })
            .collect::<Result<Vec<_>, state::Error>>()?
    };

    Ok(ProofResponse {
        address,
        balance: account_info.inner.balance,
        code_hash: account_info.inner.code_hash,
        nonce: account_info.inner.nonce,
        storage_hash: account_info.inner.storage_root,
        account_proof,
        storage_proof,
    })
}
