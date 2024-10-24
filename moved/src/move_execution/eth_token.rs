use {
    crate::{genesis::FRAMEWORK_ADDRESS, EthToken},
    move_core_types::{
        account_address::AccountAddress, ident_str, identifier::IdentStr,
        language_storage::ModuleId, value::MoveValue,
    },
    move_vm_runtime::{module_traversal::TraversalContext, session::Session},
    move_vm_types::{gas::GasMeter, values::Value},
};

#[cfg(test)]
use {
    crate::types::session_id::SessionId, aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError, move_core_types::resolver::MoveResolver,
    move_vm_runtime::module_traversal::TraversalStorage, move_vm_types::gas::UnmeteredGasMeter,
};

const TOKEN_ADMIN: AccountAddress = FRAMEWORK_ADDRESS;
const TOKEN_MODULE_NAME: &IdentStr = ident_str!("eth_token");
const MINT_FUNCTION_NAME: &IdentStr = ident_str!("mint");
const GET_BALANCE_FUNCTION_NAME: &IdentStr = ident_str!("get_balance");
const TRANSFER_FUNCTION_NAME: &IdentStr = ident_str!("transfer");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferArgs<'a> {
    pub from: &'a AccountAddress,
    pub to: &'a AccountAddress,
    pub amount: u64,
}

pub fn mint_eth<G: GasMeter>(
    to: &AccountAddress,
    amount: u64,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> Result<(), crate::Error> {
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let to_arg = bcs::to_bytes(to).expect("address can serialize");
    let amount_arg = bcs::to_bytes(&MoveValue::U64(amount)).expect("amount can serialize");

    session
        .execute_entry_function(
            &token_module_id,
            MINT_FUNCTION_NAME,
            Vec::new(),
            vec![
                admin_arg.as_slice(),
                to_arg.as_slice(),
                amount_arg.as_slice(),
            ],
            gas_meter,
            traversal_context,
        )
        .map_err(|e| {
            println!("{e:?}");
            crate::Error::eth_token_invariant_violation(EthToken::MintAlwaysSucceeds)
        })?;

    Ok(())
}

pub fn transfer_eth<G: GasMeter>(
    args: TransferArgs<'_>,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> Result<(), crate::Error> {
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let from_arg = bcs::to_bytes(args.from).expect("from address can serialize");
    let to_arg = bcs::to_bytes(args.to).expect("to address can serialize");
    let amount_arg = bcs::to_bytes(&MoveValue::U64(args.amount)).expect("amount can serialize");

    // Note: transfer function can fail if user has insufficient balance.
    session.execute_entry_function(
        &token_module_id,
        TRANSFER_FUNCTION_NAME,
        Vec::new(),
        vec![
            admin_arg.as_slice(),
            from_arg.as_slice(),
            to_arg.as_slice(),
            amount_arg.as_slice(),
        ],
        gas_meter,
        traversal_context,
    )?;

    Ok(())
}

pub fn get_eth_balance<G: GasMeter>(
    account: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> Result<u64, crate::Error> {
    let addr_arg = bcs::to_bytes(account).expect("address can serialize");
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());

    let return_values = session
        .execute_function_bypass_visibility(
            &token_module_id,
            GET_BALANCE_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg.as_slice()],
            gas_meter,
            traversal_context,
        )
        .map_err(|_| {
            crate::Error::eth_token_invariant_violation(EthToken::GetBalanceAlwaysSucceeds)
        })?
        .return_values;

    let (raw_output, layout) =
        return_values
            .first()
            .ok_or(crate::Error::eth_token_invariant_violation(
                EthToken::GetBalanceReturnsAValue,
            ))?;

    let value = Value::simple_deserialize(raw_output, layout)
        .ok_or(crate::Error::eth_token_invariant_violation(
            EthToken::GetBalanceReturnDeserializes,
        ))?
        .as_move_value(layout);

    match value {
        MoveValue::U64(balance) => Ok(balance),
        _ => Err(crate::Error::eth_token_invariant_violation(
            EthToken::GetBalanceReturnsU64,
        )),
    }
}

/// Simplified API for getting the base token balance for use in tests.
#[cfg(test)]
pub fn quick_get_eth_balance(
    account: &AccountAddress,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
) -> u64 {
    let move_vm = super::create_move_vm().unwrap();
    let mut session = super::create_vm_session(&move_vm, state, SessionId::default());
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = UnmeteredGasMeter;
    get_eth_balance(
        account,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
    )
    .unwrap()
}
