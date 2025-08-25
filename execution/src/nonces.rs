use {
    crate::session_id::SessionId,
    alloy::primitives::Address,
    aptos_table_natives::TableResolver,
    move_core_types::{
        account_address::AccountAddress, ident_str, identifier::IdentStr,
        language_storage::ModuleId, value::MoveValue, vm_status::StatusCode,
    },
    move_vm_runtime::{
        AsUnsyncCodeStorage, ModuleStorage,
        module_traversal::{TraversalContext, TraversalStorage},
        session::Session,
    },
    move_vm_types::{
        gas::{GasMeter, UnmeteredGasMeter},
        resolver::MoveResolver,
        value_serde::ValueSerDeContext,
    },
    std::collections::HashMap,
    umi_evm_ext::state::StorageTrieRepository,
    umi_genesis::{CreateMoveVm, FRAMEWORK_ADDRESS, UmiVm},
    umi_shared::{
        error::{Error, InvalidTransactionCause, NonceChecking},
        primitives::ToMoveAddress,
    },
    umi_state::ResolverBasedModuleBytesStorage,
};

const ACCOUNT_MODULE_NAME: &IdentStr = umi_evm_ext::ACCOUNT_MODULE_NAME;
const CREATE_ACCOUNT_FUNCTION_NAME: &IdentStr = ident_str!("create_account_if_does_not_exist");
const GET_NONCE_FUNCTION_NAME: &IdentStr = ident_str!("get_sequence_number");
const INCREMENT_NONCE_FUNCTION_NAME: &IdentStr = ident_str!("increment_sequence_number");

/// Useful in tests and queries. Do not use in transaction execution
/// since this method creates a new session and does not charge gas.
pub fn quick_get_nonce(
    address: &AccountAddress,
    state: &(impl MoveResolver + TableResolver),
    storage_trie: &impl StorageTrieRepository,
) -> u64 {
    let umi_vm = UmiVm::new(&Default::default());
    let module_storage_bytes = ResolverBasedModuleBytesStorage::new(state);
    let code_storage = module_storage_bytes.as_unsync_code_storage(&umi_vm);
    let vm = umi_vm.create_move_vm().expect("Must create MoveVM");
    // Noop block hash lookup is safe here because the EVM is not used for
    // querying account nonces.
    let mut session =
        super::create_vm_session(&vm, state, SessionId::default(), storage_trie, &(), &());
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = UnmeteredGasMeter;
    let account_module_id = ModuleId::new(FRAMEWORK_ADDRESS, ACCOUNT_MODULE_NAME.into());
    let addr_arg = bcs::to_bytes(address).expect("address can serialize");
    get_account_nonce(
        &account_module_id,
        &addr_arg,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
        &code_storage,
    )
    .unwrap_or_default()
}

pub fn check_nonce<G: GasMeter, MS: ModuleStorage>(
    tx_nonce: u64,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &MS,
) -> Result<(), Error> {
    let account_module_id = ModuleId::new(FRAMEWORK_ADDRESS, ACCOUNT_MODULE_NAME.into());
    let addr_arg = bcs::to_bytes(signer).expect("address can serialize");

    session
        .execute_function_bypass_visibility(
            &account_module_id,
            CREATE_ACCOUNT_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg.as_slice()],
            gas_meter,
            traversal_context,
            module_storage,
        )
        .map_err(|e| {
            if e.major_status() == StatusCode::OUT_OF_GAS {
                Error::InvalidTransaction(InvalidTransactionCause::InsufficientIntrinsicGas)
            } else {
                Error::nonce_invariant_violation(NonceChecking::AnyAccountCanBeCreated)
            }
        })?;

    let account_nonce = get_account_nonce(
        &account_module_id,
        &addr_arg,
        session,
        traversal_context,
        gas_meter,
        module_storage,
    )?;

    if tx_nonce != account_nonce {
        Err(InvalidTransactionCause::IncorrectNonce {
            expected: account_nonce,
            given: tx_nonce,
        })?;
    }
    if account_nonce == u64::MAX {
        Err(InvalidTransactionCause::ExhaustedAccount)?;
    }

    Ok(())
}

pub fn increment_account_nonce<G: GasMeter, MS: ModuleStorage>(
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &MS,
) -> Result<(), Error> {
    let account_module_id = ModuleId::new(FRAMEWORK_ADDRESS, ACCOUNT_MODULE_NAME.into());
    let addr_arg = bcs::to_bytes(signer).expect("address can serialize");

    session
        .execute_function_bypass_visibility(
            &account_module_id,
            INCREMENT_NONCE_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg.as_slice()],
            gas_meter,
            traversal_context,
            module_storage,
        )
        .map_err(|e| {
            if e.major_status() == StatusCode::OUT_OF_GAS {
                Error::InvalidTransaction(InvalidTransactionCause::InsufficientIntrinsicGas)
            } else {
                Error::nonce_invariant_violation(NonceChecking::IncrementNonceAlwaysSucceeds)
            }
        })?;

    Ok(())
}

pub fn nonce_epilogue<MS: ModuleStorage>(
    signer: &AccountAddress,
    evm_nonces: HashMap<Address, u64>,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    module_storage: &MS,
) -> Result<(), Error> {
    // These actions are unmetered because they happen after we have
    // already charged for gas.
    let mut gas_meter = UnmeteredGasMeter;

    // The signer nonce must be incremented once regardless
    // of what is in the EVM. This prevents replaying the
    // current transaction.
    increment_account_nonce(
        signer,
        session,
        traversal_context,
        &mut gas_meter,
        module_storage,
    )?;

    for (address, nonce) in evm_nonces {
        let address = address.to_move_address();
        // If the Move nonce does not match the EVM nonce then we need to update
        if let Err(Error::InvalidTransaction(InvalidTransactionCause::IncorrectNonce {
            expected,
            given,
        })) = check_nonce(
            nonce,
            &address,
            session,
            traversal_context,
            &mut gas_meter,
            module_storage,
        ) {
            // Note: `expected != given` because they come from the
            // incorrect nonce error.
            if expected < given {
                // If the Move nonce is lower than the EVM nonce then we
                // must increment the Move nonce accordingly.
                let diff = given - expected;
                for _ in 0..diff {
                    increment_account_nonce(
                        &address,
                        session,
                        traversal_context,
                        &mut gas_meter,
                        module_storage,
                    )?;
                }
            } else {
                // It is impossible for the EVM nonce to be lower than
                // the Move nonce because the EVM reads the Move nonce
                // to fill the account info.
                // I.e. there is an invariant that `Move nonce <= EVM nonce`.
                unreachable!("Impossible: EVM nonce < Move nonce");
            }
        }
    }

    Ok(())
}

fn get_account_nonce<G: GasMeter, MS: ModuleStorage>(
    account_module_id: &ModuleId,
    addr_arg: &[u8],
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &MS,
) -> Result<u64, Error> {
    let return_values = session
        .execute_function_bypass_visibility(
            account_module_id,
            GET_NONCE_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg],
            gas_meter,
            traversal_context,
            module_storage,
        )
        .map_err(|_| Error::nonce_invariant_violation(NonceChecking::GetNonceAlwaysSucceeds))?
        .return_values;
    let (raw_output, layout) = return_values
        .first()
        .ok_or(Error::nonce_invariant_violation(
            NonceChecking::GetNonceReturnsAValue,
        ))?;
    let value = ValueSerDeContext::new()
        .deserialize(raw_output, layout)
        .ok_or(Error::nonce_invariant_violation(
            NonceChecking::GetNoneReturnDeserializes,
        ))?
        .as_move_value(layout);
    match value {
        MoveValue::U64(nonce) => Ok(nonce),
        _ => Err(Error::nonce_invariant_violation(
            NonceChecking::GetNonceReturnsU64,
        )),
    }
}
