use super::*;

/// Deposits can be made to the L2.
#[test]
fn test_deposit_tx() {
    let mut ctx = TestContext::new();

    let mint_amount = U256::from(0x56bc75e2d63100000_u128);
    let dest_account = address!("84a124e4ec6f0f9914b49dcc71669a8cac556ad6");

    // Example transaction take from end-to-end integration test
    let tx = DepositedTx {
        source_hash: B256::new(hex!("ad2cd5c72f8d6b25e4da049d76790993af597050965f2aee87e12f98f8c2427f")),
        from: address!("4a04a3191b7a44a99bfd3184f0d2c2c82b98b939"),
        to: address!("4200000000000000000000000000000000000007"),
        mint: U256::from(mint_amount),
        value: U256::from(mint_amount),
        gas: U64::from(0x77d2e_u64),
        is_system_tx: false,
        data: hex!("d764ad0b0001000000000000000000000000000000000000000000000000000000000000000000000000000000000000c8088d0362bb4ac757ca77e211c30503d39cef4800000000000000000000000042000000000000000000000000000000000000100000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000030d4000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000a41635f5fd00000000000000000000000084a124e4ec6f0f9914b49dcc71669a8cac556ad600000000000000000000000084a124e4ec6f0f9914b49dcc71669a8cac556ad60000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").into(),
    };
    let tx_hash = ExtendedTxEnvelope::DepositedTx(tx.clone()).compute_hash();
    let test_tx = TestTransaction::new(NormalizedExtendedTxEnvelope::DepositedTx(tx), tx_hash);

    let outcome = ctx.execute_tx(&test_tx).unwrap();
    outcome.vm_outcome.unwrap();
    ctx.state.apply(outcome.changes).unwrap();

    let balance = ctx.get_balance(dest_account);
    assert_eq!(balance, mint_amount);
}

#[test]
fn test_withdrawal_tx() {
    let mut ctx = TestContext::new();

    // 1. Deposit ETH to user
    let mint_amount = U256::from(123);
    ctx.deposit_eth(EVM_ADDRESS, mint_amount);

    let balance = ctx.get_balance(EVM_ADDRESS);
    assert_eq!(balance, mint_amount);

    // 2. Use script to withdraw
    let logs = ctx.run_script(
        "withdrawal_script",
        &[],
        vec![
            TransactionArgument::Address(EVM_ADDRESS.to_move_address()),
            TransactionArgument::U256(U256::from(mint_amount).to_move_u256()),
        ],
    );
    assert_eq!(ctx.get_balance(EVM_ADDRESS), U256::ZERO);
    assert!(
        logs.iter().any(|log| log.address.to_move_address()
            == address!("4200000000000000000000000000000000000007").to_move_address()),
        "Outcome must have logs from the L2CrossDomainMessenger contract"
    );
}

#[test]
fn test_eoa_base_token_transfer() {
    // Initialize state
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = U256::from(123);
    ctx.deposit_eth(sender, mint_amount);

    // Should fail when transfer is larger than account balance
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.saturating_add(U256::from(1));
    let outcome = ctx.transfer(receiver, transfer_amount, 0);
    outcome.vm_outcome.unwrap_err();

    // Should work with proper transfer
    let transfer_amount = mint_amount.wrapping_shr(1);
    let outcome = ctx.transfer(receiver, transfer_amount, 0);
    outcome.vm_outcome.unwrap();

    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);
    assert_eq!(sender_balance, mint_amount - transfer_amount);
    assert_eq!(receiver_balance, transfer_amount);
}
