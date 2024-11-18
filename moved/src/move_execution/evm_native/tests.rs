use {
    super::{
        state_changes::extract_evm_changes, CODE_LAYOUT, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE,
    },
    crate::{
        block::HeaderForExecution,
        genesis::config::GenesisConfig,
        move_execution::{create_move_vm, create_vm_session, execute_transaction, tests::*},
        primitives::{ToEthAddress, ToMoveAddress, ToMoveU256},
        storage::{InMemoryState, State},
        tests::{signer::Signer, ALT_EVM_ADDRESS, EVM_ADDRESS, PRIVATE_KEY},
        types::session_id::SessionId,
    },
    alloy::{
        primitives::utils::parse_ether,
        providers::{self, network::AnyNetwork},
        sol,
    },
    aptos_table_natives::TableResolver,
    aptos_types::transaction::EntryFunction,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        effects::ChangeSet,
        ident_str,
        language_storage::ModuleId,
        resolver::MoveResolver,
        value::{MoveStructLayout, MoveTypeLayout, MoveValue},
    },
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        native_extensions::NativeContextExtensions,
        session::SerializedReturnValues,
    },
    move_vm_types::{
        gas::UnmeteredGasMeter,
        values::{Struct, Value, Vector},
    },
    revm::primitives::{Log, TxKind, U256},
};

sol!(
    #[sol(rpc)]
    ERC20,
    "../server/src/tests/res/ERC20.json"
);

/// Tests that EVM native works by deploying an ERC-20 contract and
/// then having a user transfer some tokens between accounts.
#[test]
fn test_evm() {
    // -------- Initialize state
    let genesis_config = GenesisConfig::default();
    let mut signer = Signer::new(&PRIVATE_KEY);
    let (erc20_move_interface, mut state) =
        deploy_contract("erc20_interface", &mut signer, &genesis_config);

    // -------- Setup ERC-20 interface
    let mint_amount = parse_ether("1").unwrap();
    let provider = providers::builder::<AnyNetwork>()
        .with_recommended_fillers()
        .on_http("http://localhost:1234".parse().unwrap());
    let deploy = ERC20::deploy_builder(
        &provider,
        "Gold".into(),
        "AU".into(),
        EVM_ADDRESS,
        mint_amount,
    );

    // -------- Deploy ERC-20 token
    let (outcome, mut changes, extensions) =
        evm_quick_create(deploy.calldata().to_vec(), state.resolver());

    assert!(outcome.is_success, "Contract deploy must succeed");

    // The ERC-20 contract produces a log because it minted some tokens.
    // We can use this log to get the address of the newly deployed contract.
    let contract_address = outcome.logs[0].address;
    let deployed_contract = ERC20::new(contract_address, &provider);
    let contract_move_address = contract_address.to_move_address();

    let evm_changes = extract_evm_changes(&extensions);
    changes.squash(evm_changes).unwrap();
    drop(extensions);

    state.apply(changes).unwrap();

    // -------- Transfer ERC-20 tokens
    let transfer_amount = parse_ether("0.35").unwrap();
    let user_address = EVM_ADDRESS.to_move_address();
    let signer_input_arg = MoveValue::Signer(user_address);
    let to_input_arg = MoveValue::Address(contract_move_address);
    let transfer_call = deployed_contract.transfer(ALT_EVM_ADDRESS, transfer_amount);
    let data_input_arg = Value::vector_u8(transfer_call.calldata().clone());
    let entry_fn = EntryFunction::new(
        ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into()),
        ident_str!("entry_evm_call").into(),
        Vec::new(),
        vec![
            bcs::to_bytes(&signer_input_arg).unwrap(),
            bcs::to_bytes(&to_input_arg).unwrap(),
            data_input_arg.simple_serialize(&CODE_LAYOUT).unwrap(),
        ],
    );
    let (tx_hash, tx) = create_transaction(
        &mut signer,
        TxKind::Call(EVM_NATIVE_ADDRESS.to_eth_address()),
        bcs::to_bytes(&entry_fn).unwrap(),
    );

    let outcome = execute_transaction(
        &tx,
        &tx_hash,
        state.resolver(),
        &genesis_config,
        0,
        &(),
        HeaderForExecution::default(),
    )
    .unwrap();
    outcome.vm_outcome.unwrap();
    state.apply(outcome.changes).unwrap();

    // -------- Validate ERC-20 balances
    let balance_of = |address, state: &InMemoryState| {
        let balance_of_call = deployed_contract.balanceOf(address);
        let (outcome, _, _) = evm_quick_call(
            EVM_NATIVE_ADDRESS,
            contract_move_address,
            balance_of_call.calldata().to_vec(),
            state.resolver(),
        );
        U256::from_be_slice(&outcome.output)
    };
    let sender_balance = balance_of(EVM_ADDRESS, &state);
    let receiver_balance = balance_of(ALT_EVM_ADDRESS, &state);

    assert_eq!(sender_balance, mint_amount - transfer_amount);
    assert_eq!(receiver_balance, transfer_amount);

    // -------- Transfer ERC-20 tokens (Move interface this time)
    let token_address_input_arg = MoveValue::Address(contract_move_address);
    let to_input_arg = MoveValue::Address(ALT_EVM_ADDRESS.to_move_address());
    let amount_arg = transfer_amount.to_move_u256();
    let entry_fn = EntryFunction::new(
        erc20_move_interface,
        ident_str!("erc20_transfer").into(),
        Vec::new(),
        vec![
            bcs::to_bytes(&token_address_input_arg).unwrap(),
            bcs::to_bytes(&signer_input_arg).unwrap(),
            bcs::to_bytes(&to_input_arg).unwrap(),
            bcs::to_bytes(&amount_arg).unwrap(),
        ],
    );
    let (tx_hash, tx) = create_transaction(
        &mut signer,
        TxKind::Call(EVM_ADDRESS),
        bcs::to_bytes(&entry_fn).unwrap(),
    );

    let outcome = execute_transaction(
        &tx,
        &tx_hash,
        state.resolver(),
        &genesis_config,
        0,
        &(),
        HeaderForExecution::default(),
    )
    .unwrap();
    outcome.vm_outcome.unwrap();
    state.apply(outcome.changes).unwrap();

    // -------- Validate ERC-20 balances (again)
    let sender_balance = balance_of(EVM_ADDRESS, &state);
    let receiver_balance = balance_of(ALT_EVM_ADDRESS, &state);

    assert_eq!(
        sender_balance,
        mint_amount - transfer_amount - transfer_amount
    );
    assert_eq!(receiver_balance, transfer_amount + transfer_amount);
}

#[test]
fn test_solidity_fixed_bytes() {
    let genesis_config = GenesisConfig::default();
    let mut signer = Signer::new(&PRIVATE_KEY);
    let (contract, mut state) =
        deploy_contract("solidity_fixed_bytes", &mut signer, &genesis_config);

    let mut call_contract = |input: Vec<u8>, state: &InMemoryState| {
        let arg = MoveValue::vector_u8(input);
        let entry_fn = EntryFunction::new(
            contract.clone(),
            ident_str!("encode_fixed_bytes").into(),
            Vec::new(),
            vec![bcs::to_bytes(&arg).unwrap()],
        );
        let (tx_hash, tx) = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );
        execute_transaction(
            &tx,
            &tx_hash,
            state.resolver(),
            &genesis_config,
            0,
            &(),
            HeaderForExecution::default(),
        )
        .unwrap()
    };

    // Calling with empty bytes is an error
    let outcome = call_contract(Vec::new(), &state);
    outcome.vm_outcome.unwrap_err();
    state.apply(outcome.changes).unwrap();

    // Calling with bytes longer than 32 is an error
    let outcome = call_contract(vec![0x88; 33], &state);
    outcome.vm_outcome.unwrap_err();
    state.apply(outcome.changes).unwrap();

    // Calling with any length between 1 and 32 (inclusive) works
    for n in 1..=32 {
        let outcome = call_contract(vec![0x88; n], &state);
        outcome.vm_outcome.unwrap();
        state.apply(outcome.changes).unwrap();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvmNativeOutcome {
    is_success: bool,
    output: Vec<u8>,
    logs: Vec<Log>,
}

/// Create MoveVM instance and invoke EVM create native.
/// For tests only since it does not use an existing session or charge gas.
fn evm_quick_create(
    contract_bytecode: Vec<u8>,
    resolver: &(impl MoveResolver<PartialVMError> + TableResolver),
) -> (EvmNativeOutcome, ChangeSet, NativeContextExtensions) {
    let move_vm = create_move_vm().unwrap();
    let session_id = SessionId::default();
    let mut session = create_vm_session(&move_vm, resolver, session_id);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = UnmeteredGasMeter;

    let module_id = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
    let args = vec![
        // From
        Value::address(EVM_NATIVE_ADDRESS)
            .simple_serialize(&MoveTypeLayout::Address)
            .unwrap(),
        // Value
        serialize_fungible_asset_value(0),
        // Data (code to deploy)
        Value::vector_u8(contract_bytecode)
            .simple_serialize(&CODE_LAYOUT)
            .unwrap(),
    ];

    let outcome = session
        .execute_function_bypass_visibility(
            &module_id,
            ident_str!("evm_create"),
            Vec::new(),
            args,
            &mut gas_meter,
            &mut traversal_context,
        )
        .unwrap();

    let outcome = extract_evm_result(outcome);
    let (changes, extensions) = session.finish_with_extensions().unwrap();
    (outcome, changes, extensions)
}

/// Create MoveVM instance and invoke EVM call native.
/// For tests only since it does not use an existing session or charge gas.
fn evm_quick_call(
    from: AccountAddress,
    to: AccountAddress,
    data: Vec<u8>,
    resolver: &(impl MoveResolver<PartialVMError> + TableResolver),
) -> (EvmNativeOutcome, ChangeSet, NativeContextExtensions) {
    let move_vm = create_move_vm().unwrap();
    let session_id = SessionId::default();
    let mut session = create_vm_session(&move_vm, resolver, session_id);
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = UnmeteredGasMeter;

    let module_id = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
    let args = vec![
        // From
        Value::address(from)
            .simple_serialize(&MoveTypeLayout::Address)
            .unwrap(),
        // to
        Value::address(to)
            .simple_serialize(&MoveTypeLayout::Address)
            .unwrap(),
        // Value
        serialize_fungible_asset_value(0),
        // Data (code to deploy)
        Value::vector_u8(data)
            .simple_serialize(&CODE_LAYOUT)
            .unwrap(),
    ];

    let outcome = session
        .execute_function_bypass_visibility(
            &module_id,
            ident_str!("evm_call"),
            Vec::new(),
            args,
            &mut gas_meter,
            &mut traversal_context,
        )
        .unwrap();

    let outcome = extract_evm_result(outcome);
    let (changes, extensions) = session.finish_with_extensions().unwrap();
    (outcome, changes, extensions)
}

fn extract_evm_result(outcome: SerializedReturnValues) -> EvmNativeOutcome {
    let mut return_values = outcome
        .return_values
        .into_iter()
        .map(|(bytes, layout)| Value::simple_deserialize(&bytes, &layout).unwrap());

    let mut evm_result_fields = return_values
        .next()
        .unwrap()
        .value_as::<Struct>()
        .unwrap()
        .unpack()
        .unwrap();

    assert!(
        return_values.next().is_none(),
        "There is only one return value."
    );

    let is_success: bool = evm_result_fields.next().unwrap().value_as().unwrap();
    let output: Vec<u8> = evm_result_fields.next().unwrap().value_as().unwrap();
    let logs: Vec<Value> = evm_result_fields.next().unwrap().value_as().unwrap();
    let logs = logs
        .into_iter()
        .map(|value| {
            let mut fields = value.value_as::<Struct>().unwrap().unpack().unwrap();

            let address = fields.next().unwrap().value_as::<AccountAddress>().unwrap();
            let topics = fields
                .next()
                .unwrap()
                .value_as::<Vector>()
                .unwrap()
                .unpack_unchecked()
                .unwrap();
            let data = fields.next().unwrap().value_as::<Vec<u8>>().unwrap();

            Log::new(
                address.to_eth_address(),
                topics
                    .into_iter()
                    .map(|value| {
                        value
                            .value_as::<move_core_types::u256::U256>()
                            .unwrap()
                            .to_le_bytes()
                            .into()
                    })
                    .collect(),
                data.into(),
            )
            .unwrap()
        })
        .collect();

    assert!(
        evm_result_fields.next().is_none(),
        "There are only 3 field in EVM return value."
    );

    EvmNativeOutcome {
        is_success,
        output,
        logs,
    }
}

/// Serialize a number as a Move fungible asset type.
/// This is needed to directly call the EVM natives which
/// take `value` as a fungible asset.
fn serialize_fungible_asset_value(value: u64) -> Vec<u8> {
    // Fungible asset Move type is a struct with two fields:
    // 1. another struct with a single address field,
    // 2. a u256 value.
    let fungible_asset_layout = MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
        MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![MoveTypeLayout::Address])),
        MoveTypeLayout::U256,
    ]));

    Value::struct_(Struct::pack([
        Value::struct_(Struct::pack([Value::address(AccountAddress::ZERO)])),
        Value::u256(value.into()),
    ]))
    .simple_serialize(&fungible_asset_layout)
    .unwrap()
}
