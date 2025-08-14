use {
    crate::tests::test_context::TestContext,
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        eips::BlockNumberOrTag,
        network::TxSignerSync,
        primitives::{TxKind, U256},
        signers::local::PrivateKeySigner,
    },
    aptos_types::transaction::EntryFunction,
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        identifier::Identifier,
        language_storage::{ModuleId, StructTag},
        value::MoveValue,
    },
    umi_blockchain::receipt::TransactionReceipt,
    umi_execution::transaction::TransactionData,
    umi_shared::primitives::{ToEthAddress, ToMoveAddress},
};

#[tokio::test]
async fn test_mv_list_modules() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        // Listing API returns all modules in alphabetical order
        let genesis_framework_modules = get_all_modules(&ctx, AccountAddress::ONE).await;
        let mut expected_modules: Vec<Identifier> = umi_genesis::load_aptos_framework_snapshot()
            .compiled_modules()
            .into_iter()
            .filter_map(|m| {
                if m.self_addr() == &AccountAddress::ONE {
                    Some(m.self_id().name().into())
                } else {
                    None
                }
            })
            .collect();
        expected_modules.sort();
        assert_eq!(genesis_framework_modules, expected_modules);

        // Listing API can get up to 100 modules at a time
        let genesis_framework_modules = ctx
            .mv_list_modules(
                AccountAddress::ONE.to_eth_address(),
                None,
                Some(expected_modules.len() as u32),
                BlockNumberOrTag::Latest,
            )
            .await
            .unwrap();
        assert_eq!(&genesis_framework_modules, &expected_modules[0..100]);

        // Listing API handles the case where there are no modules
        let signer = PrivateKeySigner::from_slice(&[0xaa; 32]).unwrap();
        let address = signer.address();
        let account_modules = ctx
            .mv_list_modules(address, None, None, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(account_modules, Vec::new());

        // When a new module is deployed, the listing API sees it.
        let receipt = deploy_counter_contract(&mut ctx, &signer).await;
        let account_modules = ctx
            .mv_list_modules(address, None, None, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(account_modules, vec![Identifier::new("counter").unwrap()]);

        // Listing API gives previous result for older blocks
        let account_modules = ctx
            .mv_list_modules(
                address,
                None,
                None,
                BlockNumberOrTag::Number(receipt.inner.block_number.unwrap() - 1),
            )
            .await
            .unwrap();
        assert_eq!(account_modules, Vec::new());

        ctx.shutdown().await;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn test_mv_list_resources() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        // Listing API can get all the framework resources via pagination
        let genesis_framework_resources = get_all_resources(&ctx, AccountAddress::ONE).await;

        // Listing API can get up to 100 resources at a time.
        let resources = ctx
            .mv_list_resources(
                AccountAddress::ONE.to_eth_address(),
                None,
                Some(genesis_framework_resources.len() as u32),
                BlockNumberOrTag::Latest,
            )
            .await
            .unwrap();
        assert_eq!(&genesis_framework_resources[0..100], resources);

        // Listing API handles the case where there are no resources
        let signer = PrivateKeySigner::from_slice(&[0xaa; 32]).unwrap();
        let address = signer.address();
        let account_resources = ctx
            .mv_list_resources(address, None, None, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(account_resources, Vec::new());

        // When a new resource is created the listing API sees it
        let deploy_receipt = deploy_counter_contract(&mut ctx, &signer).await;
        let account_resources = ctx
            .mv_list_resources(address, None, None, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(account_resources, vec![account_struct_tag()]);

        let call_receipt = call_counter_publish(&mut ctx, &signer).await;
        let account_resources = ctx
            .mv_list_resources(address, None, None, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(
            account_resources,
            vec![
                account_struct_tag(),
                counter_struct_tag(address.to_move_address())
            ]
        );

        // Listing API shows previous values for older blocks
        let account_resources = ctx
            .mv_list_resources(
                address,
                None,
                None,
                BlockNumberOrTag::Number(deploy_receipt.inner.block_number.unwrap() - 1),
            )
            .await
            .unwrap();
        assert_eq!(account_resources, Vec::new());

        let account_resources = ctx
            .mv_list_resources(
                address,
                None,
                None,
                BlockNumberOrTag::Number(deploy_receipt.inner.block_number.unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(account_resources, vec![account_struct_tag()]);

        let account_resources = ctx
            .mv_list_resources(
                address,
                None,
                None,
                BlockNumberOrTag::Number(call_receipt.inner.block_number.unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(
            account_resources,
            vec![
                account_struct_tag(),
                counter_struct_tag(address.to_move_address())
            ]
        );

        ctx.shutdown().await;
        Ok(())
    })
    .await
}

async fn deploy_counter_contract(
    ctx: &mut TestContext<'static>,
    signer: &PrivateKeySigner,
) -> TransactionReceipt {
    let address = signer.address();
    let bytecode = crate::tests::integration::create_move_counter_contract_bytecode(address);
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

async fn call_counter_publish(
    ctx: &mut TestContext<'static>,
    signer: &PrivateKeySigner,
) -> TransactionReceipt {
    let address = signer.address();
    let input = TransactionData::EntryFunction(EntryFunction::new(
        ModuleId::new(address.to_move_address(), ident_str!("counter").into()),
        ident_str!("publish").into(),
        Vec::new(),
        vec![
            bcs::to_bytes(&MoveValue::Signer(address.to_move_address())).unwrap(),
            bcs::to_bytes(&MoveValue::U64(7)).unwrap(),
        ],
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

async fn get_all_modules(ctx: &TestContext<'static>, address: AccountAddress) -> Vec<Identifier> {
    let address = address.to_eth_address();
    let mut all_modules = Vec::new();
    loop {
        let mut modules = ctx
            .mv_list_modules(address, all_modules.last(), None, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        if modules.is_empty() {
            return all_modules;
        }
        all_modules.append(&mut modules);
    }
}

async fn get_all_resources(ctx: &TestContext<'static>, address: AccountAddress) -> Vec<StructTag> {
    let address = address.to_eth_address();
    let mut all_resources = Vec::new();
    loop {
        let mut resources = ctx
            .mv_list_resources(
                address,
                all_resources.last(),
                None,
                BlockNumberOrTag::Latest,
            )
            .await
            .unwrap();
        if resources.is_empty() {
            return all_resources;
        }
        all_resources.append(&mut resources);
    }
}

fn account_struct_tag() -> StructTag {
    StructTag {
        address: AccountAddress::ONE,
        module: Identifier::new("account").unwrap(),
        name: Identifier::new("Account").unwrap(),
        type_args: Vec::new(),
    }
}

fn counter_struct_tag(address: AccountAddress) -> StructTag {
    StructTag {
        address,
        module: Identifier::new("counter").unwrap(),
        name: Identifier::new("Counter").unwrap(),
        type_args: Vec::new(),
    }
}
