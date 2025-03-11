use {
    super::{
        native_evm_context::NativeEVMContext,
        type_utils::{account_info_struct_tag, code_hash_struct_tag, get_account_code_hash},
        CODE_LAYOUT, EVM_NATIVE_ADDRESS,
    },
    crate::{
        storage::{StorageTrieChanges, StorageTrieRepository, StorageTriesChanges},
        trie_types,
    },
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        effects::{AccountChangeSet, ChangeSet, Op},
        language_storage::StructTag,
        resolver::MoveResolver,
    },
    move_vm_runtime::native_extensions::NativeContextExtensions,
    move_vm_types::values::Value,
    revm::primitives::{
        utilities::KECCAK_EMPTY, Account, AccountInfo, AccountStatus, Address, Bytecode,
        EvmStorageSlot, U256,
    },
};

#[derive(Debug)]
pub struct Changes {
    pub accounts: ChangeSet,
    pub storage: StorageTriesChanges,
}

impl Changes {
    pub fn new(accounts: ChangeSet, storage: StorageTriesChanges) -> Self {
        Self { accounts, storage }
    }
}

pub fn genesis_state_changes<'a>(
    genesis: alloy::genesis::Genesis,
    resolver: &impl MoveResolver<PartialVMError>,
    storage_trie: &impl StorageTrieRepository,
) -> Changes {
    let mut result = ChangeSet::new();
    let empty_changes = AccountChangeSet::new();
    let mut account_changes = AccountChangeSet::new();
    let mut storage_tries = StorageTriesChanges::empty();
    for (address, genesis_account) in genesis.alloc {
        let (code_hash, code) = match genesis_account.code {
            Some(raw) => {
                let code = Bytecode::new_legacy(raw);
                (code.hash_slow(), Some(code))
            }
            None => (KECCAK_EMPTY, None),
        };
        let storage = genesis_account
            .storage
            .map(|xs| {
                xs.into_iter()
                    .map(|(index, data)| {
                        let index = U256::from_be_bytes(index.0);
                        let data = U256::from_be_bytes(data.0);
                        let mut slot = EvmStorageSlot::new(data);
                        // Original value must be marked as 0 to make sure we
                        // know it is now a new value.
                        slot.original_value = U256::ZERO;
                        (index, slot)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let account = Account {
            info: AccountInfo {
                balance: genesis_account.balance,
                nonce: genesis_account.nonce.unwrap_or_default(),
                code_hash,
                code,
            },
            storage,
            status: AccountStatus::Touched,
        };
        let storage_changes = add_account_changes(
            &address,
            &account,
            resolver,
            &empty_changes,
            &mut account_changes,
            storage_trie,
        );
        storage_tries = storage_tries.with_trie_changes(address, storage_changes);
    }
    result
        .add_account_changeset(EVM_NATIVE_ADDRESS, account_changes)
        .expect("EVM native changes must be added");

    Changes::new(result, storage_tries)
}

pub fn extract_evm_changes<'a>(extensions: &NativeContextExtensions) -> Changes {
    let evm_native_ctx = extensions.get::<NativeEVMContext>();
    let mut evm_move_account_changes = ChangeSet::new();
    let mut account_changes = AccountChangeSet::new();
    let mut storage_tries = StorageTriesChanges::empty();
    for state in &evm_native_ctx.state_changes {
        let mut single_account_changes = AccountChangeSet::new();
        for (address, account) in state {
            // If the account is not touched then there are no changes.
            if !account.is_touched() {
                continue;
            }

            let storage_changes = add_account_changes(
                address,
                account,
                evm_native_ctx.resolver,
                &account_changes,
                &mut single_account_changes,
                evm_native_ctx.storage_trie,
            );
            storage_tries = storage_tries.with_trie_changes(*address, storage_changes);
        }
        account_changes
            .squash(single_account_changes)
            .expect("Sequential EVM native changes must merge");
    }
    evm_move_account_changes
        .add_account_changeset(EVM_NATIVE_ADDRESS, account_changes)
        .expect("EVM native changes must be added");

    Changes::new(evm_move_account_changes, storage_tries)
}

fn add_account_changes<'a>(
    address: &Address,
    account: &Account,
    resolver: &dyn MoveResolver<PartialVMError>,
    prior_changes: &AccountChangeSet,
    result: &mut AccountChangeSet,
    storage_trie: &dyn StorageTrieRepository,
) -> StorageTrieChanges {
    debug_assert!(
        account.is_touched(),
        "Untouched accounts are filtered out before calling this function."
    );

    if account.is_selfdestructed() {
        unimplemented!("EVM account self-destruct is not implemented");
    }

    let code_hash = get_account_code_hash(&account.info);

    let resource_exists = |struct_tag: &StructTag| {
        let exists_in_prior_changes = prior_changes.resources().contains_key(struct_tag);
        // Early exit since we don't need to check the resolver if it's in the prior changes.
        if exists_in_prior_changes {
            return exists_in_prior_changes;
        }
        // If not in the prior changes then check the resolver
        resolver
            .get_resource(&EVM_NATIVE_ADDRESS, struct_tag)
            .map(|x| x.is_some())
            .unwrap_or(false)
    };

    let mut storage = storage_trie.for_account(&address);
    for (index, value) in account.changed_storage_slots() {
        storage.insert(index, &value.present_value).unwrap();
    }
    let storage_changes = storage.commit().unwrap();

    // Push AccountInfo resource
    let struct_tag = account_info_struct_tag(address);
    let account_info = trie_types::Account::new(
        account.info.nonce,
        account.info.balance,
        account.info.code_hash,
        storage_changes.root,
    );
    let account_bytes = account_info.serialize();
    let is_created = !resource_exists(&struct_tag);
    let op = if is_created {
        Op::New(account_bytes.into())
    } else {
        Op::Modify(account_bytes.into())
    };
    result
        .add_resource_op(struct_tag, op)
        .expect("Resource cannot already exist in result");

    // Push CodeHash resource if needed.
    // We don't need to push anything if the resource already exists.
    let struct_tag = code_hash_struct_tag(&code_hash);
    let code_resource_exists = resource_exists(&struct_tag);
    if !code_resource_exists {
        if let Some(code) = &account.info.code {
            if !code.is_empty() {
                let struct_tag = code_hash_struct_tag(&code_hash);
                let code = Value::vector_u8(code.original_bytes())
                    .simple_serialize(&CODE_LAYOUT)
                    .expect("EVM code must serialize");
                let op = Op::New(code.into());
                // If the same contract is deployed more than once then the same resource
                // could be added twice, but that's ok we can just skip the duplicate.
                result.add_resource_op(struct_tag, op).ok();
            }
        }
    }

    storage_changes
}
