use super::*;

/// How much L1 gas cost charging depletes the gas meter
const L1_GAS_COST: u64 = 9;

#[test]
fn test_treasury_charges_l1_and_l2_cost_to_sender_account_on_success() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = U256::from(123);
    ctx.deposit_eth(sender, mint_amount);

    // Transfer to receiver account
    let l1_cost = 1;
    // Set a gas limit higher than the cost of operation
    let l2_gas_limit = 50;
    let l2_gas_price = U256::from(1);
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.wrapping_shr(2);

    let outcome = ctx
        .transfer(
            receiver,
            transfer_amount,
            l1_cost,
            l2_gas_limit,
            l2_gas_price,
        )
        .expect("Transfer should succeed");
    assert!(outcome.vm_outcome.is_ok());

    let l2_cost = outcome
        .gas_used
        .saturating_mul(l2_gas_price.saturating_to());
    let expected_sender_balance = mint_amount - transfer_amount - U256::from(l1_cost + l2_cost);
    let sender_balance = ctx.get_balance(sender);
    assert_eq!(sender_balance, expected_sender_balance);

    let receiver_balance = ctx.get_balance(receiver);
    assert_eq!(receiver_balance, transfer_amount);
}

#[test]
fn test_treasury_charges_correct_l1_and_l2_cost_to_sender_account_on_user_error() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = U256::from(123);
    ctx.deposit_eth(sender, mint_amount);

    let l1_cost = 1;
    // Set a gas limit higher than the cost of operation
    let l2_gas_limit = 50;
    let l2_gas_price = U256::from(2);
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.saturating_add(U256::from(1));

    // Transfer to receiver account
    let outcome = ctx
        .transfer(
            receiver,
            transfer_amount,
            l1_cost,
            l2_gas_limit,
            l2_gas_price,
        )
        .unwrap();
    assert!(outcome.vm_outcome.is_err());

    let sender_balance = ctx.get_balance(sender);
    let l2_cost = outcome
        .gas_used
        .saturating_mul(l2_gas_price.saturating_to());
    let expected_sender_balance = mint_amount - U256::from(l1_cost + l2_cost);
    let receiver_balance = ctx.get_balance(receiver);

    assert_eq!(sender_balance, expected_sender_balance);

    assert_eq!(receiver_balance, U256::ZERO);
}

#[test]
fn test_very_low_gas_limit_makes_tx_invalid() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = U256::from(123);
    ctx.deposit_eth(sender, mint_amount);

    let l1_cost = 1;
    let l2_gas_price = U256::from(2);
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.wrapping_shr(1);

    // Set a gas limit lower than the cost of operation, but still enough to pay L1 costs
    let l2_gas_limit = L1_GAS_COST;
    let outcome = ctx.transfer(
        receiver,
        transfer_amount,
        l1_cost,
        l2_gas_limit,
        l2_gas_price,
    );
    let err = outcome.unwrap_err();
    assert!(
        matches!(
            err,
            moved_shared::error::Error::InvalidTransaction(
                moved_shared::error::InvalidTransactionCause::InsufficientIntrinsicGas
            )
        ),
        "Unexpected err {err:?}"
    );

    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);

    // In this case no fees are paid
    assert_eq!(sender_balance, mint_amount);
    assert_eq!(receiver_balance, U256::ZERO);
}

#[test]
fn test_low_gas_limit_gets_charged_and_fails_the_tx() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = U256::from(123);
    ctx.deposit_eth(sender, mint_amount);

    let l1_cost = 1;
    let l2_gas_price = U256::from(2);
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.wrapping_shr(1);

    // Just enough for passing verification
    let l2_gas_limit = L1_GAS_COST + 3;
    let outcome = ctx.transfer(
        receiver,
        transfer_amount,
        l1_cost,
        l2_gas_limit,
        l2_gas_price,
    );

    let l2_cost = l2_gas_limit.saturating_mul(l2_gas_price.saturating_to());
    let expected_sender_balance = mint_amount - U256::from(l1_cost + l2_cost);
    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);

    // A higher gas limit that can include L2 charges but not the actual transfer costs
    // successfully charges the sender account only up to the initial gas limit
    assert!(outcome.is_ok());
    assert_eq!(sender_balance, expected_sender_balance);
    assert_eq!(receiver_balance, U256::ZERO);
}
