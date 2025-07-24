use {
    self::evm_contract::BlockEnv::{
        getBlockNumberCall, getBlockhashCall, getChainIdCall, getMsgSenderCall, getTimestampCall,
        BlockEnvEvents,
    },
    super::*,
    crate::tests::test_context::TestContext,
    alloy::{
        eips::BlockNumberOrTag,
        primitives::{B256, U256},
        sol_types::{SolCall, SolEventInterface},
    },
    umi_blockchain::receipt::TransactionReceipt,
};

mod evm_contract {
    pub const BYTE_CODE_HEX: &str = include_str!("block_env_contract.hex");

    alloy::sol! {
        #[derive(Debug)]
        contract BlockEnv {
            event TheHash (
                bytes32 indexed hash
            );

            event TheUint (
                uint indexed value
            );

            event TheAddress (
                address indexed addr
            );

            function getBlockhash(uint blockNumber) public returns (bytes32);
            function getBlockNumber() public returns (uint);
            function getTimestamp() public returns (uint);
            function getChainId() public returns (uint);
            function getMsgSender() public returns (address);
        }
    }
}

#[tokio::test]
async fn test_block_env_evm_contract() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        // Change the timestamp so that time != block height
        ctx.timestamp = 1000;
        let chain_id = ctx.genesis_config.chain_id;

        // 1. Deploy contract
        let bytecode = alloy::hex::decode(evm_contract::BYTE_CODE_HEX).unwrap();
        let tx = deploy_evm_contract(chain_id, &bytecode);
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        assert!(receipt.inner.inner.is_success());
        let contract_address = receipt.inner.contract_address.unwrap();

        // 2.a check the `msg.sender` as a transaction
        let tx = call_contract(
            chain_id,
            contract_address,
            getMsgSenderCall::SELECTOR.to_vec(),
        );
        let signer = tx.recover_signer().unwrap();
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        let logged_sender = get_logged_address(&receipt);
        assert_eq!(logged_sender, signer);

        // 2.b check `msg.sender` as a view call
        let request = view_contract(contract_address, getMsgSenderCall::SELECTOR.to_vec());
        let sender = request.from.unwrap();
        let response = ctx
            .eth_call(request, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(
            response,
            [[0_u8; 12].as_slice(), sender.0.as_slice()].concat()
        );

        // 3.a check the block number as a transaction
        let tx = call_contract(
            chain_id,
            contract_address,
            getBlockNumberCall::SELECTOR.to_vec(),
        );
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        let block_hash_3 = receipt.inner.block_hash.unwrap(); // We'll need this later.
        let logged_block_height = get_logged_uint(&receipt);
        assert_eq!(
            U256::from(receipt.inner.block_number.unwrap()),
            logged_block_height
        );

        // 3.b check block number as a view call
        let request = view_contract(contract_address, getBlockNumberCall::SELECTOR.to_vec());
        let latest_response = ctx
            .eth_call(request.clone(), BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(U256::from_be_slice(&latest_response), U256::from(3));
        let response = ctx
            .eth_call(request.clone(), BlockNumberOrTag::Number(2))
            .await
            .unwrap();
        assert_eq!(U256::from_be_slice(&response), U256::from(2));

        // 4.a check the block timestamp as a transaction
        let tx = call_contract(
            chain_id,
            contract_address,
            getTimestampCall::SELECTOR.to_vec(),
        );
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        let logged_timestamp = get_logged_uint(&receipt);
        assert_eq!(logged_timestamp, U256::from(ctx.timestamp));

        // 4.b check the timestamp as a view call
        let request = view_contract(contract_address, getTimestampCall::SELECTOR.to_vec());
        let latest_response = ctx
            .eth_call(request.clone(), BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(
            U256::from_be_slice(&latest_response),
            U256::from(ctx.timestamp)
        );
        let response = ctx
            .eth_call(request.clone(), BlockNumberOrTag::Number(3))
            .await
            .unwrap();
        // The timestamp is incremented by 1 in each test, so the previous block has timestamp - 1.
        assert_eq!(
            U256::from_be_slice(&response),
            U256::from(ctx.timestamp - 1)
        );

        // 5.a check the block hash as a transaction
        let evm_input = getBlockhashCall::new((U256::from(3),)).abi_encode();
        let tx = call_contract(chain_id, contract_address, evm_input.clone());
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        let logged_hash = get_logged_hash(&receipt);
        assert_eq!(logged_hash, block_hash_3);

        // 5.b check the block hash as a view call
        let request = view_contract(contract_address, evm_input);
        let response = ctx
            .eth_call(request, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(response, block_hash_3.to_vec());

        // 6.a check the chain id as a transaction
        let tx = call_contract(
            chain_id,
            contract_address,
            getChainIdCall::SELECTOR.to_vec(),
        );
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        let logged_chain_id = get_logged_uint(&receipt);
        assert_eq!(logged_chain_id, U256::from(chain_id));

        // 6.b check the chain id as a view call
        let request = view_contract(contract_address, getChainIdCall::SELECTOR.to_vec());
        let response = ctx
            .eth_call(request, BlockNumberOrTag::Latest)
            .await
            .unwrap();
        assert_eq!(U256::from_be_slice(&response), logged_chain_id);

        ctx.shutdown().await;

        Ok(())
    })
    .await
}

fn get_logged_hash(receipt: &TransactionReceipt) -> B256 {
    match get_logged_event(receipt) {
        BlockEnvEvents::TheHash(hash) => hash.hash,
        _ => panic!("Expected hash log"),
    }
}

fn get_logged_uint(receipt: &TransactionReceipt) -> U256 {
    match get_logged_event(receipt) {
        BlockEnvEvents::TheUint(uint) => uint.value,
        _ => panic!("Expected uint log"),
    }
}

fn get_logged_address(receipt: &TransactionReceipt) -> Address {
    match get_logged_event(receipt) {
        BlockEnvEvents::TheAddress(addr) => addr.addr,
        _ => panic!("Expected address log"),
    }
}

fn get_logged_event(receipt: &TransactionReceipt) -> BlockEnvEvents {
    let log = receipt.inner.inner.logs().first().unwrap();
    BlockEnvEvents::decode_raw_log(log.topics(), &log.data().data, true).unwrap()
}
