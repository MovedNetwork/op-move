use {
    super::{L2GasFee, L2GasFeeInput},
    crate::{
        CanonicalExecutionInput, Logs, create_vm_session,
        eth_token::{self, BaseTokenAccounts, TransferArgs},
        execute::{
            deploy_evm_contract, deploy_module, execute_entry_function, execute_evm_contract,
            execute_script,
        },
        gas::{new_gas_meter, total_gas_used},
        nonces::check_nonce,
        session_id::SessionId,
        transaction::{
            Changes, NormalizedEthTransaction, ScriptOrDeployment, TransactionData,
            TransactionExecutionOutcome,
        },
    },
    alloy::primitives::U256,
    aptos_gas_algebra::FeePerGasUnit,
    aptos_gas_meter::{AptosGasMeter, StandardGasAlgebra, StandardGasMeter},
    aptos_table_natives::TableResolver,
    aptos_types::{state_store::state_key::StateKey, write_set::WriteOpSize},
    move_core_types::{
        effects::{ChangeSet, Op},
        language_storage::ModuleId,
    },
    move_vm_runtime::{
        AsUnsyncCodeStorage, ModuleStorage,
        module_traversal::{TraversalContext, TraversalStorage},
        native_extensions::NativeContextExtensions,
        session::Session,
    },
    move_vm_types::{gas::UnmeteredGasMeter, resolver::MoveResolver},
    umi_evm_ext::{
        EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE,
        events::EthTransfersLogger,
        state::{BlockHashLookup, StorageTrieRepository},
    },
    umi_genesis::{CreateMoveVm, UmiVm, config::GenesisConfig},
    umi_shared::{
        error::{
            Error::{InvalidTransaction, User},
            EthToken, InvalidTransactionCause, InvariantViolation,
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

pub(super) fn execute_canonical_transaction<
    S: MoveResolver + TableResolver,
    ST: StorageTrieRepository,
    F: L2GasFee,
    B: BaseTokenAccounts,
    H: BlockHashLookup,
>(
    input: CanonicalExecutionInput<S, ST, F, B, H>,
) -> umi_shared::error::Result<TransactionExecutionOutcome> {
    let sender_move_address = input.tx.signer.to_move_address();

    let tx_data = TransactionData::parse_from(input.tx)?;

    let umi_vm = UmiVm::new(input.genesis_config);
    let module_bytes_storage: ResolverBasedModuleBytesStorage<'_, S> =
        ResolverBasedModuleBytesStorage::new(input.state);
    let code_storage = module_bytes_storage.as_unsync_code_storage(&umi_vm);
    let vm = umi_vm.create_move_vm()?;
    let session_id = SessionId::new_from_canonical(
        input.tx,
        tx_data.maybe_entry_fn(),
        input.tx_hash,
        input.genesis_config,
        input.block_header,
        tx_data.script_hash(),
    );
    let eth_transfers_logger = EthTransfersLogger::default();
    let mut session = create_vm_session(
        &vm,
        input.state,
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
        TransactionData::ScriptOrDeployment(ScriptOrDeployment::Module(module)) => {
            let charge_gas = crate::gas::charge_new_module_processing(
                &mut gas_meter,
                input.genesis_config,
                &sender_move_address,
                module.code().len() as u64,
            );
            let module_id =
                charge_gas.and_then(|_| deploy_module(module, sender_move_address, &code_storage));
            module_id.map(|(id, writes)| {
                deployment = Some((sender_move_address, id));
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
            address.map(|a| {
                deployment = Some((
                    a.to_move_address(),
                    ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into()),
                ))
            })
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
            &sender_move_address,
            &contract.to_move_address(),
            input.tx.value,
            input.tx.data.to_vec(),
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
            &code_storage,
        )
        .map(|_| ()),
        TransactionData::EvmContract { address, data } => execute_evm_contract(
            &sender_move_address,
            &address.to_move_address(),
            input.tx.value,
            data,
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

    let (mut user_changes, extensions) = session.finish_with_extensions(&code_storage)?;
    let evm_changes = umi_evm_ext::extract_evm_changes(&extensions);
    user_changes
        .squash(evm_changes.accounts)
        .expect("EVM changes must merge with other session changes");
    user_changes
        .squash(deploy_changes)
        .expect("Module deploy changes must merge with other session changes");

    let gas_unit_price = FeePerGasUnit::new(wei_to_octa(input.l2_input.effective_gas_price));
    let vm_outcome = vm_outcome.and_then(|_| {
        charge_io_gas(
            &user_changes,
            &extensions,
            &mut gas_meter,
            input.genesis_config,
            gas_unit_price,
        )
    });

    let changes_resolver = ChangesBasedResolver::new(&user_changes);
    let refund_resolver = PairedResolvers::new(&changes_resolver, input.state);
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
            umi_shared::error::Error::InvariantViolation(InvariantViolation::EthToken(
                EthToken::RefundAlwaysSucceeds,
            ))
        })?;

    let (changes, mut extensions) = refund_session.finish_with_extensions(&code_storage)?;
    let logs = extensions.logs();

    // Drop a bunch of borrows we no longer need to allow `user_changes` to be used again.
    drop(extensions);
    drop(refund_resolver);
    drop(changes_resolver);

    user_changes
        .squash(changes)
        .expect("User changes must merge with refund session changes");
    let changes = Changes::new(user_changes.into(), evm_changes.storage);

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
    changes: &ChangeSet,
    extensions: &NativeContextExtensions,
    gas_meter: &mut StandardGasMeter<StandardGasAlgebra>,
    genesis_config: &GenesisConfig,
    gas_unit_price: FeePerGasUnit,
) -> umi_shared::error::Result<()> {
    let invariant_violation =
        |_| umi_shared::error::Error::InvariantViolation(InvariantViolation::StateKey);

    for (address, struct_tag, op) in changes.resources() {
        let op_size = to_op_size(op);
        let key = StateKey::resource(&address, struct_tag).map_err(invariant_violation)?;
        gas_meter.charge_io_gas_for_write(&key, &op_size)?;

        // TODO: storage gas for ops. Need to charge based on change in size in case of modified.
        // Aptos does this with the StateValueMetadata which we are currently not using.
    }

    for (address, id, op) in changes.modules() {
        let op_size = to_op_size(op);
        let key = StateKey::module(address, id);
        gas_meter.charge_io_gas_for_write(&key, &op_size)?;
    }

    // TODO: io gas for events

    Ok(())
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

// Ethereum's smallest base token unit is Wei (1 Wei = 10^{-18} ETH),
// but Aptos' smallest base token unit is Octa (1 Octa = 10^{-8} APT).
// This function scales down Wei to Octa for use in Aptos code.
fn wei_to_octa(x: U256) -> u64 {
    const FACTOR: U256 = U256::from_limbs([10_000_000_000, 0, 0, 0]);
    x.wrapping_div(FACTOR).saturating_to()
}
