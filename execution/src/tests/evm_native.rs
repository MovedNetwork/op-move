use {
    crate::{
        CanonicalExecutionInput, execute_transaction,
        tests::{ALT_EVM_ADDRESS, EVM_ADDRESS, *},
        transaction::TransactionData,
    },
    alloy::{
        primitives::utils::parse_ether,
        providers::{self, network::AnyNetwork},
        sol,
    },
    aptos_types::transaction::EntryFunction,
    move_core_types::{ident_str, language_storage::ModuleId, value::MoveValue},
    move_vm_types::{value_serde::ValueSerDeContext, values::Value},
    moved_evm_ext::{
        CODE_LAYOUT, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE, state::InMemoryStorageTrieRepository,
    },
    moved_shared::primitives::{ToEthAddress, ToMoveAddress, ToMoveU256},
    moved_state::{InMemoryState, State},
    revm::primitives::{TxKind, U256},
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
    let mut ctx = TestContext::new();
    let erc20_move_interface = ctx.deploy_contract("erc20_interface");

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
    let outcome = ctx.evm_quick_create(deploy.calldata().to_vec());

    assert!(outcome.is_success, "Contract deploy must succeed");

    // The ERC-20 contract produces a log because it minted some tokens.
    // We can use this log to get the address of the newly deployed contract.
    let contract_address = outcome.logs[0].address;
    let deployed_contract = ERC20::new(contract_address, &provider);
    let contract_move_address = contract_address.to_move_address();

    // -------- Transfer ERC-20 tokens
    let transfer_amount = parse_ether("0.35").unwrap();
    let user_address = EVM_ADDRESS.to_move_address();
    let signer_arg = MoveValue::Signer(user_address);
    let to_arg = MoveValue::Address(contract_move_address);
    let transfer_call = deployed_contract.transfer(ALT_EVM_ADDRESS, transfer_amount);
    let data_input_arg = Value::vector_u8(transfer_call.calldata().clone());
    let entry_fn = EntryFunction::new(
        ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into()),
        ident_str!("entry_evm_call").into(),
        Vec::new(),
        vec![
            bcs::to_bytes(&signer_arg).unwrap(),
            bcs::to_bytes(&to_arg).unwrap(),
            ValueSerDeContext::new()
                .serialize(&data_input_arg, &CODE_LAYOUT)
                .unwrap()
                .unwrap(),
        ],
    );
    let (tx_hash, tx) = create_transaction(
        &mut ctx.signer,
        TxKind::Call(EVM_NATIVE_ADDRESS.to_eth_address()),
        bcs::to_bytes(&TransactionData::EntryFunction(entry_fn)).unwrap(),
    );

    let transaction = TestTransaction::new(tx, tx_hash);
    let outcome = ctx.execute_tx(&transaction).unwrap();
    outcome.vm_outcome.unwrap();
    ctx.state.apply(outcome.changes.move_vm).unwrap();
    ctx.evm_storage.apply(outcome.changes.evm).unwrap();

    // -------- Validate ERC-20 balances
    let sender_balance_call = deployed_contract.balanceOf(EVM_ADDRESS).calldata().to_vec();
    let receiver_balance_call = deployed_contract
        .balanceOf(ALT_EVM_ADDRESS)
        .calldata()
        .to_vec();
    let sender_balance = balance_of(&ctx, contract_move_address, sender_balance_call.clone());
    let receiver_balance = balance_of(&ctx, contract_move_address, receiver_balance_call.clone());

    assert_eq!(sender_balance, mint_amount - transfer_amount);
    assert_eq!(receiver_balance, transfer_amount);

    // -------- Transfer ERC-20 tokens (Move interface this time)
    let token_address_arg = MoveValue::Address(contract_move_address);
    let to_arg = MoveValue::Address(ALT_EVM_ADDRESS.to_move_address());
    let amount_arg = MoveValue::U256(transfer_amount.to_move_u256());
    ctx.execute(
        &erc20_move_interface,
        "erc20_transfer",
        vec![&token_address_arg, &signer_arg, &to_arg, &amount_arg],
    );

    // -------- Validate ERC-20 balances (again)
    let sender_balance = balance_of(&ctx, contract_move_address, sender_balance_call);
    let receiver_balance = balance_of(&ctx, contract_move_address, receiver_balance_call);

    assert_eq!(
        sender_balance,
        mint_amount - transfer_amount - transfer_amount
    );
    assert_eq!(receiver_balance, transfer_amount + transfer_amount);
}

#[test]
fn test_solidity_fixed_bytes() {
    let mut ctx = TestContext::new();
    let contract = ctx.deploy_contract("solidity_fixed_bytes");

    let mut call_contract =
        |input: Vec<u8>, state: &InMemoryState, evm_storage: &InMemoryStorageTrieRepository| {
            let arg = MoveValue::vector_u8(input);
            let entry_fn = EntryFunction::new(
                contract.clone(),
                ident_str!("encode_fixed_bytes").into(),
                Vec::new(),
                vec![bcs::to_bytes(&arg).unwrap()],
            );
            let (tx_hash, tx) = create_transaction(
                &mut ctx.signer,
                TxKind::Call(EVM_ADDRESS),
                bcs::to_bytes(&TransactionData::EntryFunction(entry_fn)).unwrap(),
            );
            let tx = tx.into_canonical().unwrap();
            let input = CanonicalExecutionInput {
                tx: &tx,
                tx_hash: &tx_hash,
                state: state.resolver(),
                storage_trie: evm_storage,
                genesis_config: &ctx.genesis_config,
                l1_cost: 0,
                l2_fee: U256::ZERO,
                l2_input: (u64::MAX, U256::ZERO).into(),
                base_token: &(),
                block_header: HeaderForExecution::default(),
            };
            execute_transaction(input.into()).unwrap()
        };

    // Calling with empty bytes is an error
    let outcome = call_contract(Vec::new(), &ctx.state, &ctx.evm_storage);
    outcome.vm_outcome.unwrap_err();
    ctx.state.apply(outcome.changes.move_vm).unwrap();
    ctx.evm_storage.apply(outcome.changes.evm).unwrap();

    // Calling with bytes longer than 32 is an error
    let outcome = call_contract(vec![0x88; 33], &ctx.state, &ctx.evm_storage);
    outcome.vm_outcome.unwrap_err();
    ctx.state.apply(outcome.changes.move_vm).unwrap();
    ctx.evm_storage.apply(outcome.changes.evm).unwrap();

    // Calling with any length between 1 and 32 (inclusive) works
    for n in 1..=32 {
        let outcome = call_contract(vec![0x88; n], &ctx.state, &ctx.evm_storage);
        outcome.vm_outcome.unwrap();
        ctx.state.apply(outcome.changes.move_vm).unwrap();
        ctx.evm_storage.apply(outcome.changes.evm).unwrap();
    }
}

fn balance_of(ctx: &TestContext, contract_address: AccountAddress, calldata: Vec<u8>) -> U256 {
    let outcome = ctx.evm_quick_call(EVM_NATIVE_ADDRESS, contract_address, calldata);
    U256::from_be_slice(&outcome.output)
}
