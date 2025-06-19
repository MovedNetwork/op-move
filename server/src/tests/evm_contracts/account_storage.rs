use {
    self::evm_contract::AccountStorage::{getCall, setCall, AccountStorageEvents},
    super::*,
    crate::tests::test_context::TestContext,
    alloy::{
        eips::BlockNumberOrTag,
        primitives::U256,
        rpc::types::{TransactionInput, TransactionRequest},
        sol_types::{SolCall, SolEventInterface},
    },
    umi_blockchain::receipt::TransactionReceipt,
};

mod evm_contract {
    // Compiled EVM bytecode for the contract below.
    pub const BYTE_CODE: &[u8] = &alloy::hex!("6080604052348015600e575f5ffd5b5060fb8061001b5f395ff3fe6080604052348015600e575f5ffd5b50600436106030575f3560e01c80636d4ce63c146034578063b8e010de14604e575b5f5ffd5b603a6056565b6040516045919060ae565b60405180910390f35b6054605e565b005b5f5f54905090565b5f439050805f81905550807f7c5c37c4d1bd29015cf8cce0679fb2f5f2304c146e166b9818eb88066fa20b2f60405160405180910390a250565b5f819050919050565b60a8816098565b82525050565b5f60208201905060bf5f83018460a1565b9291505056fea2646970667358221220da549b6120938432a34ac97107116b998dee3f0f19d27c262490ff744e02654064736f6c634300081e0033");

    alloy::sol! {
        // This contract has one function which stores the current block height in the
        // smart contract account storage, and another function which retrieves that
        // stored value.
        #[derive(Debug)]
        contract AccountStorage {
            event TheHeight (
                uint indexed height
            );

            // Writes the current block height into the contract storage.
            function set() public;

            // Reads the value stored in the contract.
            function get() public view returns (uint);
        }
    }
}

#[tokio::test]
async fn test_storage_evm_contract() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        let chain_id = ctx.genesis_config.chain_id;

        // 1. Deploy contract in block with height = 1
        let tx = deploy_evm_contract(chain_id, evm_contract::BYTE_CODE);
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        assert!(receipt.inner.inner.is_success());
        let contract_address = receipt.inner.contract_address.unwrap();

        // 2. Call `set` function in block with heights <= 3
        for block_height in [2, 3] {
            let tx = call_contract(chain_id, contract_address, setCall::SELECTOR);
            let receipt = ctx.execute_transaction(tx).await.unwrap();
            assert_eq!(receipt.inner.block_number.unwrap(), block_height);

            assert_eq!(get_logged_height(&receipt), U256::from(block_height));
        }

        // 3. Use a view call to check the value stored in the contract at heights 2 and 3.
        let input = TransactionData::EvmContract {
            address: contract_address,
            data: getCall::SELECTOR.to_vec(),
        };
        let view_request = TransactionRequest {
            to: Some(TxKind::Call(contract_address)),
            input: TransactionInput::new(input.to_bytes().unwrap().into()),
            ..Default::default()
        };
        let height_2 = ctx
            .eth_call(view_request.clone(), BlockNumberOrTag::Number(2))
            .await
            .unwrap();
        assert_eq!(U256::from_be_slice(&height_2), U256::from(2));

        let height_3 = ctx
            .eth_call(view_request, BlockNumberOrTag::Number(3))
            .await
            .unwrap();
        assert_eq!(U256::from_be_slice(&height_3), U256::from(3));

        ctx.shutdown().await;

        Ok(())
    })
    .await
}

fn get_logged_height(receipt: &TransactionReceipt) -> U256 {
    let log = receipt.inner.inner.logs().first().unwrap();
    let AccountStorageEvents::TheHeight(height) =
        AccountStorageEvents::decode_raw_log(log.topics(), &log.data().data, true).unwrap();
    height.height
}
