use {
    super::{L2GasFee, L2GasFeeInput},
    crate::{
        block::HeaderForExecution,
        genesis::config::GenesisConfig,
        move_execution::{
            create_move_vm, create_vm_session,
            eth_token::{BaseTokenAccounts, TransferArgs},
            evm_native,
            execute::{deploy_module, execute_entry_function, execute_l2_contract, execute_script},
            gas::{new_gas_meter, total_gas_used},
            nonces::check_nonce,
            Logs,
        },
        primitives::{ToMoveAddress, ToSaturatedU64, B256},
        types::{
            session_id::SessionId,
            transactions::{
                NormalizedEthTransaction, ScriptOrModule, TransactionData,
                TransactionExecutionOutcome,
            },
        },
        Error::{InvalidTransaction, User},
        EthToken, InvalidTransactionCause, InvariantViolation,
    },
    aptos_gas_meter::{AptosGasMeter, StandardGasAlgebra, StandardGasMeter},
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        session::Session,
    },
};

#[allow(clippy::too_many_arguments)]
pub(super) fn verify_transaction(
    tx: &NormalizedEthTransaction,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut StandardGasMeter<StandardGasAlgebra>,
    genesis_config: &GenesisConfig,
    l1_cost: u64,
    l2_cost: u64,
    base_token: &impl BaseTokenAccounts,
) -> crate::Result<()> {
    if let Some(chain_id) = tx.chain_id {
        if chain_id != genesis_config.chain_id {
            return Err(InvalidTransactionCause::IncorrectChainId.into());
        }
    }

    let sender_move_address = tx.signer.to_move_address();

    // Charge gas for the transaction itself.
    // Immediately exit if there is not enough.
    let txn_size = (tx.data.len() as u64).into();
    let charge_gas = gas_meter
        .charge_intrinsic_gas_for_transaction(txn_size)
        .and_then(|_| gas_meter.charge_io_gas_for_transaction(txn_size));
    if charge_gas.is_err() {
        return Err(InvalidTransaction(
            InvalidTransactionCause::InsufficientIntrinsicGas,
        ));
    }

    base_token
        .charge_gas_cost(
            &sender_move_address,
            l1_cost,
            session,
            traversal_context,
            gas_meter,
        )
        .map_err(|_| InvalidTransaction(InvalidTransactionCause::FailedToPayL1Fee))?;

    base_token
        .charge_gas_cost(
            &sender_move_address,
            l2_cost,
            session,
            traversal_context,
            gas_meter,
        )
        .map_err(|_| InvalidTransaction(InvalidTransactionCause::FailedToPayL2Fee))?;

    check_nonce(
        tx.nonce,
        &sender_move_address,
        session,
        traversal_context,
        gas_meter,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_canonical_transaction(
    tx: &NormalizedEthTransaction,
    tx_hash: &B256,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
    l1_cost: u64,
    l2_fee: impl L2GasFee,
    l2_input: L2GasFeeInput,
    base_token: &impl BaseTokenAccounts,
    block_header: HeaderForExecution,
) -> crate::Result<TransactionExecutionOutcome> {
    let sender_move_address = tx.signer.to_move_address();

    let tx_data = TransactionData::parse_from(tx)?;

    let move_vm = create_move_vm()?;
    let session_id = SessionId::new_from_canonical(
        tx,
        tx_data.maybe_entry_fn(),
        tx_hash,
        genesis_config,
        block_header,
        tx_data.script_hash(),
    );
    let mut session = create_vm_session(&move_vm, state, session_id);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);

    let mut gas_meter = new_gas_meter(genesis_config, l2_input.gas_limit);
    let mut deployment = None;
    // Using l2 input here as test transactions don't set the max limit directly on itself
    let l2_cost = l2_fee.l2_fee(l2_input.clone()).saturating_to();
    let mut evm_logs = Vec::new();

    // TODO: use free gas meter for things that shouldn't fail due to
    // insufficient gas limit, impose a lower bound on the latter
    verify_transaction(
        tx,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
        genesis_config,
        l1_cost,
        l2_cost,
        base_token,
    )?;

    let vm_outcome = match tx_data {
        TransactionData::EntryFunction(entry_fn) => execute_entry_function(
            entry_fn,
            &sender_move_address,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
        ),
        TransactionData::ScriptOrModule(ScriptOrModule::Script(script)) => execute_script(
            script,
            &sender_move_address,
            &mut session,
            &mut traversal_context,
            &mut gas_meter,
        ),
        TransactionData::ScriptOrModule(ScriptOrModule::Module(module)) => {
            let module_id =
                deploy_module(module, sender_move_address, &mut session, &mut gas_meter);
            module_id.map(|id| {
                deployment = Some((sender_move_address, id));
            })
        }
        TransactionData::EoaBaseTokenTransfer(to) => {
            let to = to.to_move_address();
            let amount = tx.value;
            let args = TransferArgs {
                to: &to,
                from: &sender_move_address,
                amount,
            };

            base_token.transfer(args, &mut session, &mut traversal_context, &mut gas_meter)
        }
        TransactionData::L2Contract(contract) => {
            evm_logs = execute_l2_contract(
                &sender_move_address,
                &contract.to_move_address(),
                tx.value,
                tx.data.to_vec(),
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
            )?;
            Ok(())
        }
    };

    let gas_used = total_gas_used(&gas_meter, genesis_config);
    let used_l2_input = L2GasFeeInput::new(gas_used, l2_input.effective_gas_price);
    let used_l2_cost = l2_fee.l2_fee(used_l2_input).to_saturated_u64();

    // Refunds should not be metered as they're supposed to always succeed
    base_token
        .refund_gas_cost(
            &sender_move_address,
            l2_cost.saturating_sub(used_l2_cost),
            &mut session,
            &mut traversal_context,
        )
        .map_err(|_| {
            crate::Error::InvariantViolation(InvariantViolation::EthToken(
                EthToken::RefundAlwaysSucceeds,
            ))
        })?;

    let (mut changes, mut extensions) = session.finish_with_extensions()?;
    let mut logs = extensions.logs();
    logs.extend(evm_logs);
    let evm_changes = evm_native::extract_evm_changes(&extensions);
    changes
        .squash(evm_changes)
        .expect("EVM changes must merge with other session changes");

    match vm_outcome {
        Ok(_) => Ok(TransactionExecutionOutcome::new(
            Ok(()),
            changes,
            gas_used,
            l2_input.effective_gas_price,
            logs,
            deployment,
        )),
        // User error still generates a receipt and consumes gas
        Err(User(e)) => Ok(TransactionExecutionOutcome::new(
            Err(e),
            changes,
            gas_used,
            l2_input.effective_gas_price,
            logs,
            None,
        )),
        Err(e) => Err(e),
    }
}
