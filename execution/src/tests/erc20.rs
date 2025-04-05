use {
    super::*,
    alloy::{network::AnyNetwork, primitives::U256, providers, sol, sol_types::SolValue},
    move_binary_format::errors::VMError,
    moved_evm_ext::{EVM_NATIVE_ADDRESS, EvmNativeOutcome},
};

#[test]
fn test_erc20_failed_transfer() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    let sender_balance = balance_of(&ctx, token_address, EVM_ADDRESS);
    let receiver_balance = balance_of(&ctx, token_address, ALT_EVM_ADDRESS);

    // intentionally bigger than the initial mint amount (1234)
    let transfer_amount = U256::from(1250u64);
    let err = transfer_err(
        &ctx,
        EVM_ADDRESS,
        token_address,
        ALT_EVM_ADDRESS,
        transfer_amount,
    );
    assert!(err.to_string().contains("ABORTED"));

    let new_sender_balance = balance_of(&ctx, token_address, EVM_ADDRESS);
    let new_receiver_balance = balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(new_sender_balance, sender_balance);
    assert_eq!(new_receiver_balance, receiver_balance);
}

#[test]
fn test_erc20_transfer() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    let initial_sender_balance = balance_of(&ctx, token_address, EVM_ADDRESS);
    let initial_receiver_balance = balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(initial_sender_balance, mint_amount);
    assert_eq!(initial_receiver_balance, U256::ZERO);

    let transfer_amount = U256::from(123u64);
    transfer(
        &mut ctx,
        EVM_ADDRESS,
        token_address,
        ALT_EVM_ADDRESS,
        transfer_amount,
    );
    let sender_balance = balance_of(&ctx, token_address, EVM_ADDRESS);
    let receiver_balance = balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(sender_balance, initial_sender_balance - transfer_amount);
    assert_eq!(receiver_balance, transfer_amount);
}

#[test]
fn test_erc20_transfer_from() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let approve_amount = U256::from(246u64);
    let transfer_amount = U256::from(123u64);

    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    // ERC20 are minted to sender account
    let initial_sender_balance = balance_of(&ctx, token_address, EVM_ADDRESS);
    let initial_receiver_balance = balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(initial_sender_balance, mint_amount);
    assert_eq!(initial_receiver_balance, U256::ZERO);

    // transferFrom without allowance fails
    let err = transfer_from_err(
        &ctx,
        ALT_EVM_ADDRESS,
        token_address,
        EVM_ADDRESS,
        ALT_EVM_ADDRESS,
        transfer_amount,
    );

    assert!(err.to_string().contains("ABORTED"));

    let receiver_allowance = allowance(&ctx, token_address, EVM_ADDRESS, ALT_EVM_ADDRESS);
    assert_eq!(receiver_allowance, U256::ZERO);

    let outcome = approve(
        &mut ctx,
        EVM_ADDRESS,
        token_address,
        ALT_EVM_ADDRESS,
        approve_amount,
    );
    assert!(outcome.is_success);

    let receiver_allowance = allowance(&ctx, token_address, EVM_ADDRESS, ALT_EVM_ADDRESS);
    assert_eq!(receiver_allowance, approve_amount);

    // trying to send a sum less than total allowance succeeds
    let outcome = transfer_from(
        &mut ctx,
        ALT_EVM_ADDRESS,
        token_address,
        EVM_ADDRESS,
        ALT_EVM_ADDRESS,
        transfer_amount,
    );
    assert!(outcome.is_success);

    let sender_balance = balance_of(&ctx, token_address, EVM_ADDRESS);
    let receiver_balance = balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(sender_balance, initial_sender_balance - transfer_amount);
    assert_eq!(receiver_balance, initial_receiver_balance + transfer_amount);

    // the allowance is decreased by the transfer amount
    let receiver_allowance = allowance(&ctx, token_address, EVM_ADDRESS, ALT_EVM_ADDRESS);
    assert_eq!(receiver_allowance, approve_amount - transfer_amount);

    // trying to send a sum larger than current allowance fails, i.e. no partial transfers
    let err = transfer_from_err(
        &ctx,
        ALT_EVM_ADDRESS,
        token_address,
        EVM_ADDRESS,
        ALT_EVM_ADDRESS,
        approve_amount,
    );
    assert!(err.to_string().contains("ABORTED"));
}

#[test]
fn test_erc20_metadata() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    let total_supply = total_supply(&ctx, token_address);
    assert_eq!(total_supply, mint_amount);

    let name = name(&ctx, token_address);
    assert_eq!(name, "Gold");

    let symbol = symbol(&ctx, token_address);
    assert_eq!(symbol, "AU");

    let decimals = decimals(&ctx, token_address);
    // As it wasn't set during creation, should be 18 by default
    assert_eq!(decimals, 18u8);
}

fn deploy_mock_erc20(ctx: &mut TestContext, mint_amount: U256) -> Address {
    sol!(
        #[sol(rpc)]
        ERC20,
        "../server/src/tests/res/ERC20.json"
    );

    // We just need a mock to get proper calldata
    let mock_provider = providers::builder::<AnyNetwork>()
        .with_recommended_fillers()
        .on_http("http://localhost:1234".parse().unwrap());
    let deploy = ERC20::deploy_builder(
        &mock_provider,
        "Gold".into(),
        "AU".into(),
        EVM_ADDRESS,
        mint_amount,
    );

    let outcome = ctx.evm_quick_create(deploy.calldata().to_vec());

    // The ERC-20 contract produces a log because it minted some tokens.
    // We can use this log to get the address of the newly deployed contract.
    outcome.logs[0].address
}

fn balance_of(ctx: &TestContext, token_address: Address, account_address: Address) -> U256 {
    let outcome = ctx
        .quick_call(
            vec![
                MoveValue::Signer(EVM_NATIVE_ADDRESS),
                MoveValue::Address(token_address.to_move_address()),
                MoveValue::Address(account_address.to_move_address()),
            ],
            "erc20",
            "balance_of",
        )
        .0;
    U256::from_be_slice(&outcome.output)
}

fn transfer_err(
    ctx: &TestContext,
    caller_address: Address,
    token_address: Address,
    to_address: Address,
    transfer_amount: U256,
) -> VMError {
    ctx.quick_call_err(
        vec![
            MoveValue::Signer(caller_address.to_move_address()),
            MoveValue::Address(token_address.to_move_address()),
            MoveValue::Address(to_address.to_move_address()),
            MoveValue::U256(transfer_amount.to_move_u256()),
        ],
        "erc20",
        "transfer",
    )
}

fn transfer(
    ctx: &mut TestContext,
    caller_address: Address,
    token_address: Address,
    to_address: Address,
    transfer_amount: U256,
) -> EvmNativeOutcome {
    ctx.quick_send(
        vec![
            MoveValue::Signer(caller_address.to_move_address()),
            MoveValue::Address(token_address.to_move_address()),
            MoveValue::Address(to_address.to_move_address()),
            MoveValue::U256(transfer_amount.to_move_u256()),
        ],
        "erc20",
        "transfer",
    )
}

fn allowance(
    ctx: &TestContext,
    token_address: Address,
    owner_address: Address,
    spender_address: Address,
) -> U256 {
    let outcome = ctx
        .quick_call(
            vec![
                MoveValue::Signer(EVM_NATIVE_ADDRESS),
                MoveValue::Address(token_address.to_move_address()),
                MoveValue::Address(owner_address.to_move_address()),
                MoveValue::Address(spender_address.to_move_address()),
            ],
            "erc20",
            "allowance",
        )
        .0;
    U256::from_be_slice(&outcome.output)
}

fn approve(
    ctx: &mut TestContext,
    caller_address: Address,
    token_address: Address,
    spender_address: Address,
    approve_amount: U256,
) -> EvmNativeOutcome {
    ctx.quick_send(
        vec![
            MoveValue::Signer(caller_address.to_move_address()),
            MoveValue::Address(token_address.to_move_address()),
            MoveValue::Address(spender_address.to_move_address()),
            MoveValue::U256(approve_amount.to_move_u256()),
        ],
        "erc20",
        "approve",
    )
}

fn transfer_from(
    ctx: &mut TestContext,
    caller_address: Address,
    token_address: Address,
    from_address: Address,
    to_address: Address,
    transfer_amount: U256,
) -> EvmNativeOutcome {
    ctx.quick_send(
        vec![
            MoveValue::Signer(caller_address.to_move_address()),
            MoveValue::Address(token_address.to_move_address()),
            MoveValue::Address(from_address.to_move_address()),
            MoveValue::Address(to_address.to_move_address()),
            MoveValue::U256(transfer_amount.to_move_u256()),
        ],
        "erc20",
        "transfer_from",
    )
}

fn transfer_from_err(
    ctx: &TestContext,
    caller_address: Address,
    token_address: Address,
    from_address: Address,
    to_address: Address,
    transfer_amount: U256,
) -> VMError {
    ctx.quick_call_err(
        vec![
            MoveValue::Signer(caller_address.to_move_address()),
            MoveValue::Address(token_address.to_move_address()),
            MoveValue::Address(from_address.to_move_address()),
            MoveValue::Address(to_address.to_move_address()),
            MoveValue::U256(transfer_amount.to_move_u256()),
        ],
        "erc20",
        "transfer_from",
    )
}

fn total_supply(ctx: &TestContext, token_address: Address) -> U256 {
    let outcome = ctx
        .quick_call(
            vec![
                MoveValue::Signer(EVM_NATIVE_ADDRESS),
                MoveValue::Address(token_address.to_move_address()),
            ],
            "erc20",
            "total_supply",
        )
        .0;
    U256::from_be_slice(&outcome.output)
}

fn decimals(ctx: &TestContext, token_address: Address) -> u8 {
    let outcome = ctx
        .quick_call(
            vec![
                MoveValue::Signer(EVM_NATIVE_ADDRESS),
                MoveValue::Address(token_address.to_move_address()),
            ],
            "erc20",
            "decimals",
        )
        .0;
    let val = U256::from_be_slice(&outcome.output);
    val.as_limbs()[0] as u8
}

fn name(ctx: &TestContext, token_address: Address) -> String {
    let outcome = ctx
        .quick_call(
            vec![
                MoveValue::Signer(EVM_NATIVE_ADDRESS),
                MoveValue::Address(token_address.to_move_address()),
            ],
            "erc20",
            "name",
        )
        .0;
    let name = outcome.output;
    String::abi_decode(&name, true).unwrap()
}

fn symbol(ctx: &TestContext, token_address: Address) -> String {
    let outcome = ctx
        .quick_call(
            vec![
                MoveValue::Signer(EVM_NATIVE_ADDRESS),
                MoveValue::Address(token_address.to_move_address()),
            ],
            "erc20",
            "symbol",
        )
        .0;
    let symbol = outcome.output;
    String::abi_decode(&symbol, true).unwrap()
}
