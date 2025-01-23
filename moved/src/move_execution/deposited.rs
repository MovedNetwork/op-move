use {
    crate::{
        error::UserError,
        genesis::config::GenesisConfig,
        move_execution::{
            create_move_vm, create_vm_session, eth_token,
            gas::{new_gas_meter, total_gas_used},
            DepositExecutionInput, ADDRESS_LAYOUT, U256_LAYOUT,
        },
        primitives::{ToMoveAddress, ToMoveU256, B256},
        types::{
            session_id::SessionId,
            transactions::{DepositedTx, TransactionExecutionOutcome},
        },
    },
    alloy::{hex, primitives::U256},
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress, language_storage::ModuleId, resolver::MoveResolver,
    },
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
    move_vm_types::values::Value,
    moved_evm_ext::{
        evm_native::{self, EvmNativeOutcome},
        native_evm_context::HeaderForExecution,
    },
};

// Topic identifying the event
// ETHBridgeFinalized(address indexed from, address indexed to, uint256 amount, bytes extraData)
const ETH_BRIDGE_FINALIZED: B256 = B256::new(hex!(
    "31b2166ff604fc5672ea5df08a78081d2bc6d746cadce880747f3643d819e83d"
));

pub(super) fn execute_deposited_transaction<S: MoveResolver<PartialVMError> + TableResolver>(
    input: DepositExecutionInput<S>,
) -> crate::Result<TransactionExecutionOutcome> {
    #[cfg(any(feature = "test-doubles", test))]
    if input.tx.data.is_empty() {
        return direct_mint(
            input.tx,
            input.tx_hash,
            input.state,
            input.genesis_config,
            input.block_header,
        );
    }

    let move_vm = create_move_vm()?;
    let session_id = SessionId::new_from_deposited(
        input.tx,
        input.tx_hash,
        input.genesis_config,
        input.block_header,
    );
    let mut session = create_vm_session(&move_vm, input.state, session_id);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    // The type of `tx.gas` is essentially `[u64; 1]` so taking the 0th element
    // is a 1:1 mapping to `u64`.
    let mut gas_meter = new_gas_meter(input.genesis_config, input.tx.gas.as_limbs()[0]);

    let module = ModuleId::new(
        evm_native::EVM_NATIVE_ADDRESS,
        evm_native::EVM_NATIVE_MODULE.into(),
    );
    let function_name = evm_native::EVM_CALL_FN_NAME;
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
            .simple_serialize(&evm_native::CODE_LAYOUT)
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
        .map_err(Into::into);

    let mint_params = outcome.and_then(|values| {
        let evm_outcome = evm_native::extract_evm_result(values);
        if !evm_outcome.is_success {
            return Err(UserError::DepositFailure(evm_outcome.output));
        }
        let mint_params = get_mint_params(&evm_outcome);
        Ok((mint_params, evm_outcome.logs))
    });

    let (logs, vm_outcome) = match mint_params {
        Ok((Some(mint_params), logs)) => {
            eth_token::mint_eth(
                &mint_params.destination,
                mint_params.amount,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
            )?;
            (logs, Ok(()))
        }
        Ok((None, logs)) => (logs, Ok(())),
        Err(e) => (Vec::new(), Err(e)),
    };

    let (mut changes, extensions) = session.finish_with_extensions()?;
    let gas_used = total_gas_used(&gas_meter, input.genesis_config);
    let evm_changes = evm_native::extract_evm_changes(&extensions);
    changes
        .squash(evm_changes)
        .expect("EVM changes must merge with other session changes");

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

// Note: Not all deposit-type transactions are actual deposits; hence
// why the return value of this function is an `Option`. Deposit-type
// transactions are produced by the sequencer to call L2 contracts other
// than the bridge.
fn get_mint_params(outcome: &EvmNativeOutcome) -> Option<MintParams> {
    // TODO: Should consider ERC-20 deposits here too?
    let bridge_log = outcome
        .logs
        .iter()
        .find(|l| l.topics()[0] == ETH_BRIDGE_FINALIZED)?;
    // For the ETHBridgeFinalized log the topics are:
    // topics[0]: 0x31b2166ff604fc5672ea5df08a78081d2bc6d746cadce880747f3643d819e83d (fixed identifier)
    // topics[1]: from address (sender)
    // topics[2]: to address (destination)
    let destination = AccountAddress::new(bridge_log.topics()[2].0);
    // For the ETHBridgeFinalized log the data is Solidity ABI encoded a tuple consisting of
    // 1. amount deposited (32 bytes since it is U256)
    // 2. extra data (optional; ignored by us)
    let amount = U256::from_be_slice(&bridge_log.data.data[..32]);
    Some(MintParams {
        destination,
        amount,
    })
}

struct MintParams {
    destination: AccountAddress,
    amount: U256,
}

/// This function is only used in tests.
/// It allows us to mint ETH directly without going through the EVM.
#[cfg(any(feature = "test-doubles", test))]
fn direct_mint(
    tx: &DepositedTx,
    tx_hash: &B256,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
    block_header: HeaderForExecution,
) -> crate::Result<TransactionExecutionOutcome> {
    use crate::move_execution::Logs;

    let amount = tx.mint.saturating_add(tx.value);
    let to = tx.to.to_move_address();

    let move_vm = create_move_vm()?;
    let session_id = SessionId::new_from_deposited(tx, tx_hash, genesis_config, block_header);
    let mut session = create_vm_session(&move_vm, state, session_id);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    // The type of `tx.gas` is essentially `[u64; 1]` so taking the 0th element
    // is a 1:1 mapping to `u64`.
    let mut gas_meter = new_gas_meter(genesis_config, tx.gas.as_limbs()[0]);

    eth_token::mint_eth(
        &to,
        amount,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
    )?;

    debug_assert!(
        eth_token::get_eth_balance(&to, &mut session, &mut traversal_context, &mut gas_meter)?
            >= amount,
        "tokens were minted"
    );

    let (changes, mut extensions) = session.finish_with_extensions()?;
    let gas_used = total_gas_used(&gas_meter, genesis_config);
    let logs = extensions.logs();

    Ok(TransactionExecutionOutcome::new(
        Ok(()),
        changes,
        gas_used,
        // No L2 gas for deposited txs
        U256::ZERO,
        logs,
        None,
    ))
}
