use super::*;

#[test]
fn test_execute_signer_struct_contract() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("signer_struct");

    // Call main function with correct signer
    let move_address = EVM_ADDRESS.to_move_address();
    let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(vec![
        MoveValue::Signer(move_address),
    ])]));
    ctx.execute(&module_id, "main", vec![&input_arg]);

    // Call main function with incorrect signer (get an error)
    let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(vec![
        MoveValue::Signer(AccountAddress::new([0x11; 32])),
    ])]));
    let err = ctx.execute_err(&module_id, "main", vec![&input_arg]);
    let err_message = err.to_string();
    assert_eq!(err_message, "Signer does not match transaction signature");
}

#[test]
fn test_recursive_struct() {
    // This test intentionally modifies a module to have a cycle in a struct definition
    // then tries to deploy it. The MoveVM returns an error in this case.

    // Load a real module
    let module_name = "signer_struct";
    let move_address = EVM_ADDRESS.to_move_address();
    let mut module_bytes = ModuleCompileJob::new(module_name, &move_address)
        .compile()
        .unwrap();
    let mut compiled_module = CompiledModule::deserialize(&module_bytes).unwrap();

    // Modify to include a recursive struct (it has one field which has type
    // equal to itself).
    let struct_name: Identifier = "RecursiveStruct".parse().unwrap();
    let struct_name_index = IdentifierIndex::new(compiled_module.identifiers.len() as u16);
    compiled_module.identifiers.push(struct_name);
    let struct_handle_index = StructHandleIndex::new(compiled_module.struct_handles.len() as u16);
    let struct_handle = StructHandle {
        module: ModuleHandleIndex::new(0),
        name: struct_name_index,
        abilities: AbilitySet::EMPTY,
        type_parameters: Vec::new(),
    };
    compiled_module.struct_handles.push(struct_handle);
    let struct_def = StructDefinition {
        struct_handle: struct_handle_index,
        field_information: StructFieldInformation::Declared(vec![FieldDefinition {
            name: struct_name_index,
            signature: TypeSignature(SignatureToken::Struct(struct_handle_index)),
        }]),
    };
    compiled_module.struct_defs.push(struct_def);
    *compiled_module
        .signatures
        .first_mut()
        .unwrap()
        .0
        .first_mut()
        .unwrap() = SignatureToken::Struct(struct_handle_index);

    // Re-serialize the new module
    module_bytes.clear();
    compiled_module.serialize(&mut module_bytes).unwrap();

    // Attempt to deploy the module, but get an error.
    let mut ctx = TestContext::new();
    // Deploy some other contract to ensure the state is properly initialized.
    ctx.deploy_contract("natives");
    let tx_data = module_bytes_to_tx_data(module_bytes);
    let (tx_hash, tx) = create_transaction(&mut ctx.signer, TxKind::Create, tx_data);
    let transaction = TestTransaction::new(tx, tx_hash);
    let err = ctx.execute_tx(&transaction).unwrap();
    assert!(format!("{err:?}").contains("RECURSIVE_STRUCT_DEFINITION"));
}

#[test]
fn test_deeply_nested_type() {
    // This test intentionally modifies a module to include a type
    // which is very deeply nested (Option<Option<Option<...>>>).
    // Then the test tries to deploy the module, and it fails due to
    // the Move recursion limit.

    // Load a real module
    let module_name = "signer_struct";
    let move_address = EVM_ADDRESS.to_move_address();
    let mut module_bytes = ModuleCompileJob::new(module_name, &move_address)
        .compile()
        .unwrap();
    let mut compiled_module = CompiledModule::deserialize(&module_bytes).unwrap();

    // Define a procedure which wraps the argument to the function `main` in an
    // additional `Option`, e.g. `Option<signer>` -> `Option<Option<Signer>>`.
    fn wrap_with_option(compiled_module: &mut CompiledModule, module_bytes: &mut Vec<u8>) {
        let signature = compiled_module.signatures.first_mut().unwrap();
        let inner = signature.0.clone();
        signature.0 = vec![SignatureToken::StructInstantiation(
            StructHandleIndex(0),
            inner,
        )];

        // Re-serialize the new module
        module_bytes.clear();
        compiled_module.serialize(module_bytes).unwrap();
    }

    // This function does the same thing as `wrap_with_option` except it
    // acts directly on the module bytes instead of on the `CompiledModule`
    // data type. This allows us to continue wrapping with Option even once
    // the module serialization would fail due to the recursion limit.
    fn byte_level_wrap_with_option(module_bytes: &[u8]) -> Vec<u8> {
        // Helper function for this procedure
        fn update_byte(x: u8) -> (u8, u8) {
            let (y, overflow) = x.overflowing_add(3);
            if overflow { (y + 128, 1) } else { (y, 0) }
        }

        let mut result = Vec::with_capacity(module_bytes.len() + 3);

        // Copy first 20 bytes
        for b in &module_bytes[0..20] {
            result.push(*b);
        }

        // Update next 2 bytes
        let (x, y) = update_byte(module_bytes[20]);
        result.push(x);
        result.push(module_bytes[21] + y);

        // Copy next byte
        result.push(module_bytes[22]);

        // Update next 2 bytes
        let (x, y) = update_byte(module_bytes[23]);
        result.push(x);
        result.push(module_bytes[24] + y);

        // Copy next 2 bytes
        result.push(module_bytes[25]);
        result.push(module_bytes[26]);

        // Update next 2 bytes
        let (x, y) = update_byte(module_bytes[27]);
        result.push(x);
        result.push(module_bytes[28] + y);

        // Copy next 2 bytes
        result.push(module_bytes[29]);
        result.push(module_bytes[30]);

        // Update next 2 bytes
        let (x, y) = update_byte(module_bytes[31]);
        result.push(x);
        result.push(module_bytes[32] + y);

        // Copy next 2 bytes
        result.push(module_bytes[33]);
        result.push(module_bytes[34]);

        // Update next 2 bytes
        let (x, y) = update_byte(module_bytes[35]);
        result.push(x);
        result.push(module_bytes[36] + y);

        // Copy next 17 bytes
        for b in &module_bytes[37..54] {
            result.push(*b);
        }

        // Push 3 new bytes
        result.push(1);
        result.push(11);
        result.push(0);

        // Copy remaining bytes
        for b in &module_bytes[54..] {
            result.push(*b);
        }

        result
    }

    // Run the `wrap_with_option` procedure many times to make a deep nesting
    // of `Option<Option<Option<...>>>`.
    for _ in 0..51 {
        wrap_with_option(&mut compiled_module, &mut module_bytes);
    }

    let mut computed_module_bytes = module_bytes.clone();

    // Continue wrapping up to the recursion limit.
    // Also now also act on a separate copy of the module bytes directly
    // and validate the changes are identical. We couldn't use the byte-level
    // procedure on iterations 0 to 50 because the byte sequence is a little
    // different for some reason.
    for _ in 51..254 {
        wrap_with_option(&mut compiled_module, &mut module_bytes);

        computed_module_bytes = byte_level_wrap_with_option(&computed_module_bytes);

        assert_eq!(computed_module_bytes, module_bytes);
    }

    // Do one extra iteration beyond the serialization recursion limit
    module_bytes = byte_level_wrap_with_option(&computed_module_bytes);

    // Try to deploy the module
    let mut ctx = TestContext::new();
    let tx_data = module_bytes_to_tx_data(module_bytes);
    let (tx_hash, tx) = create_transaction(&mut ctx.signer, TxKind::Create, tx_data);
    let transaction = TestTransaction::new(tx, tx_hash);
    let err = ctx.execute_tx(&transaction).unwrap();
    // The deployment fails because the Aptos code refuses to deserialize
    // the module with too deep recursion.
    assert!(
        format!("{err:?}").contains("Maximum recursion depth reached"),
        "Actual error: {err:?}"
    );
}
