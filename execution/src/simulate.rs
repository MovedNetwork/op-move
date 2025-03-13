use {
    super::{CreateL2GasFee, CreateMovedL2GasFee, L2GasFeeInput},
    crate::{
        canonical::{verify_transaction, CanonicalVerificationInput},
        create_move_vm, create_vm_session, execute_transaction,
        gas::new_gas_meter,
        quick_get_nonce,
        session_id::SessionId,
        transaction::{
            NormalizedEthTransaction, ScriptOrModule, TransactionData, TransactionExecutionOutcome,
        },
        BaseTokenAccounts, CanonicalExecutionInput,
    },
    alloy::rpc::types::TransactionRequest,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_table_extension::TableResolver,
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
    moved_evm_ext::{state::StorageTrieRepository, HeaderForExecution},
    moved_genesis::config::GenesisConfig,
    moved_shared::{
        error::{Error::InvalidTransaction, InvalidTransactionCause},
        primitives::{ToMoveAddress, B256, U256},
    },
    std::time::{SystemTime, UNIX_EPOCH},
};

pub fn simulate_transaction(
    request: TransactionRequest,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    storage_trie: &impl StorageTrieRepository,
    genesis_config: &GenesisConfig,
    base_token: &impl BaseTokenAccounts,
    block_height: u64,
) -> moved_shared::error::Result<TransactionExecutionOutcome> {
    let mut tx = NormalizedEthTransaction::from(request.clone());
    if request.from.is_some() && request.nonce.is_none() {
        tx.nonce = quick_get_nonce(&tx.signer.to_move_address(), state, storage_trie);
    }

    let block_header = HeaderForExecution {
        number: block_height,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Should get current time")
            .as_secs(),
        prev_randao: B256::random(),
    };

    let l2_input = L2GasFeeInput::new(u64::MAX, U256::ZERO);
    let l2_fee = CreateMovedL2GasFee.with_default_gas_fee_multiplier();
    let input = CanonicalExecutionInput {
        tx: &tx,
        tx_hash: &B256::random(),
        state,
        storage_trie,
        genesis_config,
        l1_cost: 0,
        l2_fee,
        l2_input,
        base_token,
        block_header,
    };

    execute_transaction(input.into())
}

pub fn call_transaction(
    request: TransactionRequest,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    storage_trie: &impl StorageTrieRepository,
    genesis_config: &GenesisConfig,
    base_token: &impl BaseTokenAccounts,
) -> moved_shared::error::Result<Vec<u8>> {
    let mut tx = NormalizedEthTransaction::from(request.clone());
    if request.from.is_some() && request.nonce.is_none() {
        tx.nonce = quick_get_nonce(&tx.signer.to_move_address(), state, storage_trie);
    }
    let tx_data = TransactionData::parse_from(&tx)?;

    let move_vm = create_move_vm()?;
    let session_id = SessionId::default();
    let mut session = create_vm_session(&move_vm, state, session_id, storage_trie);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = new_gas_meter(genesis_config, tx.gas_limit());

    let mut verify_input = CanonicalVerificationInput {
        tx: &tx,
        session: &mut session,
        traversal_context: &mut traversal_context,
        gas_meter: &mut gas_meter,
        genesis_config,
        l1_cost: 0,
        l2_cost: 0,
        base_token,
    };
    verify_transaction(&mut verify_input)?;

    match tx_data {
        TransactionData::EntryFunction(entry_fn) => {
            let outcome = verify_input.session.execute_function_bypass_visibility(
                entry_fn.module(),
                entry_fn.function(),
                entry_fn.ty_args().to_vec(),
                entry_fn.args().to_vec(),
                verify_input.gas_meter,
                verify_input.traversal_context,
            )?;
            Ok(bcs::to_bytes(&outcome.return_values)?)
        }
        TransactionData::ScriptOrModule(ScriptOrModule::Script(script)) => {
            crate::execute::execute_script(
                script,
                &tx.signer.to_move_address(),
                verify_input.session,
                verify_input.traversal_context,
                verify_input.gas_meter,
            )?;
            Ok(vec![])
        }
        _ => Err(InvalidTransaction(InvalidTransactionCause::UnsupportedType)),
    }
}
