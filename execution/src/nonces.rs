use {
    crate::session_id::SessionId,
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress, ident_str, identifier::IdentStr,
        language_storage::ModuleId, resolver::MoveResolver, value::MoveValue,
        vm_status::StatusCode,
    },
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        session::Session,
    },
    move_vm_types::{
        gas::{GasMeter, UnmeteredGasMeter},
        values::Value,
    },
    moved_evm_ext::state::StorageTrieRepository,
    moved_genesis::FRAMEWORK_ADDRESS,
    moved_shared::error::{Error, InvalidTransactionCause, NonceChecking},
};

const ACCOUNT_MODULE_NAME: &IdentStr = ident_str!("account");
const CREATE_ACCOUNT_FUNCTION_NAME: &IdentStr = ident_str!("create_account_if_does_not_exist");
const GET_NONCE_FUNCTION_NAME: &IdentStr = ident_str!("get_sequence_number");
const INCREMENT_NONCE_FUNCTION_NAME: &IdentStr = ident_str!("increment_sequence_number");

/// Useful in tests and queries. Do not use in transaction execution
/// since this method creates a new session and does not charge gas.
pub fn quick_get_nonce(
    address: &AccountAddress,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    storage_trie: &impl StorageTrieRepository,
) -> u64 {
    let move_vm = super::create_move_vm().expect("Must create MoveVM");
    let mut session =
        super::create_vm_session(&move_vm, state, SessionId::default(), storage_trie, &());
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
    )
    .unwrap_or_default()
}

pub fn check_nonce<G: GasMeter>(
    tx_nonce: u64,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
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

    session
        .execute_function_bypass_visibility(
            &account_module_id,
            INCREMENT_NONCE_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg.as_slice()],
            gas_meter,
            traversal_context,
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

fn get_account_nonce<G: GasMeter>(
    account_module_id: &ModuleId,
    addr_arg: &[u8],
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> Result<u64, Error> {
    let return_values = session
        .execute_function_bypass_visibility(
            account_module_id,
            GET_NONCE_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg],
            gas_meter,
            traversal_context,
        )
        .map_err(|_| Error::nonce_invariant_violation(NonceChecking::GetNonceAlwaysSucceeds))?
        .return_values;
    let (raw_output, layout) = return_values
        .first()
        .ok_or(Error::nonce_invariant_violation(
            NonceChecking::GetNonceReturnsAValue,
        ))?;
    let value = Value::simple_deserialize(raw_output, layout)
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
