use {
    super::{CreateL2GasFee, CreateUmiL2GasFee, L2GasFeeInput},
    crate::{
        BaseTokenAccounts, CanonicalExecutionInput,
        canonical::{CanonicalVerificationInput, verify_transaction},
        create_vm_session,
        execute::execute_evm_contract,
        execute_transaction,
        gas::new_gas_meter,
        quick_get_nonce,
        resolver_cache::ResolverCache,
        session_id::SessionId,
        transaction::{
            NormalizedEthTransaction, ScriptOrDeployment, TransactionData,
            TransactionExecutionOutcome,
        },
    },
    alloy::rpc::types::TransactionRequest,
    move_table_extension::TableResolver,
    move_vm_runtime::{
        AsUnsyncCodeStorage,
        module_traversal::{TraversalContext, TraversalStorage},
    },
    move_vm_types::resolver::MoveResolver,
    std::time::{SystemTime, UNIX_EPOCH},
    umi_evm_ext::{
        HeaderForExecution,
        state::{BlockHashLookup, StorageTrieRepository},
    },
    umi_genesis::{CreateMoveVm, UmiVm, config::GenesisConfig},
    umi_shared::{
        error::{Error::InvalidTransaction, InvalidTransactionCause},
        primitives::{B256, ToMoveAddress, U256},
    },
    umi_state::ResolverBasedModuleBytesStorage,
};

pub fn simulate_transaction(
    request: TransactionRequest,
    state: &(impl MoveResolver + TableResolver),
    storage_trie: &impl StorageTrieRepository,
    genesis_config: &GenesisConfig,
    base_token: &impl BaseTokenAccounts,
    block_height: u64,
    block_hash_lookup: &impl BlockHashLookup,
) -> umi_shared::error::Result<TransactionExecutionOutcome> {
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
    let l2_fee = CreateUmiL2GasFee.with_default_gas_fee_multiplier();
    let input = CanonicalExecutionInput {
        tx: &tx,
        tx_hash: &B256::random(),
        state,
        storage_trie,
        genesis_config,
        l1_cost: U256::ONE,
        l2_fee,
        l2_input,
        base_token,
        block_header,
        block_hash_lookup,
    };

    execute_transaction(input.into(), &mut ResolverCache::default())
}

pub fn call_transaction(
    request: TransactionRequest,
    state: &(impl MoveResolver + TableResolver),
    storage_trie: &impl StorageTrieRepository,
    genesis_config: &GenesisConfig,
    base_token: &impl BaseTokenAccounts,
    block_hash_lookup: &impl BlockHashLookup,
) -> umi_shared::error::Result<Vec<u8>> {
    let mut tx = NormalizedEthTransaction::from(request.clone());
    if request.from.is_some() && request.nonce.is_none() {
        tx.nonce = quick_get_nonce(&tx.signer.to_move_address(), state, storage_trie);
    }
    let tx_data = TransactionData::parse_from(&tx)?;

    let umi_vm = UmiVm::new(genesis_config);
    let vm = umi_vm.create_move_vm()?;
    let module_storage_bytes = ResolverBasedModuleBytesStorage::new(state);
    let code_storage = module_storage_bytes.as_unsync_code_storage(&umi_vm);
    let session_id = SessionId::default();
    let mut session =
        create_vm_session(&vm, state, session_id, storage_trie, &(), block_hash_lookup);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = new_gas_meter(genesis_config, tx.gas_limit());

    verify_transaction(CanonicalVerificationInput {
        tx: &tx,
        session: &mut session,
        traversal_context: &mut traversal_context,
        gas_meter: &mut gas_meter,
        genesis_config,
        l1_cost: U256::ZERO,
        l2_cost: U256::ZERO,
        base_token,
        module_storage: &code_storage,
    })?;

    match tx_data {
        TransactionData::EntryFunction(entry_fn) => {
            let outcome = session.execute_function_bypass_visibility(
                entry_fn.module(),
                entry_fn.function(),
                entry_fn.ty_args().to_vec(),
                entry_fn.args().to_vec(),
                &mut gas_meter,
                &mut traversal_context,
                &code_storage,
            )?;
            // Only return the results of the transaction in bytes without the Move value layout.
            // Sending just the bytes works better when it comes to parsing on the client side.
            Ok(bcs::to_bytes(
                &outcome
                    .return_values
                    .into_iter()
                    .map(|(bytes, _ty)| bytes)
                    .collect::<Vec<_>>(),
            )?)
        }
        TransactionData::ScriptOrDeployment(ScriptOrDeployment::Script(script)) => {
            crate::execute::execute_script(
                script,
                &tx.signer.to_move_address(),
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
                &code_storage,
            )?;
            Ok(vec![])
        }
        TransactionData::L2Contract(contract) => {
            let outcome = execute_evm_contract(
                &tx.signer.to_move_address(),
                &contract.to_move_address(),
                tx.value,
                tx.data.to_vec(),
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
                &code_storage,
            )?;
            Ok(outcome.output)
        }
        TransactionData::EvmContract { address, data } => {
            let outcome = execute_evm_contract(
                &tx.signer.to_move_address(),
                &address.to_move_address(),
                tx.value,
                data,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
                &code_storage,
            )?;
            Ok(outcome.output)
        }
        _ => Err(InvalidTransaction(InvalidTransactionCause::UnsupportedType)),
    }
}
