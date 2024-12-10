use super::*;

#[test]
fn test_move_event_converts_to_eth_log_successfully() {
    let data = vec![0u8, 1, 2, 3];
    let type_tag = TypeTag::Struct(Box::new(StructTag {
        address: hex!("0000111122223333444455556666777788889999aaaabbbbccccddddeeeeffff").into(),
        module: Identifier::new("moved").unwrap(),
        name: Identifier::new("test").unwrap(),
        type_args: vec![],
    }));
    let event = ContractEvent::V2(ContractEventV2::new(type_tag, data));

    let actual_log = {
        let mut tmp = Vec::with_capacity(1);
        push_logs(&event, &mut tmp);
        tmp.pop().unwrap()
    };
    let expected_log = Log::new_unchecked(
        address!("6666777788889999aaaabbbbccccddddeeeeffff"),
        vec![keccak256(
            "0000111122223333444455556666777788889999aaaabbbbccccddddeeeeffff::moved::test",
        )],
        Bytes::from([0u8, 1, 2, 3]),
    );

    assert_eq!(actual_log, expected_log);
}

#[test]
fn test_transaction_replay_is_forbidden() {
    // Transaction replay is forbidden by the nonce checking.

    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("natives");

    // Use a transaction to call a function; this passes
    let (tx_hash, tx) = create_test_tx(&mut ctx.signer, &module_id, "hashing", vec![]);
    let transaction = TestTransaction::new(tx, tx_hash);
    let outcome = ctx.execute_tx(&transaction).unwrap();
    outcome.vm_outcome.unwrap();
    ctx.state.apply(outcome.changes).unwrap();

    // Send the same transaction again without state update; this fails with a nonce error
    let err = ctx.execute_tx(&transaction).unwrap_err();
    assert_eq!(err.to_string(), "Incorrect nonce: given=1 expected=2");
}

#[test]
fn test_transaction_incorrect_destination() {
    // If a transaction uses an EntryFunction to call a module
    // then that EntryFunction's address must match the to field
    // of the user's transaction.

    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("natives");

    ctx.execute(&module_id, "hashing", vec![]);

    // Try to call a function of that contract
    let entry_fn = EntryFunction::new(
        module_id,
        Identifier::new("hashing").unwrap(),
        Vec::new(),
        vec![],
    );
    let (tx_hash, tx) = create_transaction(
        &mut ctx.signer,
        TxKind::Call(Default::default()), // Wrong address!
        bcs::to_bytes(&TransactionData::EntryFunction(entry_fn)).unwrap(),
    );

    let transaction = TestTransaction::new(tx, tx_hash);
    let err = ctx.execute_tx(&transaction).unwrap_err();
    assert_eq!(err.to_string(), "tx.to must match payload module address");
}

#[test]
fn test_transaction_chain_id() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("natives");

    // Use a transaction to call a function but pass the wrong chain id
    let entry_fn = TransactionData::EntryFunction(EntryFunction::new(
        module_id,
        Identifier::new("hashing").unwrap(),
        Vec::new(),
        vec![],
    ));
    let mut tx = TxEip1559 {
        // Intentionally setting the wrong chain id
        chain_id: ctx.genesis_config.chain_id + 1,
        nonce: ctx.signer.nonce,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Call(EVM_ADDRESS),
        value: Default::default(),
        access_list: Default::default(),
        input: bcs::to_bytes(&entry_fn).unwrap().into(),
    };
    let signature = ctx.signer.inner.sign_transaction_sync(&mut tx).unwrap();
    let signed_tx = TxEnvelope::Eip1559(tx.into_signed(signature));
    let tx_hash = *signed_tx.tx_hash();
    let signed_tx = NormalizedExtendedTxEnvelope::Canonical(signed_tx.try_into().unwrap());

    let transaction = TestTransaction::new(signed_tx, tx_hash);
    let err = ctx.execute_tx(&transaction).unwrap_err();
    assert_eq!(err.to_string(), "Incorrect chain id");
}

#[test]
fn test_out_of_gas() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("natives");

    // Use a transaction to call a function but pass in too little gas
    let entry_fn = TransactionData::EntryFunction(EntryFunction::new(
        module_id,
        Identifier::new("hashing").unwrap(),
        Vec::new(),
        vec![],
    ));
    let mut tx = TxEip1559 {
        chain_id: ctx.genesis_config.chain_id,
        nonce: ctx.signer.nonce,
        // Intentionally pass a small amount of gas
        gas_limit: 1,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Call(EVM_ADDRESS),
        value: Default::default(),
        access_list: Default::default(),
        input: bcs::to_bytes(&entry_fn).unwrap().into(),
    };
    let signature = ctx.signer.inner.sign_transaction_sync(&mut tx).unwrap();
    let signed_tx = TxEnvelope::Eip1559(tx.into_signed(signature));
    let tx_hash = *signed_tx.tx_hash();
    let signed_tx = NormalizedExtendedTxEnvelope::Canonical(signed_tx.try_into().unwrap());

    let transaction = TestTransaction::new(signed_tx, tx_hash);
    let err = ctx.execute_tx(&transaction).unwrap_err();
    assert_eq!(err.to_string(), "Insufficient intrinsic gas");
}
