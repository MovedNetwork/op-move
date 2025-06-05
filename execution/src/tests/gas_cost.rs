use super::*;

/// How much L1 gas cost charging depletes the gas meter
const L1_GAS_COST: u64 = 10_000;

#[test]
fn test_treasury_charges_l1_and_l2_cost_to_sender_account_on_success() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = one_eth() * U256::from(100);
    ctx.deposit_eth(sender, mint_amount);

    // Transfer to receiver account
    let l1_cost = 1;
    // Set a gas limit higher than the cost of operation
    let l2_gas_limit = 100_000;
    let l2_gas_price = U256::from(10).pow(U256::from(9)); // 1 Gwei
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
    outcome.vm_outcome.unwrap();

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
    let mint_amount = one_eth();
    ctx.deposit_eth(sender, mint_amount);

    let l1_cost = 1;
    // Set a gas limit higher than the cost of operation
    let l2_gas_limit = 100_000;
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
    let mint_amount = one_eth();
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
            umi_shared::error::Error::InvalidTransaction(
                umi_shared::error::InvalidTransactionCause::InsufficientIntrinsicGas
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
    let mint_amount = one_eth();
    ctx.deposit_eth(sender, mint_amount);

    let l1_cost = 1;
    let l2_gas_price = U256::from(2);
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.wrapping_shr(1);

    // Just enough for passing verification
    let l2_gas_limit = 2 * L1_GAS_COST;
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
    outcome.unwrap().vm_outcome.unwrap_err();
    assert_eq!(sender_balance, expected_sender_balance);
    assert_eq!(receiver_balance, U256::ZERO);
}

#[test]
fn test_storage_update_cost() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("hello_strings");

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = one_eth();
    ctx.deposit_eth(sender, mint_amount);

    let gas_limit = 50_000;
    // Choose gas price equal to 1 for convenience in checking how much the gas changes.
    let gas_price = U256::ONE;

    // Create the resource
    let text = "Hello, world";
    let signer_arg = MoveValue::Signer(ctx.move_address);
    let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(
        text.bytes().map(MoveValue::U8).collect(),
    )]));
    ctx.execute(&module_id, "publish", vec![&signer_arg, &input_arg]);

    // Do nothing to modify it (stays the same size)
    let address_arg = MoveValue::Address(ctx.move_address);
    let base_outcome = ctx.execute_with_fee(
        &module_id,
        "update",
        vec![&address_arg, &input_arg],
        gas_price,
        gas_limit,
    );

    let storage_per_byte_cost: u64 = ctx
        .genesis_config
        .gas_costs
        .vm
        .txn
        .storage_fee_per_state_byte
        .into();

    // Increase the size of the resource by a number of bytes
    for size_increase in 1..10 {
        let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(
            text.bytes()
                .chain(std::iter::repeat_n(b'!', size_increase))
                .map(MoveValue::U8)
                .collect(),
        )]));
        let outcome = ctx.execute_with_fee(
            &module_id,
            "update",
            vec![&address_arg, &input_arg],
            gas_price,
            gas_limit,
        );
        let size_increase = size_increase as u64;
        // The two transactions are identical except for larger input argument causing the new bytes of stored data.
        // Therefore the only difference in gas cost should be from the larger transaction size and the new stored bytes.
        // We calculate the new stored bytes difference and check the observed difference is at least that big.
        assert!(
            (outcome.gas_used - base_outcome.gas_used) >= (storage_per_byte_cost * size_increase)
        );
    }
}

fn one_eth() -> U256 {
    U256::from(10).pow(U256::from(18))
}
