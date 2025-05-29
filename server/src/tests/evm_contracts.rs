use {
    self::evm_contract::BlockHash::{getBlockHashCall, BlockHashEvents},
    crate::tests::test_context::TestContext,
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        network::TxSignerSync,
        primitives::{Address, TxKind, B256},
        signers::local::PrivateKeySigner,
        sol_types::{SolCall, SolEventInterface},
    },
    umi_blockchain::receipt::TransactionReceipt,
    umi_execution::transaction::{ScriptOrDeployment, TransactionData},
};

mod evm_contract {
    // Compiled EVM bytecode for the contract below.
    pub const BYTE_CODE: &[u8] = &alloy::hex!("6080604052348015600e575f80fd5b50609e80601a5f395ff3fe6080604052348015600e575f80fd5b50600436106026575f3560e01c80639663f88f14602a575b5f80fd5b60306032565b005b5f6003409050807fdb1186d7ae4c4cb4bbea2fcfa5bf68b2b1c9026e9a2fc5ab0c8b1c8f2fcf555f60405160405180910390a25056fea2646970667358221220503bd64eb974d245be70ccfd75762d5de56b77df2d4bf59eb89f2fb2d993d8d564736f6c634300081a0033");

    alloy::sol! {
        // This contract has one function which uses the `BLOCKHASH` EVM opcode
        // to try to get the hash for block number 3. It emits the response
        // as an event.
        contract BlockHash {
            event TheHash (
                bytes32 indexed hash
            );

            // Returns the block hash of block number 3
            function getBlockHash() public;
        }
    }
}

#[tokio::test]
async fn test_evm_contracts() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        let chain_id = ctx.genesis_config.chain_id;

        // 1. Deploy contract in block with height = 1
        let tx = deploy_evm_contract(chain_id, evm_contract::BYTE_CODE);
        let receipt = ctx.execute_transaction(tx).await.unwrap();
        assert!(receipt.inner.inner.is_success());
        let contract_address = receipt.inner.contract_address.unwrap();

        // 2. Call `getBlockHash` function in block with heights <= 3
        for block_height in [2, 3] {
            let tx = call_contract(chain_id, contract_address, getBlockHashCall::SELECTOR);
            let receipt = ctx.execute_transaction(tx).await.unwrap();
            assert_eq!(receipt.inner.block_number.unwrap(), block_height);

            // In this range the `BLOCKHASH` EVM opcode with input 0x3 returns 0x0 because
            // height 3 has not happened yet.
            assert_eq!(get_logged_hash(&receipt), B256::ZERO);
        }

        let block = ctx.get_block_by_number(3).await.unwrap();
        let expected_hash = block.0.header.hash;

        // 3. Call `getBlockHash` function in block with 4 <= height <= 259
        for block_height in 4..=259 {
            let tx = call_contract(chain_id, contract_address, getBlockHashCall::SELECTOR);
            let receipt = ctx.execute_transaction(tx).await.unwrap();
            assert_eq!(receipt.inner.block_number.unwrap(), block_height);

            // In this range the `BLOCKHASH` EVM opcode with input 0x3 returns the block
            // hash for the block at height 3.
            assert_eq!(get_logged_hash(&receipt), expected_hash);
        }

        // 4. Call `getBlockHash` function in block with heights > 259
        for block_height in [260, 261] {
            let tx = call_contract(chain_id, contract_address, getBlockHashCall::SELECTOR);
            let receipt = ctx.execute_transaction(tx).await.unwrap();
            assert_eq!(receipt.inner.block_number.unwrap(), block_height);

            // In this range the `BLOCKHASH` EVM opcode with input 0x3 returns 0x0 because
            // height 3 is too far in the past (more than 256 blocks ago).
            assert_eq!(get_logged_hash(&receipt), B256::ZERO);
        }

        ctx.shutdown().await;

        Ok(())
    })
    .await
}

fn get_logged_hash(receipt: &TransactionReceipt) -> B256 {
    let log = receipt.inner.inner.logs().first().unwrap();
    let BlockHashEvents::TheHash(hash) =
        BlockHashEvents::decode_raw_log(log.topics(), &log.data().data, true).unwrap();
    hash.hash
}

fn deploy_evm_contract(chain_id: u64, bytecode: &[u8]) -> TxEnvelope {
    let signer = PrivateKeySigner::random();
    let input = ScriptOrDeployment::EvmContract(bytecode.to_vec());
    sign_transaction(
        chain_id,
        TxKind::Create,
        || bcs::to_bytes(&input).unwrap(),
        &signer,
    )
}

fn call_contract(chain_id: u64, to: Address, selector: [u8; 4]) -> TxEnvelope {
    let signer = PrivateKeySigner::random();
    let input = TransactionData::EvmContract {
        address: to,
        data: selector.to_vec(),
    };
    sign_transaction(
        chain_id,
        TxKind::Call(to),
        || input.to_bytes().unwrap(),
        &signer,
    )
}

fn sign_transaction<F: FnOnce() -> Vec<u8>>(
    chain_id: u64,
    to: TxKind,
    input: F,
    signer: &PrivateKeySigner,
) -> TxEnvelope {
    let mut tx = TxEip1559 {
        chain_id,
        nonce: 0,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to,
        value: Default::default(),
        access_list: Default::default(),
        input: input().into(),
    };
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    TxEnvelope::Eip1559(tx.into_signed(signature))
}
