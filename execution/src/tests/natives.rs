use super::*;

#[test]
fn test_execute_natives_contract() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("natives");

    // Call entry function to run the internal native hashing methods
    ctx.execute(&module_id, "hashing", vec![]);
}

#[test]
fn test_execute_tables_contract() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("tables");

    let move_address = EVM_ADDRESS.to_move_address();
    let signer_arg = MoveValue::Signer(move_address);
    let tx = utils::create_test_tx(
        &mut ctx.signer,
        &module_id,
        "make_test_tables",
        vec![bcs::to_bytes(&signer_arg).unwrap()],
    );
    let outcome = ctx.execute_tx(&TestTransaction::new(tx)).unwrap();
    let table_change_set = outcome.changes.move_vm.tables;

    // tables.move creates 11 new tables and makes 11 changes
    const TABLE_CHANGE_SET_NEW_TABLES_LEN: usize = 11;
    const TABLE_CHANGE_SET_CHANGES_LEN: usize = 11;

    assert_eq!(
        table_change_set.new_tables.len(),
        TABLE_CHANGE_SET_NEW_TABLES_LEN
    );
    assert_eq!(table_change_set.changes.len(), TABLE_CHANGE_SET_CHANGES_LEN);
}
