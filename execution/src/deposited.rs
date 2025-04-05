use {
    crate::{
        ADDRESS_LAYOUT, DepositExecutionInput, Logs, U256_LAYOUT, create_vm_session, eth_token,
        gas::{new_gas_meter, total_gas_used},
        session_id::SessionId,
        transaction::{Changes, TransactionExecutionOutcome},
    },
    alloy::primitives::U256,
    aptos_table_natives::TableResolver,
    move_core_types::language_storage::ModuleId,
    move_vm_runtime::{
        AsUnsyncCodeStorage,
        module_traversal::{TraversalContext, TraversalStorage},
    },
    move_vm_types::{resolver::MoveResolver, value_serde::ValueSerDeContext, values::Value},
    moved_evm_ext::{
        self, CODE_LAYOUT, EVM_CALL_FN_NAME, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE,
        events::EthTransfersLogger, extract_evm_changes, extract_evm_result,
        state::StorageTrieRepository,
    },
    moved_genesis::{CreateMoveVm, MovedVm},
    moved_shared::{
        error::{Error, UserError},
        primitives::{ToMoveAddress, ToMoveU256},
    },
    moved_state::ResolverBasedModuleBytesStorage,
};

pub(super) fn execute_deposited_transaction<
    S: MoveResolver + TableResolver,
    ST: StorageTrieRepository,
>(
    input: DepositExecutionInput<S, ST>,
) -> moved_shared::error::Result<TransactionExecutionOutcome> {
    let moved_vm = MovedVm::default();
    let module_bytes_storage = ResolverBasedModuleBytesStorage::new(input.state);
    let code_storage = module_bytes_storage.as_unsync_code_storage(&moved_vm);
    let vm = moved_vm.create_move_vm()?;
    let session_id = SessionId::new_from_deposited(
        input.tx,
        input.tx_hash,
        input.genesis_config,
        input.block_header,
    );
    let eth_transfers_log = EthTransfersLogger::default();
    let mut session = create_vm_session(
        &vm,
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
    let args: Vec<Vec<u8>> = [
        (
            Value::address(input.tx.from.to_move_address()),
            &ADDRESS_LAYOUT,
        ),
        (
            Value::address(input.tx.to.to_move_address()),
            &ADDRESS_LAYOUT,
        ),
        (Value::u256(input.tx.value.to_move_u256()), &U256_LAYOUT),
        (
            Value::vector_u8(input.tx.data.iter().copied()),
            &CODE_LAYOUT,
        ),
    ]
    .into_iter()
    .map(|(value, layout)| {
        ValueSerDeContext::new()
            .serialize(&value, layout)
            .unwrap()
            .unwrap()
    })
    .collect();
    let outcome = session
        .execute_function_bypass_visibility(
            &module,
            function_name,
            Vec::new(),
            args,
            &mut gas_meter,
            &mut traversal_context,
            &code_storage,
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
            // accounts according to the transfers that happened
            // during EVM execution.
            if !input.tx.mint.is_zero() {
                eth_token::mint_eth(
                    &EVM_NATIVE_ADDRESS,
                    input.tx.mint,
                    &mut session,
                    &mut traversal_context,
                    &mut gas_meter,
                    &code_storage,
                )?;
            }
            eth_token::replicate_transfers(
                &eth_transfers_log,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
                &code_storage,
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

    let (mut changes, mut extensions) = session.finish_with_extensions(&code_storage)?;
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
