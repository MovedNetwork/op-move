use {
    crate::tests::test_context::TestContext, alloy::eips::BlockNumberOrTag,
    move_core_types::account_address::AccountAddress, umi_shared::primitives::ToEthAddress,
};

#[tokio::test]
async fn test_mv_list_modules() -> anyhow::Result<()> {
    TestContext::run(|ctx| async move {
        let address = AccountAddress::ONE;
        let modules = ctx
            .mv_list_modules(
                address.to_eth_address(),
                None,
                None,
                BlockNumberOrTag::Latest,
            )
            .await
            .unwrap();

        println!("{modules:#?}");

        ctx.shutdown().await;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn test_mv_list_resources() -> anyhow::Result<()> {
    TestContext::run(|ctx| async move {
        let address = AccountAddress::ONE;
        let resources = ctx
            .mv_list_resources(
                address.to_eth_address(),
                None,
                None,
                BlockNumberOrTag::Latest,
            )
            .await
            .unwrap();

        println!("{resources:#?}");

        ctx.shutdown().await;
        Ok(())
    })
    .await
}
