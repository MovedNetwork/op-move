use {
    crate::tests::test_context::TestContext,
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        eips::BlockNumberOrTag,
        network::TxSignerSync,
        primitives::{Address, TxKind, U256},
        signers::local::PrivateKeySigner,
    },
    aptos_types::transaction::EntryFunction,
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        language_storage::{ModuleId, StructTag},
        value::MoveValue,
    },
    umi_api::schema::mv::{
        IdentifierWrapper, MoveStructTag, MoveType, TableHandle, TableItemRequest,
    },
    umi_blockchain::receipt::TransactionReceipt,
    umi_execution::transaction::TransactionData,
    umi_shared::primitives::ToMoveAddress,
};

#[tokio::test]
async fn test_mv_get_table_item() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        let signer = PrivateKeySigner::from_slice(&[0xaa; 32]).unwrap();
        let address = signer.address();

        deploy_tables_contract(&mut ctx, &signer).await;
        call_tables_contract(&mut ctx, &signer).await;

        let struct_tag = StructTag {
            address: address.to_move_address(),
            module: ident_str!("tables").into(),
            name: ident_str!("TestTables").into(),
            type_args: Vec::new(),
        };
        let resource = ctx
            .mv_get_resource(address, &struct_tag, BlockNumberOrTag::Latest)
            .await
            .unwrap();

        // Try getting a value from the bool table
        let table_name = IdentifierWrapper(ident_str!("bool_table").into());
        let bool_table = resource.data.0.get(&table_name).unwrap()["handle"]
            .as_str()
            .unwrap();
        let handle = TableHandle(AccountAddress::from_str_strict(bool_table).unwrap());
        let request = TableItemRequest {
            key_type: MoveType::Bool,
            value_type: MoveType::Bool,
            key: serde_json::Value::Bool(true),
        };
        let table_item = ctx
            .mv_get_table_item(&handle, request, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(table_item, serde_json::Value::Bool(true));

        // Try getting a value from the vector<string> table
        let table_name = IdentifierWrapper(ident_str!("vector_string_table").into());
        let table = resource.data.0.get(&table_name).unwrap()["handle"]
            .as_str()
            .unwrap();
        let handle = TableHandle(AccountAddress::from_str_strict(table).unwrap());
        let string_type = MoveType::Struct(MoveStructTag::new(
            AccountAddress::ONE.into(),
            IdentifierWrapper(ident_str!("string").into()),
            IdentifierWrapper(ident_str!("String").into()),
            Vec::new(),
        ));
        let request = TableItemRequest {
            key_type: MoveType::Vector {
                items: Box::new(string_type.clone()),
            },
            value_type: MoveType::Vector {
                items: Box::new(string_type.clone()),
            },
            key: serde_json::Value::Array(vec![
                serde_json::Value::String("abc".into()),
                serde_json::Value::String("abc".into()),
            ]),
        };
        let table_item = ctx
            .mv_get_table_item(&handle, request.clone(), BlockNumberOrTag::Latest)
            .await
            .unwrap();

        // The key and value are equal for this table
        assert_eq!(table_item, request.key);

        ctx.shutdown().await;
        Ok(())
    })
    .await
}

async fn call_tables_contract(
    ctx: &mut TestContext<'static>,
    signer: &PrivateKeySigner,
) -> TransactionReceipt {
    let address = signer.address();
    let input = TransactionData::EntryFunction(EntryFunction::new(
        ModuleId::new(address.to_move_address(), ident_str!("tables").into()),
        ident_str!("make_test_tables").into(),
        Vec::new(),
        vec![bcs::to_bytes(&MoveValue::Signer(address.to_move_address())).unwrap()],
    ));
    let mut tx = TxEip1559 {
        chain_id: ctx.genesis_config.chain_id,
        nonce: 1,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Call(address),
        value: U256::ZERO,
        access_list: Default::default(),
        input: input.to_bytes().unwrap().into(),
    };
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    let signed_tx = TxEnvelope::Eip1559(tx.into_signed(signature));
    ctx.execute_transaction(signed_tx).await.unwrap()
}

async fn deploy_tables_contract(
    ctx: &mut TestContext<'static>,
    signer: &PrivateKeySigner,
) -> TransactionReceipt {
    let address = signer.address();
    let bytecode = create_move_tables_contract_bytecode(address);
    let mut tx = TxEip1559 {
        chain_id: ctx.genesis_config.chain_id,
        nonce: 0,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Create,
        value: U256::ZERO,
        access_list: Default::default(),
        input: bytecode.into(),
    };
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    let signed_tx = TxEnvelope::Eip1559(tx.into_signed(signature));
    ctx.execute_transaction(signed_tx).await.unwrap()
}

fn create_move_tables_contract_bytecode(address: Address) -> Vec<u8> {
    let bytecode_hex = std::fs::read_to_string("src/tests/res/tables.hex").unwrap();
    let bytecode = hex::decode(bytecode_hex.trim()).unwrap();
    crate::tests::integration::set_module_address(bytecode, address)
}
