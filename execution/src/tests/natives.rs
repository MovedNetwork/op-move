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

    let vm = create_move_vm().unwrap();
    let traversal_storage = TraversalStorage::new();

    let mut session = create_vm_session(
        &vm,
        ctx.state.resolver(),
        SessionId::default(),
        &ctx.evm_storage,
        &(),
    );
    let mut traversal_context = TraversalContext::new(&traversal_storage);

    let move_address = EVM_ADDRESS.to_move_address();
    let signer_arg = MoveValue::Signer(move_address);
    let entry_fn = EntryFunction::new(
        module_id,
        Identifier::new("make_test_tables").unwrap(),
        Vec::new(),
        vec![bcs::to_bytes(&signer_arg).unwrap()],
    );
    let (module_id, function_name, ty_args, args) = entry_fn.into_inner();

    session
        .execute_entry_function(
            &module_id,
            &function_name,
            ty_args,
            args,
            &mut UnmeteredGasMeter,
            &mut traversal_context,
        )
        .unwrap();

    let (_change_set, mut extensions) = session.finish_with_extensions().unwrap();
    let table_change_set = extensions
        .remove::<NativeTableContext>()
        .into_change_set()
        .unwrap();

    // tables.move creates 11 new tables and makes 11 changes
    const TABLE_CHANGE_SET_NEW_TABLES_LEN: usize = 11;
    const TABLE_CHANGE_SET_CHANGES_LEN: usize = 11;

    assert_eq!(
        table_change_set.new_tables.len(),
        TABLE_CHANGE_SET_NEW_TABLES_LEN
    );
    assert_eq!(table_change_set.changes.len(), TABLE_CHANGE_SET_CHANGES_LEN);
}
