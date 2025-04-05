use {
    crate::session_id::SessionId,
    alloy::primitives::U256,
    aptos_table_natives::TableResolver,
    move_core_types::{
        account_address::AccountAddress, ident_str, identifier::IdentStr,
        language_storage::ModuleId, value::MoveValue,
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
    moved_evm_ext::{EVM_NATIVE_ADDRESS, events::EthTransferLog, state::StorageTrieRepository},
    moved_genesis::{CreateMoveVm, FRAMEWORK_ADDRESS, MovedVm},
    moved_shared::{
        error::EthToken,
        primitives::{ToMoveU256, ToU256},
    },
    moved_state::ResolverBasedModuleBytesStorage,
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
    fn charge_gas_cost<G: GasMeter, MS: ModuleStorage>(
        &self,
        from: &AccountAddress,
        amount: u64,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), moved_shared::error::Error>;

    fn refund_gas_cost(
        &self,
        to: &AccountAddress,
        amount: u64,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        module_storage: &impl ModuleStorage,
    ) -> Result<(), moved_shared::error::Error>;

    fn transfer<G: GasMeter, MS: ModuleStorage>(
        &self,
        args: TransferArgs<'_>,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), moved_shared::error::Error>;
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
    fn charge_gas_cost<G: GasMeter, MS: ModuleStorage>(
        &self,
        from: &AccountAddress,
        amount: u64,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), moved_shared::error::Error> {
        transfer_eth(
            TransferArgs {
                from,
                to: &self.eth_treasury,
                amount: U256::from(amount),
            },
            session,
            traversal_context,
            gas_meter,
            module_storage,
        )
    }

    fn refund_gas_cost(
        &self,
        to: &AccountAddress,
        amount: u64,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        module_storage: &impl ModuleStorage,
    ) -> Result<(), moved_shared::error::Error> {
        let mut gas_meter = UnmeteredGasMeter;
        transfer_eth(
            TransferArgs {
                from: &self.eth_treasury,
                to,
                amount: U256::from(amount),
            },
            session,
            traversal_context,
            &mut gas_meter,
            module_storage,
        )
    }

    fn transfer<G: GasMeter, MS: ModuleStorage>(
        &self,
        args: TransferArgs<'_>,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), moved_shared::error::Error> {
        transfer_eth(args, session, traversal_context, gas_meter, module_storage)
    }
}

pub fn mint_eth<G: GasMeter>(
    to: &AccountAddress,
    amount: U256,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<(), moved_shared::error::Error> {
    if amount.is_zero() {
        return Ok(());
    }
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let to_arg = bcs::to_bytes(to).expect("address can serialize");
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(amount.to_move_u256())).expect("amount can serialize");

    let function =
        session.load_function(module_storage, &token_module_id, MINT_FUNCTION_NAME, &[])?;

    session
        .execute_entry_function(
            function,
            vec![
                admin_arg.as_slice(),
                to_arg.as_slice(),
                amount_arg.as_slice(),
            ],
            gas_meter,
            traversal_context,
            module_storage,
        )
        .map_err(|e| {
            println!("{e:?}");
            moved_shared::error::Error::eth_token_invariant_violation(EthToken::MintAlwaysSucceeds)
        })?;

    Ok(())
}

pub fn burn_eth<G: GasMeter>(
    from: &AccountAddress,
    amount: U256,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<(), moved_shared::error::Error> {
    if amount.is_zero() {
        return Ok(());
    }
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let from_arg = bcs::to_bytes(from).expect("address can serialize");
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(amount.to_move_u256())).expect("amount can serialize");

    let function =
        session.load_function(module_storage, &token_module_id, BURN_FUNCTION_NAME, &[])?;

    session.execute_entry_function(
        function,
        vec![
            admin_arg.as_slice(),
            from_arg.as_slice(),
            amount_arg.as_slice(),
        ],
        gas_meter,
        traversal_context,
        module_storage,
    )?;

    Ok(())
}

pub fn transfer_eth<G: GasMeter>(
    args: TransferArgs<'_>,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<(), moved_shared::error::Error> {
    if args.amount.is_zero() {
        return Ok(());
    }
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let from_arg = bcs::to_bytes(args.from).expect("from address can serialize");
    let to_arg = bcs::to_bytes(args.to).expect("to address can serialize");
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(args.amount.to_move_u256())).expect("amount can serialize");

    let function = session.load_function(
        module_storage,
        &token_module_id,
        TRANSFER_FUNCTION_NAME,
        &[],
    )?;

    // FIXME: transfer function can fail if user has insufficient balance or if the gas meter
    // is depleted, which is a potential attack vector
    session.execute_entry_function(
        function,
        vec![
            admin_arg.as_slice(),
            from_arg.as_slice(),
            to_arg.as_slice(),
            amount_arg.as_slice(),
        ],
        gas_meter,
        traversal_context,
        module_storage,
    )?;

    Ok(())
}

pub fn replicate_transfers<G: GasMeter, L: EthTransferLog>(
    eth_transfer_logger: &L,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<(), moved_shared::error::Error> {
    // Transfer the transaction value from EVM native account to `origin`.
    // This step is needed because all EVM transactions start with the caller
    // transferring tokens to the EVM native account as part of `evm_call`.
    // We transfer them back to then follow the sequence of transfers that
    // happened in the EVM.
    // Note: in the case of deposit transactions the new based tokens are
    // minted to the EVM native account. So this logic is still needed in
    // that case. The general invariant is that all base tokens used during
    // EVM execution are held by the EVM native account within the MoveVM.
    for (origin, value) in eth_transfer_logger.take_origins() {
        if !value.is_zero() {
            transfer_eth(
                TransferArgs {
                    from: &EVM_NATIVE_ADDRESS,
                    to: &origin,
                    amount: value,
                },
                session,
                traversal_context,
                gas_meter,
                module_storage,
            )?;
        }
    }

    for transfer in eth_transfer_logger.take_transfers() {
        transfer_eth(
            TransferArgs {
                from: &transfer.from,
                to: &transfer.to,
                amount: transfer.amount,
            },
            session,
            traversal_context,
            gas_meter,
            module_storage,
        )?;
    }

    Ok(())
}

pub fn get_eth_balance<G: GasMeter>(
    account: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<U256, moved_shared::error::Error> {
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
            module_storage,
        )
        .map_err(|e| {
            println!("{e:?}");

            moved_shared::error::Error::eth_token_invariant_violation(
                EthToken::GetBalanceAlwaysSucceeds,
            )
        })?
        .return_values;

    let (raw_output, layout) =
        return_values
            .first()
            .ok_or(moved_shared::error::Error::eth_token_invariant_violation(
                EthToken::GetBalanceReturnsAValue,
            ))?;

    let value = ValueSerDeContext::new()
        .deserialize(raw_output, layout)
        .ok_or(moved_shared::error::Error::eth_token_invariant_violation(
            EthToken::GetBalanceReturnDeserializes,
        ))?
        .as_move_value(layout);

    match value {
        MoveValue::U256(balance) => Ok(balance.to_u256()),
        _ => Err(moved_shared::error::Error::eth_token_invariant_violation(
            EthToken::GetBalanceReturnsU256,
        )),
    }
}

/// Simplified API for getting the base token balance with no side effects.
/// Use it only for view methods as it does not use a VM session in the request pipeline.
pub fn quick_get_eth_balance(
    account: &AccountAddress,
    state: &(impl MoveResolver + TableResolver),
    storage_trie: &impl StorageTrieRepository,
) -> U256 {
    let moved_vm = MovedVm::default();
    let vm = moved_vm.create_move_vm().unwrap();
    let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state);
    let code_storage = module_bytes_storage.as_unsync_code_storage(&moved_vm);
    let mut session = super::create_vm_session(&vm, state, SessionId::default(), storage_trie, &());
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = UnmeteredGasMeter;
    get_eth_balance(
        account,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
        &code_storage,
    )
    .unwrap()
}

#[cfg(any(feature = "test-doubles", test))]
mod tests {
    use {super::*, moved_shared::error::Error};

    impl BaseTokenAccounts for () {
        fn charge_gas_cost<G: GasMeter, MS: ModuleStorage>(
            &self,
            _from: &AccountAddress,
            _amount: u64,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _gas_meter: &mut G,
            _module_storage: &MS,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn transfer<G: GasMeter, MS: ModuleStorage>(
            &self,
            _args: TransferArgs<'_>,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _gas_meter: &mut G,
            _module_storage: &MS,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn refund_gas_cost(
            &self,
            _to: &AccountAddress,
            _amount: u64,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _module_storage: &impl ModuleStorage,
        ) -> Result<(), Error> {
            Ok(())
        }
    }
}
