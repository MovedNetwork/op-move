use {
    crate::{
        create_move_vm, create_vm_session, eth_token,
        gas::{new_gas_meter, total_gas_used},
        session_id::SessionId,
        transaction::TransactionExecutionOutcome,
        DepositExecutionInput, ADDRESS_LAYOUT, U256_LAYOUT,
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
        self, extract_evm_changes, extract_evm_result, EvmNativeOutcome, CODE_LAYOUT,
        EVM_CALL_FN_NAME, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE,
    },
    moved_shared::{
        error::UserError,
        primitives::{ToMoveAddress, ToMoveU256, B256},
    },
};

#[cfg(any(feature = "test-doubles", test))]
use {
    crate::transaction::DepositedTx, moved_evm_ext::HeaderForExecution,
    moved_genesis::config::GenesisConfig,
};

// Topic identifying the event
// ETHBridgeFinalized(address indexed from, address indexed to, uint256 amount, bytes extraData)
const ETH_BRIDGE_FINALIZED: B256 = B256::new(hex!(
    "31b2166ff604fc5672ea5df08a78081d2bc6d746cadce880747f3643d819e83d"
));

// Topic identifying the event
// ERC20BridgeFinalized(address indexed localToken, address indexed remoteToken, address indexed from, address to, uint256 amount, bytes extraData)
const ERC20_BRIDGE_FINALIZED: B256 = B256::new(hex!(
    "d59c65b35445225835c83f50b6ede06a7be047d22e357073e250d9af537518cd"
));

const ERC20_BRIDGE_INITIATED: B256 = B256::new(hex!(
    "2849b43074093a05396b6f2a937dee8565b15a48a7b3d4bffb732a5017380af5"
));

pub(super) fn execute_deposited_transaction<S: MoveResolver<PartialVMError> + TableResolver>(
    input: DepositExecutionInput<S>,
) -> moved_shared::error::Result<TransactionExecutionOutcome> {
    #[cfg(any(feature = "test-doubles", test))]
    if input.tx.data.is_empty() {
        eprintln!("in deposited: tx data empty");
        return direct_mint(
            input.tx,
            input.tx_hash,
            input.state,
            input.genesis_config,
            input.block_header,
        );
    }

    eprintln!("in deposited: tx data yes");
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
    dbg!(&input.tx.data);
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
    dbg!(&outcome);
    let mint_params_with_logs = outcome.and_then(|values| {
        let evm_outcome = extract_evm_result(values);
        dbg!(&evm_outcome);
        if !evm_outcome.is_success {
            return Err(UserError::DepositFailure(evm_outcome.output));
        }
        let mint_params = get_mint_params(&evm_outcome);
        dbg!(&mint_params);
        Ok((mint_params, evm_outcome.logs))
    });

    let (logs, vm_outcome) = match mint_params_with_logs {
        Ok((Some(mint_params), logs)) => {
            eprintln!("found mint params: {:?}", &mint_params);
            eth_token::mint_eth(
                &mint_params.destination,
                mint_params.amount,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
            )?;
            (logs, Ok(()))
        }
        Ok((None, logs)) => {
            eprintln!("found empty mint params. logs: {:?}", logs);
            (logs, Ok(()))
        }
        Err(e) => (Vec::new(), Err(e)),
    };

    let (mut changes, extensions) = session.finish_with_extensions()?;
    let gas_used = total_gas_used(&gas_meter, input.genesis_config);
    let evm_changes = extract_evm_changes(&extensions);
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
    eprintln!("in mint params func");
    let bridge_log = outcome
        .logs
        .iter()
        .find(|l| l.topics()[0] == ETH_BRIDGE_FINALIZED)?;
    dbg!(&bridge_log);
    let erc20_log = outcome
        .logs
        .iter()
        .find(|l| l.topics()[0] == ERC20_BRIDGE_FINALIZED)?;
    dbg!(erc20_log);
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

#[derive(Debug)]
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
) -> moved_shared::error::Result<TransactionExecutionOutcome> {
    use crate::Logs;

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
