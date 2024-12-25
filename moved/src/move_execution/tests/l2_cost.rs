use super::*;

#[test]
fn test_treasury_charges_l1_cost_to_sender_account_on_success() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = U256::from(123);
    ctx.deposit_eth(sender, mint_amount);

    // Transfer to receiver account
    let l2_cost = 1;
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.wrapping_shr(1);
    let outcome = ctx.transfer(receiver, transfer_amount, 0, l2_cost);
    outcome.vm_outcome.unwrap();

    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);
    assert_eq!(
        sender_balance,
        mint_amount - transfer_amount - U256::from(l2_cost)
    );
    assert_eq!(receiver_balance, transfer_amount);
}

#[test]
fn test_treasury_charges_l1_cost_to_sender_account_on_user_error() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = U256::from(123);
    ctx.deposit_eth(sender, mint_amount);

    // Transfer to receiver account
    let l2_cost = 1;
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.saturating_add(U256::from(1));
    let outcome = ctx.transfer(receiver, transfer_amount, 0, l2_cost);
    outcome.vm_outcome.unwrap_err();

    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);
    // let treasury_balance = ctx.get_balance(eth_treasury);
    assert_eq!(sender_balance, mint_amount - U256::from(l2_cost));
    assert_eq!(receiver_balance, U256::ZERO);
}
