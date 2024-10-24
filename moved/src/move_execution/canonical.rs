use {
    crate::{
        genesis::config::GenesisConfig,
        move_execution::{
            create_move_vm, create_vm_session,
            eth_token::{BaseTokenAccounts, TransferArgs},
            execute::{deploy_module, execute_entry_function, execute_script},
            gas::{new_gas_meter, total_gas_used},
            nonces::check_nonce,
            Logs,
        },
        primitives::{ToMoveAddress, B256},
        types::{
            session_id::SessionId,
            transactions::{
                NormalizedEthTransaction, ScriptOrModule, TransactionData,
                TransactionExecutionOutcome,
            },
        },
        Error::{InvalidTransaction, User},
        InvalidTransactionCause,
    },
    aptos_gas_meter::AptosGasMeter,
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
};

pub(super) fn execute_canonical_transaction(
    tx: &NormalizedEthTransaction,
    tx_hash: &B256,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
    l1_cost: u64,
    base_token: &impl BaseTokenAccounts,
) -> crate::Result<TransactionExecutionOutcome> {
    if let Some(chain_id) = tx.chain_id {
        if chain_id != genesis_config.chain_id {
            return Err(InvalidTransactionCause::IncorrectChainId.into());
        }
    }

    let sender_move_address = tx.signer.to_move_address();

    let tx_data = TransactionData::parse_from(tx)?;

    let move_vm = create_move_vm()?;
    let session_id =
        SessionId::new_from_canonical(tx, tx_data.maybe_entry_fn(), tx_hash, genesis_config);
    let mut session = create_vm_session(&move_vm, state, session_id);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = new_gas_meter(genesis_config, tx.gas_limit());

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

    base_token.charge_l1_cost(
        &sender_move_address,
        l1_cost,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
    )?;

    check_nonce(
        tx.nonce,
        &sender_move_address,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
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
            deploy_module(module, sender_move_address, &mut session, &mut gas_meter)
        }
        TransactionData::EoaBaseTokenTransfer(to) => {
            let to = to.to_move_address();
            // TODO: handle U256 properly
            let amount = tx.value.as_limbs()[0];
            let args = TransferArgs {
                to: &to,
                from: &sender_move_address,
                amount,
            };

            base_token.transfer(args, &mut session, &mut traversal_context, &mut gas_meter)
        }
    };

    let (changes, mut extensions) = session.finish_with_extensions()?;
    let logs = extensions.logs().collect();
    let gas_used = total_gas_used(&gas_meter, genesis_config);

    match vm_outcome {
        Ok(_) => Ok(TransactionExecutionOutcome::new(
            Ok(()),
            changes,
            gas_used,
            logs,
        )),
        // User error still generates a receipt and consumes gas
        Err(User(e)) => Ok(TransactionExecutionOutcome::new(
            Err(e),
            changes,
            gas_used,
            logs,
        )),
        Err(e) => Err(e),
    }
}
