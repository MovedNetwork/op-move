use {
    crate::{
        genesis::FRAMEWORK_ADDRESS,
        primitives::{ToMoveU256, ToU256},
        types::session_id::SessionId,
        EthToken,
    },
    alloy::primitives::U256,
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress, ident_str, identifier::IdentStr,
        language_storage::ModuleId, resolver::MoveResolver, value::MoveValue,
    },
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        session::Session,
    },
    move_vm_types::{
        gas::{GasMeter, UnmeteredGasMeter},
        values::Value,
    },
};

const TOKEN_ADMIN: AccountAddress = FRAMEWORK_ADDRESS;
const TOKEN_MODULE_NAME: &IdentStr = ident_str!("eth_token");
const MINT_FUNCTION_NAME: &IdentStr = ident_str!("mint");
const BURN_FUNCTION_NAME: &IdentStr = ident_str!("burn");
const GET_BALANCE_FUNCTION_NAME: &IdentStr = ident_str!("get_balance");
const TRANSFER_FUNCTION_NAME: &IdentStr = ident_str!("transfer");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferArgs<'a> {
    pub from: &'a AccountAddress,
    pub to: &'a AccountAddress,
    pub amount: U256,
}

pub trait BaseTokenAccounts {
    fn charge_l1_cost<G: GasMeter>(
        &self,
        from: &AccountAddress,
        amount: u64,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
    ) -> Result<(), crate::Error>;

    fn transfer<G: GasMeter>(
        &self,
        args: TransferArgs<'_>,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
    ) -> Result<(), crate::Error>;
}

#[derive(Debug)]
pub struct MovedBaseTokenAccounts {
    eth_treasury: AccountAddress,
}

impl MovedBaseTokenAccounts {
    pub fn new(eth_treasury: AccountAddress) -> Self {
        Self { eth_treasury }
    }
}

impl BaseTokenAccounts for MovedBaseTokenAccounts {
    fn charge_l1_cost<G: GasMeter>(
        &self,
        from: &AccountAddress,
        amount: u64,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
    ) -> Result<(), crate::Error> {
        transfer_eth(
            TransferArgs {
                from,
                to: &self.eth_treasury,
                amount: U256::from(amount),
            },
            session,
            traversal_context,
            gas_meter,
        )
    }

    fn transfer<G: GasMeter>(
        &self,
        args: TransferArgs<'_>,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
    ) -> Result<(), crate::Error> {
        transfer_eth(args, session, traversal_context, gas_meter)
    }
}

pub fn mint_eth<G: GasMeter>(
    to: &AccountAddress,
    amount: U256,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> Result<(), crate::Error> {
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let to_arg = bcs::to_bytes(to).expect("address can serialize");
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(amount.to_move_u256())).expect("amount can serialize");

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

pub fn burn_eth<G: GasMeter>(
    from: &AccountAddress,
    amount: U256,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> Result<(), crate::Error> {
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let from_arg = bcs::to_bytes(from).expect("address can serialize");
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(amount.to_move_u256())).expect("amount can serialize");

    session.execute_entry_function(
        &token_module_id,
        BURN_FUNCTION_NAME,
        Vec::new(),
        vec![
            admin_arg.as_slice(),
            from_arg.as_slice(),
            amount_arg.as_slice(),
        ],
        gas_meter,
        traversal_context,
    )?;

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
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(args.amount.to_move_u256())).expect("amount can serialize");

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
) -> Result<U256, crate::Error> {
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
        MoveValue::U256(balance) => Ok(balance.to_u256()),
        _ => Err(crate::Error::eth_token_invariant_violation(
            EthToken::GetBalanceReturnsU256,
        )),
    }
}

/// Simplified API for getting the base token balance with no side effects.
/// Use it only for view methods as it does not use a VM session in the request pipeline.
pub fn quick_get_eth_balance(
    account: &AccountAddress,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
) -> U256 {
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

#[cfg(any(feature = "test-doubles", test))]
mod tests {
    use {super::*, crate::Error};

    impl BaseTokenAccounts for () {
        fn charge_l1_cost<G: GasMeter>(
            &self,
            _from: &AccountAddress,
            _amount: u64,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _gas_meter: &mut G,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn transfer<G: GasMeter>(
            &self,
            _args: TransferArgs<'_>,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _gas_meter: &mut G,
        ) -> Result<(), Error> {
            Ok(())
        }
    }
}
