use {
    super::*,
    alloy::{network::AnyNetwork, primitives::U256, providers, sol_types::SolValue},
    move_binary_format::errors::VMError,
    umi_evm_ext::EvmNativeOutcome,
};

mod interface;
mod move_impl;
mod tx_impl;

#[test]
fn test_erc20_failed_transfer() {
    use {interface::Erc20Token, move_impl::MoveImpl};
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    let sender_balance = MoveImpl::balance_of(&ctx, token_address, EVM_ADDRESS);
    let receiver_balance = MoveImpl::balance_of(&ctx, token_address, ALT_EVM_ADDRESS);

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

    let new_sender_balance = MoveImpl::balance_of(&ctx, token_address, EVM_ADDRESS);
    let new_receiver_balance = MoveImpl::balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(new_sender_balance, sender_balance);
    assert_eq!(new_receiver_balance, receiver_balance);
}

#[test]
fn test_erc20_transfer() {
    generic_test_erc20_transfer::<move_impl::MoveImpl>();
    generic_test_erc20_transfer::<tx_impl::TxImpl>();
}

fn generic_test_erc20_transfer<E: interface::Erc20Token>() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    let initial_sender_balance = E::balance_of(&ctx, token_address, EVM_ADDRESS);
    let initial_receiver_balance = E::balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(initial_sender_balance, mint_amount);
    assert_eq!(initial_receiver_balance, U256::ZERO);

    let transfer_amount = U256::from(123u64);
    E::transfer(
        &mut ctx,
        EVM_ADDRESS,
        token_address,
        ALT_EVM_ADDRESS,
        transfer_amount,
    );
    let sender_balance = E::balance_of(&ctx, token_address, EVM_ADDRESS);
    let receiver_balance = E::balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(sender_balance, initial_sender_balance - transfer_amount);
    assert_eq!(receiver_balance, transfer_amount);
}

#[test]
fn test_erc20_transfer_from() {
    generic_test_erc20_transfer_from::<move_impl::MoveImpl>();
    generic_test_erc20_transfer_from::<tx_impl::TxImpl>();
}

fn generic_test_erc20_transfer_from<E: interface::Erc20Token>() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let approve_amount = U256::from(246u64);
    let transfer_amount = U256::from(123u64);

    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    // ERC20 are minted to sender account
    let initial_sender_balance = E::balance_of(&ctx, token_address, EVM_ADDRESS);
    let initial_receiver_balance = E::balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
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

    let receiver_allowance = E::allowance(&ctx, token_address, EVM_ADDRESS, ALT_EVM_ADDRESS);
    assert_eq!(receiver_allowance, U256::ZERO);

    let outcome = E::approve(
        &mut ctx,
        EVM_ADDRESS,
        token_address,
        ALT_EVM_ADDRESS,
        approve_amount,
    );
    assert!(outcome.is_success);

    let receiver_allowance = E::allowance(&ctx, token_address, EVM_ADDRESS, ALT_EVM_ADDRESS);
    assert_eq!(receiver_allowance, approve_amount);

    // trying to send a sum less than total allowance succeeds
    let outcome = E::transfer_from(
        &mut ctx,
        ALT_EVM_ADDRESS,
        token_address,
        EVM_ADDRESS,
        ALT_EVM_ADDRESS,
        transfer_amount,
    );
    assert!(outcome.is_success);

    let sender_balance = E::balance_of(&ctx, token_address, EVM_ADDRESS);
    let receiver_balance = E::balance_of(&ctx, token_address, ALT_EVM_ADDRESS);
    assert_eq!(sender_balance, initial_sender_balance - transfer_amount);
    assert_eq!(receiver_balance, initial_receiver_balance + transfer_amount);

    // the allowance is decreased by the transfer amount
    let receiver_allowance = E::allowance(&ctx, token_address, EVM_ADDRESS, ALT_EVM_ADDRESS);
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
    generic_test_erc20_metadata::<move_impl::MoveImpl>();
    generic_test_erc20_metadata::<tx_impl::TxImpl>();
}

fn generic_test_erc20_metadata<E: interface::Erc20Token>() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(1234u64);
    let token_address = deploy_mock_erc20(&mut ctx, mint_amount);

    let total_supply = E::total_supply(&ctx, token_address);
    assert_eq!(total_supply, mint_amount);

    let name = E::name(&ctx, token_address);
    assert_eq!(name, "Gold");

    let symbol = E::symbol(&ctx, token_address);
    assert_eq!(symbol, "AU");

    let decimals = E::decimals(&ctx, token_address);
    // As it wasn't set during creation, should be 18 by default
    assert_eq!(decimals, 18u8);
}

fn deploy_mock_erc20(ctx: &mut TestContext, mint_amount: U256) -> Address {
    use umi_evm_ext::erc20::abi_bindings::Erc20;

    // We just need a mock to get proper calldata
    let mock_provider = providers::builder::<AnyNetwork>()
        .with_recommended_fillers()
        .on_http("http://localhost:1234".parse().unwrap());
    let deploy = Erc20::deploy_builder(
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
