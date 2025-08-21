use {
    super::{L2GasFee, L2GasFeeInput},
    crate::{
        CanonicalExecutionInput, Logs, create_vm_session,
        eth_token::{self, BaseTokenAccounts, TransferArgs},
        execute::{
            EvmExecutionArgs, deploy_evm_contract, deploy_module, execute_entry_function,
            execute_evm_contract, execute_script,
        },
        gas::{new_gas_meter, total_gas_used},
        nonces::{check_nonce, nonce_epilogue},
        resolver_cache::{CachedResolver, ResolverCache},
        session_id::SessionId,
        transaction::{
            Changes, NormalizedEthTransaction, ScriptOrDeployment, TransactionData,
            TransactionExecutionOutcome,
        },
    },
    alloy::primitives::U256,
    aptos_framework::natives::event::NativeEventContext,
    aptos_gas_algebra::{FeePerGasUnit, GasQuantity, NumBytes, NumSlots, Octa},
    aptos_gas_meter::{AptosGasMeter, StandardGasAlgebra, StandardGasMeter},
    aptos_table_natives::{TableChangeSet, TableResolver},
    aptos_types::{
        contract_event::ContractEvent, state_store::state_key::StateKey, write_set::WriteOpSize,
    },
    move_core_types::{
        effects::{ChangeSet, Op},
        language_storage::ModuleId,
        value::MoveTypeLayout,
    },
    move_vm_runtime::{
        AsUnsyncCodeStorage, ModuleStorage,
        module_traversal::{TraversalContext, TraversalStorage},
        session::Session,
    },
    move_vm_types::{gas::UnmeteredGasMeter, resolver::MoveResolver},
    umi_evm_ext::{
        events::EthTransfersLogger,
        state::{BlockHashLookup, StorageTrieRepository},
    },
    umi_genesis::{CreateMoveVm, UmiVm, config::GenesisConfig},
    umi_shared::{
        error::{
            Error::{InvalidTransaction, User},
            EthToken, InvalidTransactionCause,
        },
        primitives::ToMoveAddress,
        resolver_utils::{ChangesBasedResolver, PairedResolvers},
    },
    umi_state::ResolverBasedModuleBytesStorage,
};

pub struct CanonicalVerificationInput<'input, 'a, 'r, 'l, B, MS> {
    pub tx: &'input NormalizedEthTransaction,
    pub session: &'input mut Session<'r, 'l>,
    pub traversal_context: &'input mut TraversalContext<'a>,
    pub gas_meter: &'input mut StandardGasMeter<StandardGasAlgebra>,
    pub genesis_config: &'input GenesisConfig,
    pub l1_cost: U256,
    pub l2_cost: U256,
    pub base_token: &'input B,
    pub module_storage: &'input MS,
}

pub(super) fn verify_transaction<B: BaseTokenAccounts, MS: ModuleStorage>(
    input: CanonicalVerificationInput<B, MS>,
) -> umi_shared::error::Result<()> {
    if let Some(chain_id) = input.tx.chain_id {
        if chain_id != input.genesis_config.chain_id {
            return Err(InvalidTransactionCause::IncorrectChainId.into());
        }
    }

    let sender_move_address = input.tx.signer.to_move_address();

    // Charge gas for the transaction itself.
    // Immediately exit if there is not enough.
    let txn_size = (input.tx.data.len() as u64).into();
    let charge_gas = input
        .gas_meter
        .charge_intrinsic_gas_for_transaction(txn_size)
        .and_then(|_| input.gas_meter.charge_io_gas_for_transaction(txn_size));
    if charge_gas.is_err() {
        return Err(InvalidTransaction(
            InvalidTransactionCause::InsufficientIntrinsicGas,
        ));
    }

    // We use the no-op gas meter for the fee-charging operations because
    // the gas they would consume was already paid in the intrinsic gas above.
    let mut noop_meter = UnmeteredGasMeter;

    input
        .base_token
        .charge_gas_cost(
            &sender_move_address,
            input.l1_cost,
            input.session,
            input.traversal_context,
            &mut noop_meter,
            input.module_storage,
        )
        .map_err(|_| InvalidTransaction(InvalidTransactionCause::FailedToPayL1Fee))?;

    input
        .base_token
        .charge_gas_cost(
            &sender_move_address,
            input.l2_cost,
            input.session,
            input.traversal_context,
            &mut noop_meter,
            input.module_storage,
        )
        .map_err(|_| InvalidTransaction(InvalidTransactionCause::FailedToPayL2Fee))?;

    check_nonce(
        input.tx.nonce,
        &sender_move_address,
        input.session,
        input.traversal_context,
        &mut noop_meter,
        input.module_storage,
    )?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip(input, resolver_cache))]
pub(super) fn execute_canonical_transaction<
    S: MoveResolver + TableResolver,
    ST: StorageTrieRepository,
    F: L2GasFee,
    B: BaseTokenAccounts,
    H: BlockHashLookup,
>(
    input: CanonicalExecutionInput<S, ST, F, B, H>,
    resolver_cache: &mut ResolverCache,
) -> umi_shared::error::Result<TransactionExecutionOutcome> {
    resolver_cache.clear();
    let cached_resolver = CachedResolver::new(input.state, resolver_cache);
    let sender_move_address = input.tx.signer.to_move_address();

    let tx_data = TransactionData::parse_from(input.tx)?;

    let gas_unit_price = input
        .l2_input
        .effective_gas_price
        .try_into()
        .map(FeePerGasUnit::new)
        .map_err(|_| {
            InvalidTransactionCause::InvalidGasPrice(input.l2_input.effective_gas_price)
        })?;
    let umi_vm = UmiVm::new(input.genesis_config);
    let module_bytes_storage = ResolverBasedModuleBytesStorage::new(&cached_resolver);
    let code_storage = module_bytes_storage.as_unsync_code_storage(&umi_vm);
    let vm = umi_vm.create_move_vm()?;
    let session_id = SessionId::new_from_canonical(
        input.tx,
        tx_data.maybe_entry_fn(),
        input.tx_hash,
        input.genesis_config,
        input.block_header,
        tx_data.script_hash(),
    )?;
    let eth_transfers_logger = EthTransfersLogger::default();
    let mut session = create_vm_session(
        &vm,
        &cached_resolver,
        session_id,
        input.storage_trie,
        &eth_transfers_logger,
        input.block_hash_lookup,
    );
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);

    let mut gas_meter = new_gas_meter(input.genesis_config, input.l2_input.gas_limit);
    let mut deployment = None;
    let mut deploy_changes = ChangeSet::new();
    // Using l2 input here as test transactions don't set the max limit directly on itself
    let l2_cost = input.l2_fee.l2_fee(input.l2_input.clone()).saturating_to();

    verify_transaction(CanonicalVerificationInput {
        tx: input.tx,
        session: &mut session,
        traversal_context: &mut traversal_context,
        gas_meter: &mut gas_meter,
        genesis_config: input.genesis_config,
        l1_cost: input.l1_cost,
        l2_cost,
        base_token: input.base_token,
        module_storage: &code_storage,
    })?;

    let vm_outcome = match tx_data {
        TransactionData::EntryFunction(entry_fn) => execute_entry_function(
            entry_fn,
            &sender_move_address,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        ),
        TransactionData::ScriptOrDeployment(ScriptOrDeployment::Script(script)) => execute_script(
            script,
            &sender_move_address,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        ),
        TransactionData::ScriptOrDeployment(ScriptOrDeployment::ModuleBundle(bundle)) => {
            let bytes_len: u64 = bundle.iter().map(|m| m.code().len() as u64).sum();
            let charge_gas = crate::gas::charge_new_module_processing(
                &mut gas_meter,
                input.genesis_config,
                &sender_move_address,
                bytes_len,
            );
            let writes =
                charge_gas.and_then(|_| deploy_module(bundle, sender_move_address, &code_storage));
            writes.map(|writes| {
                deployment = Some(input.tx.signer);
                deploy_changes
                    .squash(writes)
                    .expect("Move module deployment changes should be compatible");
            })
        }
        TransactionData::ScriptOrDeployment(ScriptOrDeployment::EvmContract(bytecode)) => {
            let address = deploy_evm_contract(
                bytecode,
                input.tx.value,
                sender_move_address,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
                &code_storage,
            );
            address.map(|a| deployment = Some(a))
        }
        TransactionData::EoaBaseTokenTransfer(to) => {
            let to = to.to_move_address();
            let amount = input.tx.value;
            let args = TransferArgs {
                to: &to,
                from: &sender_move_address,
                amount,
            };

            input.base_token.transfer(
                args,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
                &code_storage,
            )
        }
        TransactionData::L2Contract(contract) => execute_evm_contract(
            EvmExecutionArgs::new(
                sender_move_address,
                contract.to_move_address(),
                input.tx.value,
                input.tx.data.to_vec(),
            ),
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        )
        .map(|_| ()),
        TransactionData::EvmContract { address, data } => execute_evm_contract(
            EvmExecutionArgs::new(
                sender_move_address,
                address.to_move_address(),
                input.tx.value,
                data,
            ),
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        )
        .map(|_| ()),
    };

    let vm_outcome = vm_outcome.and_then(|_| {
        // Ensure any base token balance changes in EVM are reflected in Move too
        eth_token::replicate_transfers(
            &eth_transfers_logger,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        )
    });

    let (mut user_changes, mut extensions) = session.finish_with_extensions(&code_storage)?;
    let evm_nonces = umi_evm_ext::extract_evm_nonces(&extensions);
    let evm_changes = umi_evm_ext::extract_evm_changes(&extensions)?;
    let table_changes = crate::table_changes::extract_table_changes(
        &mut extensions,
        code_storage.module_storage(),
    )?;
    let user_events = extensions.remove::<NativeEventContext>().into_events();
    extensions.add(NativeEventContext::default());
    user_changes
        .squash(evm_changes.accounts)
        .expect("EVM changes must merge with other session changes");
    user_changes
        .squash(deploy_changes)
        .expect("Module deploy changes must merge with other session changes");

    let vm_outcome = vm_outcome.and_then(|_| {
        charge_io_gas(
            &cached_resolver.borrow_cache(),
            &user_changes,
            &table_changes,
            &user_events,
            &mut gas_meter,
            input.genesis_config,
            gas_unit_price,
        )
    });

    let (changes, logs, gas_used) = {
        let changes_resolver = ChangesBasedResolver::new(&user_changes);
        let refund_resolver = PairedResolvers::new(&changes_resolver, &cached_resolver);
        let mut refund_session = vm.new_session_with_extensions(&refund_resolver, extensions);

        let gas_used = total_gas_used(&gas_meter, input.genesis_config);
        let used_l2_input = L2GasFeeInput::new(gas_used, input.l2_input.effective_gas_price);
        let used_l2_cost = input.l2_fee.l2_fee(used_l2_input);

        // Refunds should not be metered as they're supposed to always succeed
        input
            .base_token
            .refund_gas_cost(
                &sender_move_address,
                l2_cost.saturating_sub(used_l2_cost),
                &mut refund_session,
                &mut traversal_context,
                &code_storage,
            )
            .map_err(|_| {
                umi_shared::error::Error::eth_token_invariant_violation(
                    EthToken::RefundAlwaysSucceeds,
                )
            })?;

        nonce_epilogue(
            &sender_move_address,
            evm_nonces,
            &mut refund_session,
            &mut traversal_context,
            &code_storage,
        )?;

        let (changes, mut extensions) = refund_session.finish_with_extensions(&code_storage)?;
        let refund_events = extensions.remove::<NativeEventContext>().into_events();
        let logs = user_events.into_iter().chain(refund_events).logs();
        (changes, logs, gas_used)
    };

    user_changes
        .squash(changes)
        .expect("User changes must merge with refund session changes");
    let changes = Changes::new(
        umi_state::Changes::new(user_changes, table_changes),
        evm_changes.storage,
    );

    match vm_outcome {
        Ok(_) => Ok(TransactionExecutionOutcome::new(
            Ok(()),
            changes,
            gas_used,
            input.l2_input.effective_gas_price,
            logs,
            deployment,
        )),
        // User error still generates a receipt and consumes gas
        Err(User(e)) => Ok(TransactionExecutionOutcome::new(
            Err(e),
            changes,
            gas_used,
            input.l2_input.effective_gas_price,
            logs,
            None,
        )),
        Err(e) => Err(e),
    }
}

fn charge_io_gas(
    resolver_cache: &ResolverCache,
    changes: &ChangeSet,
    table_changes: &TableChangeSet,
    user_events: &[(ContractEvent, Option<MoveTypeLayout>)],
    gas_meter: &mut StandardGasMeter<StandardGasAlgebra>,
    genesis_config: &GenesisConfig,
    gas_unit_price: FeePerGasUnit,
) -> umi_shared::error::Result<()> {
    // If gas is free then we can't charge storage because the parameters for
    // storage costs are denominated in base token units and converted to gas via the price.
    // Putting the early return here also skips the io charges which do not depend
    // on the gas price, but this is fine because if gas is free then computing the accurate
    // amount of gas used is not very important (we get zero tokens regardless).
    if gas_unit_price.is_zero() {
        return Ok(());
    }
    let invariant_violation = |_| umi_shared::error::Error::state_key_invariant_violation();

    let mut storage_fee: GasQuantity<Octa> = GasQuantity::new(0);
    for (address, struct_tag, op) in changes.resources() {
        let op_size = to_op_size(op);
        let key = StateKey::resource(&address, struct_tag).map_err(invariant_violation)?;
        gas_meter.charge_io_gas_for_write(&key, &op_size)?;

        storage_fee += charge_storage_gas(
            op_size,
            || resolver_cache.resource_original_size(&address, struct_tag) as u64,
            genesis_config,
        );
    }

    for (address, id, op) in changes.modules() {
        let op_size = to_op_size(op);
        let key = StateKey::module(address, id);
        gas_meter.charge_io_gas_for_write(&key, &op_size)?;

        let module_id = ModuleId::new(*address, id.clone());
        storage_fee += charge_storage_gas(
            op_size,
            || resolver_cache.module_original_size(&module_id) as u64,
            genesis_config,
        );
    }

    for (handle, changes) in table_changes.changes.iter() {
        for (id, change) in changes.entries.iter() {
            let op_size = to_op_size(change.as_ref().map(|(bytes, _)| bytes));
            let key = StateKey::table_item(&handle.into(), id);
            gas_meter.charge_io_gas_for_write(&key, &op_size)?;

            storage_fee += charge_storage_gas(
                op_size,
                || resolver_cache.table_entry_original_size(handle, id) as u64,
                genesis_config,
            );
        }
    }

    gas_meter.charge_storage_fee(storage_fee, gas_unit_price)?;

    for (event, _) in user_events {
        gas_meter.charge_io_gas_for_event(event)?;
    }

    Ok(())
}

fn charge_storage_gas<F: FnOnce() -> u64>(
    op: WriteOpSize,
    get_original_size: F,
    genesis_config: &GenesisConfig,
) -> GasQuantity<Octa> {
    let (slots, bytes) = match op {
        WriteOpSize::Creation { write_len } => (NumSlots::new(1), NumBytes::new(write_len)),
        WriteOpSize::Modification { write_len } => {
            let original_size = get_original_size();
            if original_size >= write_len {
                return GasQuantity::new(0);
            }
            (
                NumSlots::new(0),
                NumBytes::new(write_len.saturating_sub(original_size)),
            )
        }
        WriteOpSize::Deletion => {
            // No gas charge for deletion.
            return GasQuantity::new(0);
        }
    };
    genesis_config.gas_costs.vm.txn.storage_fee_per_state_slot * slots
        + genesis_config.gas_costs.vm.txn.storage_fee_per_state_byte * bytes
}

fn to_op_size<T: AsRef<[u8]>>(op: Op<&T>) -> WriteOpSize {
    match op {
        Op::New(bytes) => WriteOpSize::Creation {
            write_len: bytes.as_ref().len() as u64,
        },
        Op::Modify(bytes) => WriteOpSize::Modification {
            write_len: bytes.as_ref().len() as u64,
        },
        Op::Delete => WriteOpSize::Deletion,
    }
}
