use {
    crate::{
        create_move_vm, create_vm_session, eth_token,
        gas::{new_gas_meter, total_gas_used},
        session_id::SessionId,
        transaction::{Changes, TransactionExecutionOutcome},
        DepositExecutionInput, Logs, ADDRESS_LAYOUT, U256_LAYOUT,
    },
    alloy::primitives::U256,
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::{language_storage::ModuleId, resolver::MoveResolver},
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
    move_vm_types::values::Value,
    moved_evm_ext::{
        self, events::EthTransfersLogger, extract_evm_changes, extract_evm_result,
        state::StorageTrieRepository, CODE_LAYOUT, EVM_CALL_FN_NAME, EVM_NATIVE_ADDRESS,
        EVM_NATIVE_MODULE,
    },
    moved_shared::{
        error::{Error, UserError},
        primitives::{ToMoveAddress, ToMoveU256},
    },
};

pub(super) fn execute_deposited_transaction<
    S: MoveResolver<PartialVMError> + TableResolver,
    ST: StorageTrieRepository,
>(
    input: DepositExecutionInput<S, ST>,
) -> moved_shared::error::Result<TransactionExecutionOutcome> {
    let move_vm = create_move_vm()?;
    let session_id = SessionId::new_from_deposited(
        input.tx,
        input.tx_hash,
        input.genesis_config,
        input.block_header,
    );
    let eth_transfers_log = EthTransfersLogger::default();
    let mut session = create_vm_session(
        &move_vm,
        input.state,
        session_id,
        input.storage_trie,
        &eth_transfers_log,
    );
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    // The type of `tx.gas` is essentially `[u64; 1]` so taking the 0th element
    // is a 1:1 mapping to `u64`.
    let mut gas_meter = new_gas_meter(input.genesis_config, input.tx.gas.as_limbs()[0]);

    let module = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
    let function_name = EVM_CALL_FN_NAME;
    // Unwraps in serialization are safe because the layouts match the types.
    let args = vec![
        Value::address(input.tx.from.to_move_address())
            .simple_serialize(&ADDRESS_LAYOUT)
            .unwrap(),
        Value::address(input.tx.to.to_move_address())
            .simple_serialize(&ADDRESS_LAYOUT)
            .unwrap(),
        Value::u256(input.tx.value.to_move_u256())
            .simple_serialize(&U256_LAYOUT)
            .unwrap(),
        Value::vector_u8(input.tx.data.iter().copied())
            .simple_serialize(&CODE_LAYOUT)
            .unwrap(),
    ];
    let outcome = session
        .execute_function_bypass_visibility(
            &module,
            function_name,
            Vec::new(),
            args,
            &mut gas_meter,
            &mut traversal_context,
        )
        .map_err(Error::from)
        .and_then(|values| {
            let evm_outcome = extract_evm_result(values);
            if !evm_outcome.is_success {
                return Err(UserError::DepositFailure(evm_outcome.output).into());
            }

            // If there is a non-zero mint amount then we start by
            // giving those tokens to the EVM native address.
            // The tokens will then be distributed to the correct
            // accounts according to the transferred that happened
            // during EVM execution.
            if !input.tx.mint.is_zero() {
                eth_token::mint_eth(
                    &EVM_NATIVE_ADDRESS,
                    input.tx.mint,
                    &mut session,
                    &mut traversal_context,
                    &mut gas_meter,
                )?;
            }
            eth_token::replicate_transfers(
                &eth_transfers_log,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
            )?;

            Ok(evm_outcome.logs)
        });

    let (evm_logs, vm_outcome) = match outcome {
        Ok(logs) => (logs, Ok(())),
        Err(Error::User(e)) => (Vec::new(), Err(e)),
        Err(e) => {
            return Err(e);
        }
    };

    let (mut changes, mut extensions) = session.finish_with_extensions()?;
    let mut logs = extensions.logs();
    logs.extend(evm_logs);
    let gas_used = total_gas_used(&gas_meter, input.genesis_config);
    let evm_changes = extract_evm_changes(&extensions);
    changes
        .squash(evm_changes.accounts)
        .expect("EVM changes must merge with other session changes");
    let changes = Changes::new(changes, evm_changes.storage);

    Ok(TransactionExecutionOutcome::new(
        vm_outcome,
        changes,
        gas_used,
        // No L2 gas for deposited txs
        U256::ZERO,
        logs,
        None,
    ))
}
