use {
    super::{L2GasFee, L2GasFeeInput},
    crate::{
        create_move_vm, create_vm_session,
        eth_token::{BaseTokenAccounts, TransferArgs},
        execute::{deploy_module, execute_entry_function, execute_l2_contract, execute_script},
        gas::{new_gas_meter, total_gas_used},
        nonces::check_nonce,
        session_id::SessionId,
        transaction::{
            NormalizedEthTransaction, ScriptOrModule, TransactionData, TransactionExecutionOutcome,
        },
        CanonicalExecutionInput, Logs,
    },
    aptos_gas_meter::{AptosGasMeter, StandardGasAlgebra, StandardGasMeter},
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        session::Session,
    },
    moved_evm_ext::storage::StorageTrieRepository,
    moved_genesis::config::GenesisConfig,
    moved_shared::{
        error::{
            Error::{InvalidTransaction, User},
            EthToken, InvalidTransactionCause, InvariantViolation,
        },
        primitives::{ToMoveAddress, ToSaturatedU64},
    },
};

pub struct CanonicalVerificationInput<'input, 'r, 'l, B> {
    pub tx: &'input NormalizedEthTransaction,
    pub session: &'input mut Session<'r, 'l>,
    pub traversal_context: &'input mut TraversalContext<'input>,
    pub gas_meter: &'input mut StandardGasMeter<StandardGasAlgebra>,
    pub genesis_config: &'input GenesisConfig,
    pub l1_cost: u64,
    pub l2_cost: u64,
    pub base_token: &'input B,
}

pub(super) fn verify_transaction<B: BaseTokenAccounts>(
    input: &mut CanonicalVerificationInput<B>,
) -> moved_shared::error::Result<()> {
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

    input
        .base_token
        .charge_gas_cost(
            &sender_move_address,
            input.l1_cost,
            input.session,
            input.traversal_context,
            input.gas_meter,
        )
        .map_err(|_| InvalidTransaction(InvalidTransactionCause::FailedToPayL1Fee))?;

    input
        .base_token
        .charge_gas_cost(
            &sender_move_address,
            input.l2_cost,
            input.session,
            input.traversal_context,
            input.gas_meter,
        )
        .map_err(|_| InvalidTransaction(InvalidTransactionCause::FailedToPayL2Fee))?;

    check_nonce(
        input.tx.nonce,
        &sender_move_address,
        input.session,
        input.traversal_context,
        input.gas_meter,
    )?;

    Ok(())
}

pub(super) fn execute_canonical_transaction<
    S: MoveResolver<PartialVMError> + TableResolver,
    ST: StorageTrieRepository,
    F: L2GasFee,
    B: BaseTokenAccounts,
>(
    input: CanonicalExecutionInput<S, ST, F, B>,
) -> moved_shared::error::Result<TransactionExecutionOutcome> {
    let sender_move_address = input.tx.signer.to_move_address();

    let tx_data = TransactionData::parse_from(input.tx)?;

    let move_vm = create_move_vm()?;
    let session_id = SessionId::new_from_canonical(
        input.tx,
        tx_data.maybe_entry_fn(),
        input.tx_hash,
        input.genesis_config,
        input.block_header,
        tx_data.script_hash(),
    );
    let mut session = create_vm_session(&move_vm, input.state, session_id, input.storage_trie);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);

    let mut gas_meter = new_gas_meter(input.genesis_config, input.l2_input.gas_limit);
    let mut deployment = None;
    // Using l2 input here as test transactions don't set the max limit directly on itself
    let l2_cost = input.l2_fee.l2_fee(input.l2_input.clone()).saturating_to();
    let mut evm_logs = Vec::new();

    // TODO: use free gas meter for things that shouldn't fail due to
    // insufficient gas limit, impose a lower bound on the latter
    let mut verify_input = CanonicalVerificationInput {
        tx: input.tx,
        session: &mut session,
        traversal_context: &mut traversal_context,
        gas_meter: &mut gas_meter,
        genesis_config: input.genesis_config,
        l1_cost: input.l1_cost,
        l2_cost,
        base_token: input.base_token,
    };
    verify_transaction(&mut verify_input)?;

    let vm_outcome = match tx_data {
        TransactionData::EntryFunction(entry_fn) => execute_entry_function(
            entry_fn,
            &sender_move_address,
            verify_input.session,
            verify_input.traversal_context,
            verify_input.gas_meter,
        ),
        TransactionData::ScriptOrModule(ScriptOrModule::Script(script)) => execute_script(
            script,
            &sender_move_address,
            verify_input.session,
            verify_input.traversal_context,
            verify_input.gas_meter,
        ),
        TransactionData::ScriptOrModule(ScriptOrModule::Module(module)) => {
            let module_id = deploy_module(
                module,
                sender_move_address,
                verify_input.session,
                verify_input.gas_meter,
            );
            module_id.map(|id| {
                deployment = Some((sender_move_address, id));
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
                verify_input.session,
                verify_input.traversal_context,
                verify_input.gas_meter,
            )
        }
        TransactionData::L2Contract(contract) => {
            evm_logs = execute_l2_contract(
                &sender_move_address,
                &contract.to_move_address(),
                input.tx.value,
                input.tx.data.to_vec(),
                verify_input.session,
                verify_input.traversal_context,
                verify_input.gas_meter,
            )?;
            Ok(())
        }
    };

    let gas_used = total_gas_used(verify_input.gas_meter, input.genesis_config);
    let used_l2_input = L2GasFeeInput::new(gas_used, input.l2_input.effective_gas_price);
    let used_l2_cost = input.l2_fee.l2_fee(used_l2_input).to_saturated_u64();

    // Refunds should not be metered as they're supposed to always succeed
    input
        .base_token
        .refund_gas_cost(
            &sender_move_address,
            l2_cost.saturating_sub(used_l2_cost),
            verify_input.session,
            verify_input.traversal_context,
        )
        .map_err(|_| {
            moved_shared::error::Error::InvariantViolation(InvariantViolation::EthToken(
                EthToken::RefundAlwaysSucceeds,
            ))
        })?;

    let (mut changes, mut extensions) = session.finish_with_extensions()?;
    let mut logs = extensions.logs();
    logs.extend(evm_logs);
    let evm_changes = moved_evm_ext::extract_evm_changes(&extensions);
    changes
        .squash(evm_changes)
        .expect("EVM changes must merge with other session changes");

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
