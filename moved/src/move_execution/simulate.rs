use {
    crate::{
        block::HeaderForExecution,
        genesis::config::GenesisConfig,
        move_execution::{
            canonical::verify_transaction, create_move_vm, create_vm_session, execute_transaction,
            gas::new_gas_meter, quick_get_nonce, BaseTokenAccounts,
        },
        primitives::{ToMoveAddress, B256},
        types::{
            session_id::SessionId,
            transactions::{
                NormalizedEthTransaction, NormalizedExtendedTxEnvelope, ScriptOrModule,
                TransactionData, TransactionExecutionOutcome,
            },
        },
        Error::InvalidTransaction,
        InvalidTransactionCause,
    },
    alloy::rpc::types::TransactionRequest,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_table_extension::TableResolver,
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
    std::time::{SystemTime, UNIX_EPOCH},
};

pub fn simulate_transaction(
    request: TransactionRequest,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
    base_token: &impl BaseTokenAccounts,
    block_height: u64,
) -> crate::Result<TransactionExecutionOutcome> {
    let mut tx = NormalizedEthTransaction::from(request.clone());
    if request.from.is_some() && request.nonce.is_none() {
        tx.nonce = quick_get_nonce(&tx.signer.to_move_address(), state);
    }
    let tx = NormalizedExtendedTxEnvelope::Canonical(tx);

    let block_header = HeaderForExecution {
        number: block_height,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Should get current time")
            .as_secs(),
        prev_randao: B256::random(),
    };

    execute_transaction(
        &tx,
        &B256::random(),
        state,
        genesis_config,
        0,
        base_token,
        block_header,
    )
}

pub fn call_transaction(
    request: TransactionRequest,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
    base_token: &impl BaseTokenAccounts,
) -> crate::Result<Vec<u8>> {
    let mut tx = NormalizedEthTransaction::from(request.clone());
    if request.from.is_some() && request.nonce.is_none() {
        tx.nonce = quick_get_nonce(&tx.signer.to_move_address(), state);
    }
    let tx_data = TransactionData::parse_from(&tx)?;

    let move_vm = create_move_vm()?;
    let session_id = SessionId::default();
    let mut session = create_vm_session(&move_vm, state, session_id);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = new_gas_meter(genesis_config, tx.gas_limit());

    verify_transaction(
        &tx,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
        genesis_config,
        0,
        base_token,
    )?;

    match tx_data {
        TransactionData::EntryFunction(entry_fn) => {
            let outcome = session.execute_function_bypass_visibility(
                entry_fn.module(),
                entry_fn.function(),
                entry_fn.ty_args().to_vec(),
                entry_fn.args().to_vec(),
                &mut gas_meter,
                &mut traversal_context,
            )?;
            Ok(bcs::to_bytes(&outcome.return_values)?)
        }
        TransactionData::ScriptOrModule(ScriptOrModule::Script(script)) => {
            crate::move_execution::execute::execute_script(
                script,
                &tx.signer.to_move_address(),
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
            )?;
            Ok(vec![])
        }
        _ => Err(InvalidTransaction(InvalidTransactionCause::UnsupportedType)),
    }
}
