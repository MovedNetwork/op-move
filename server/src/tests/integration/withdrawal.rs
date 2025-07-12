//! Code related to OP-stack withdrawal flow.
//! (Separated into its own module because there is a lot of it.)

use super::*;

#[allow(clippy::too_many_arguments)]
mod op_oracle {
    alloy::sol!(
        #[sol(rpc)]
        L2OutputOracle,
        "src/tests/res/L2OutputOracle.json"
    );
}

mod op_portal {
    alloy::sol!(
        #[sol(rpc)]
        OptimismPortal,
        "src/tests/res/OptimismPortal.json"
    );
}

const MAX_WITHDRAWAL_TIMEOUT: u64 = 10 * 60;
const WITHDRAW_ADDRESS: Address =
    alloy::primitives::address!("4200000000000000000000000000000000000016");

pub async fn withdraw_eth_to_l1() -> Result<()> {
    let amount = "1";
    let prefunded_wallet = get_prefunded_wallet().await?;
    let prefunded_address = prefunded_wallet.address();

    // `check_balance=false` balance because tokens are burned.
    let withdraw_tx_hash =
        l2_send_ethers(&prefunded_wallet, WITHDRAW_ADDRESS, amount, false).await?;

    let l1_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(prefunded_wallet.clone()))
        .on_http(Url::parse(&var("L1_RPC_URL")?)?);

    let pre_finalize_balance = l1_provider.get_balance(prefunded_address).await?;

    withdraw_to_l1(withdraw_tx_hash, prefunded_wallet).await?;

    let post_finalize_balance = l1_provider.get_balance(prefunded_address).await?;
    assert!(
        pre_finalize_balance < post_finalize_balance,
        "Withdraw should increase funds"
    );

    Ok(())
}

pub async fn withdraw_to_l1(withdraw_tx_hash: B256, l1_wallet: PrivateKeySigner) -> Result<()> {
    let l2_provider = ProviderBuilder::new().on_http(Url::parse(L2_RPC_URL)?);
    let rx = l2_provider
        .get_transaction_receipt(withdraw_tx_hash)
        .await?
        .unwrap();

    // Extract the withdrawal event from the transaction log
    let withdrawal_log = rx
        .inner
        .logs()
        .iter()
        .find(|l| l.address() == WITHDRAW_ADDRESS)
        .unwrap();
    let event = withdraw_event();
    let decoded = event.decode_log(withdrawal_log.data(), true).unwrap();
    let (withdrawal_hash, _) = decoded.body.last().unwrap().as_fixed_bytes().unwrap();

    // `storage_slot` is calculated based on the Solidity convention for how maps work.
    let slot_preimage = [withdrawal_hash, &[0u8; 32]].concat();
    let storage_slot = alloy::primitives::keccak256(slot_preimage);

    let l1_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(l1_wallet))
        .on_http(Url::parse(&var("L1_RPC_URL")?)?);

    // Contract used on the L1 for withdrawals
    let portal_address = Address::from_str(OPTIMISM_PORTAL_PROXY)?;
    let portal_contract = op_portal::OptimismPortal::new(portal_address, &l1_provider);

    // Contract used on the L1 to keep track of the L2 state
    let oracle_address = Address::from_str(L2_OUTPUT_ORACLE_PROXY)?;
    let l2_oracle_contract = op_oracle::L2OutputOracle::new(oracle_address, &l1_provider);

    // Wait for proposer to push new blocks top L1
    let withdraw_block_number = withdrawal_log.block_number.unwrap();
    let now = Instant::now();
    loop {
        let l2_block_number: u64 = l2_oracle_contract
            .latestBlockNumber()
            .call()
            .await
            .unwrap()
            ._0
            .saturating_to();

        // Timeout to prevent this from being an infinite loop if something breaks
        if now.elapsed().as_secs() > MAX_WITHDRAWAL_TIMEOUT {
            anyhow::bail!(
                "WITHDRAW ERROR: L1 contract `L2OutputOracleProxy` not updated to block containing withdraw within 10 minutes. CurrentBlock={l2_block_number} TargetBlock={withdraw_block_number}"
            );
        }

        // If the latest L2 block number that the L1 knows about
        // is larger than the block where the withdraw happened then we
        // can move on to the next step. Otherwise we wait before
        // checking again.
        if l2_block_number >= withdraw_block_number {
            break;
        }
        tokio::time::sleep(Duration::from_secs(OP_BRIDGE_IN_SECS)).await;
    }

    // Get the latest L2 block height (according to the L1 contract).
    let l2_output_index = l2_oracle_contract
        .latestOutputIndex()
        .call()
        .await
        .unwrap()
        ._0;
    let l2_output = l2_oracle_contract
        .getL2Output(l2_output_index)
        .call()
        .await
        .unwrap()
        ._0;
    let l2_block_number = l2_output.l2BlockNumber as u64;

    // Look up the corresponding L2 block
    let block = l2_provider
        .get_block_by_number(l2_block_number.into())
        .await?
        .unwrap();

    // Get the merkle proof for the withdrawal L2 contract at that height
    let proof = l2_provider
        .get_proof(WITHDRAW_ADDRESS, vec![storage_slot])
        .number(l2_block_number)
        .await?;

    // Prepare args for the OptimismPortal contract
    let withdraw_tx = op_portal::Types::WithdrawalTransaction {
        nonce: decoded.indexed[0].as_uint().unwrap().0,
        sender: decoded.indexed[1].as_address().unwrap(),
        target: decoded.indexed[2].as_address().unwrap(),
        value: decoded.body[0].as_uint().unwrap().0,
        gasLimit: decoded.body[1].as_uint().unwrap().0,
        data: decoded.body[2].as_bytes().unwrap().to_vec().into(),
    };
    let output_proof = op_portal::Types::OutputRootProof {
        version: Default::default(),
        stateRoot: block.header.state_root,
        messagePasserStorageRoot: proof.storage_hash,
        latestBlockhash: block.header.hash,
    };

    // Submit proof of withdrawal to L1
    let prove_tx = portal_contract.proveWithdrawalTransaction(
        withdraw_tx.clone(),
        l2_output_index,
        output_proof,
        proof.storage_proof[0].proof.clone(),
    );

    let pending = prove_tx
        .send()
        .await
        .inspect_err(|e| println!("Prove Err {e:?}"))?;
    let prove_tx_hash = pending
        .watch()
        .await
        .inspect_err(|e| println!("Prove Err {e:?}"))?;

    let prove_rx = l1_provider
        .get_transaction_receipt(prove_tx_hash)
        .await?
        .unwrap();
    assert!(prove_rx.status(), "Prove Tx failed");

    // Wait for finalization
    tokio::time::sleep(Duration::from_secs(OP_BRIDGE_IN_SECS)).await;

    // Finalize withdrawal
    let pending = portal_contract
        .finalizeWithdrawalTransaction(withdraw_tx)
        .send()
        .await
        .inspect_err(|e| println!("Finalize Err {e:?}"))?;
    let finalize_tx_hash = pending
        .watch()
        .await
        .inspect_err(|e| println!("Finalize Err {e:?}"))?;
    let finalize_rx = l1_provider
        .get_transaction_receipt(finalize_tx_hash)
        .await?
        .unwrap();
    assert!(finalize_rx.status(), "Finalize Tx failed");

    Ok(())
}

fn withdraw_event() -> alloy::json_abi::Event {
    let message_passed = r#"event MessagePassed(
            uint256 indexed nonce,
            address indexed sender,
            address indexed target,
            uint256 value,
            uint256 gasLimit,
            bytes data,
            bytes32 withdrawalHash
        )"#
    .replace('\n', "");
    alloy::json_abi::Event::parse(&message_passed).unwrap()
}

#[test]
fn test_withdrawal_event() {
    let event = withdraw_event();
    assert_eq!(
        event.selector().0,
        alloy::hex!("02a52367d10742d8032712c1bb8e0144ff1ec5ffda1ed7d70bb05a2744955054")
    );
}
