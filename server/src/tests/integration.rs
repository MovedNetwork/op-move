use {
    alloy::{
        contract::CallBuilder,
        dyn_abi::EventExt,
        network::{EthereumWallet, TransactionBuilder},
        primitives::{address, utils::parse_ether, Address, B256, U256},
        providers::{Provider, ProviderBuilder},
        rpc::types::eth::TransactionRequest,
        signers::{
            k256::ecdsa::SigningKey,
            local::{LocalSigner, PrivateKeySigner},
        },
        transports::http::reqwest::Url,
    },
    anyhow::{Context, Result},
    aptos_types::transaction::{EntryFunction, ModuleBundle},
    move_binary_format::CompiledModule,
    move_core_types::{ident_str, language_storage::ModuleId, value::MoveValue},
    std::{
        env::var,
        io::Read,
        process::Command,
        str::FromStr,
        time::{Duration, Instant},
    },
    tokio::fs,
    umi_execution::transaction::{ScriptOrDeployment, TransactionData},
    umi_shared::primitives::ToMoveAddress,
};

const L2_RPC_URL: &str = "http://localhost:8545";
const OP_BRIDGE_IN_SECS: u64 = 2 * 60; // Allow up to two minutes for bridging
const OP_BRIDGE_POLL_IN_SECS: u64 = 5;
const TXN_RECEIPT_WAIT_IN_MILLIS: u64 = 100;

// These proxy addresses come from the contract deployments file `1337-deploy.json`
const L1_STANDARD_BRIDGE_PROXY: &str = "0xC8088D0362Bb4AC757ca77e211C30503d39CEf48";
const L2_OUTPUT_ORACLE_PROXY: &str = "0x44C44C1f26aA5b81047DbFd2682F85b78Bd265a8";
const OPTIMISM_PORTAL_PROXY: &str = "0x63F7A1fB6b1C1F0B620cEDF99bE217C5E3e8871D";

mod erc20;
mod withdrawal;

pub fn create_move_counter_contract_bytecode(address: Address) -> Vec<u8> {
    let bytecode_hex = std::fs::read_to_string("src/tests/res/counter.hex").unwrap();
    let bytecode = hex::decode(bytecode_hex.trim()).unwrap();
    set_module_address(bytecode, address)
}

// Ensure the self-address of the module to deploy matches the given address
pub fn set_module_address(bytecode: Vec<u8>, address: Address) -> Vec<u8> {
    let payload: ScriptOrDeployment = bcs::from_bytes(&bytecode).unwrap();
    if let ScriptOrDeployment::ModuleBundle(bundle) = payload {
        // Update the self-address for all modules in the bundle
        let mut updated = Vec::new();
        for module in bundle.into_iter() {
            let mut code = module.into_inner();
            let mut compiled = CompiledModule::deserialize(&code).unwrap();
            let self_module_index = compiled.self_module_handle_idx.0 as usize;
            let self_address_index = compiled.module_handles[self_module_index].address.0 as usize;
            compiled.address_identifiers[self_address_index] = address.to_move_address();
            code.clear();
            compiled.serialize(&mut code).unwrap();
            updated.push(code);
        }
        let module_bundle = ModuleBundle::new(updated);
        bcs::to_bytes(&ScriptOrDeployment::ModuleBundle(module_bundle)).unwrap()
    } else {
        bytecode
    }
}

#[tokio::test]
async fn test_on_ethereum() -> Result<()> {
    dotenvy::dotenv().expect(".env file not found");

    // 1. Test out the OP bridge
    use_optimism_bridge().await?;

    // 2. Test out a simple Move contract
    deploy_move_counter().await?;

    Ok(())
}

async fn use_optimism_bridge() -> Result<()> {
    // Deposit via standard bridge
    deposit_eth_to_l2(Address::from_str(L1_STANDARD_BRIDGE_PROXY)?).await?;
    // Deposit via Optimism Portal
    deposit_eth_to_l2(Address::from_str(OPTIMISM_PORTAL_PROXY)?).await?;

    let erc20_deposit_amount = U256::from(1234);
    let erc20::Erc20AddressPair {
        l1_address,
        l2_address,
    } = deposit_erc20_to_l2(erc20_deposit_amount).await?;

    withdrawal::withdraw_eth_to_l1().await?;

    let erc20_withdrawal_amount = erc20_deposit_amount;
    erc20::withdraw_erc20_token_from_l2_to_l1(
        &get_prefunded_wallet().await?,
        l1_address,
        l2_address,
        erc20_withdrawal_amount,
        &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
        L2_RPC_URL,
    )
    .await?;
    Ok(())
}

async fn deposit_eth_to_l2(bridge_address: Address) -> Result<()> {
    let amount = "100";
    let prefunded_wallet = get_prefunded_wallet().await?;

    let pre_deposit_balance = get_op_balance(prefunded_wallet.address()).await?;
    l1_send_ethers(&prefunded_wallet, bridge_address, amount, false).await?;

    let now = Instant::now();
    let expected_balance = pre_deposit_balance + parse_ether(amount)?;
    while get_op_balance(prefunded_wallet.address()).await? != expected_balance {
        if now.elapsed().as_secs() > OP_BRIDGE_IN_SECS {
            anyhow::bail!(
                "Failed to receive bridged funds within {OP_BRIDGE_POLL_IN_SECS} seconds"
            );
        }
        tokio::time::sleep(Duration::from_secs(OP_BRIDGE_POLL_IN_SECS)).await;
    }
    Ok(())
}

async fn deposit_erc20_to_l2(amount: U256) -> Result<erc20::Erc20AddressPair> {
    let l1_rpc = var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL");
    let from_wallet = get_prefunded_wallet().await?;
    let receiver = from_wallet.address();

    // Deploy ERC-20 token to bridge
    let l1_address = erc20::deploy_l1_token(&from_wallet, &l1_rpc).await?;
    // Create corresponding token on L2
    let l2_address = erc20::deploy_l2_token(&from_wallet, l1_address, L2_RPC_URL).await?;
    // Perform deposit
    erc20::deposit_l1_token(&from_wallet, l1_address, l2_address, amount, &l1_rpc).await?;

    let poll_start = Instant::now();
    while erc20::l2_erc20_balance_of(l2_address, receiver, L2_RPC_URL).await? != amount {
        if poll_start.elapsed().as_secs() > OP_BRIDGE_IN_SECS {
            anyhow::bail!("Failed to receive ERC-20 tokens to L2");
        }
        tokio::time::sleep(Duration::from_secs(OP_BRIDGE_POLL_IN_SECS)).await;
    }

    Ok(erc20::Erc20AddressPair {
        l1_address,
        l2_address,
    })
}

async fn deploy_move_counter() -> Result<()> {
    let from_wallet = get_prefunded_wallet().await?;
    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(L2_RPC_URL)?);

    let bytecode = create_move_counter_contract_bytecode(from_wallet.address());
    let call = CallBuilder::<(), _, _, _>::new_raw_deploy(&provider, bytecode.into());
    let contract_address = call.deploy().await.unwrap();

    let input = TransactionData::EntryFunction(EntryFunction::new(
        ModuleId::new(
            contract_address.to_move_address(),
            ident_str!("counter").into(),
        ),
        ident_str!("publish").into(),
        Vec::new(),
        vec![
            bcs::to_bytes(&MoveValue::Signer(from_wallet.address().to_move_address())).unwrap(),
            bcs::to_bytes(&MoveValue::U64(7)).unwrap(),
        ],
    ));
    let pending_tx =
        CallBuilder::<(), _, _, _>::new_raw(&provider, input.to_bytes().unwrap().into())
            .to(contract_address)
            .send()
            .await
            .unwrap();
    let receipt = pending_tx.get_receipt().await.unwrap();
    assert!(receipt.status(), "Transaction should succeed");

    Ok(())
}

async fn l1_send_ethers(
    from_wallet: &PrivateKeySigner,
    to: Address,
    how_many_ethers: &str,
    check_balance: bool,
) -> Result<()> {
    send_ethers(
        from_wallet,
        to,
        how_many_ethers,
        check_balance,
        &var("L1_RPC_URL")?,
    )
    .await?;
    Ok(())
}

async fn l2_send_ethers(
    from_wallet: &PrivateKeySigner,
    to: Address,
    how_many_ethers: &str,
    check_balance: bool,
) -> Result<B256> {
    send_ethers(from_wallet, to, how_many_ethers, check_balance, L2_RPC_URL).await
}

async fn send_ethers(
    from_wallet: &PrivateKeySigner,
    to: Address,
    how_many_ethers: &str,
    check_balance: bool,
    url: &str,
) -> Result<B256> {
    let from = from_wallet.address();
    let tx = TransactionRequest::default()
        .with_from(from)
        .with_to(to)
        .with_value(parse_ether(how_many_ethers)?);

    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(url)?);
    let prev_balance = provider.get_balance(to).await?;
    let receipt = provider.send_transaction(tx).await?;
    pause(Some(Duration::from_millis(TXN_RECEIPT_WAIT_IN_MILLIS)));
    let tx_hash = receipt.watch().await?;

    if check_balance {
        let new_balance = provider.get_balance(to).await?;
        assert_eq!(new_balance - prev_balance, parse_ether(how_many_ethers)?);
    }
    Ok(tx_hash)
}

async fn get_op_balance(account: Address) -> Result<U256> {
    let provider = ProviderBuilder::new().on_http(Url::parse(L2_RPC_URL)?);
    // Ok(provider.get_balance(account).await?)
    let balance = provider.get_balance(account).await?;
    Ok(balance)
}

async fn get_prefunded_wallet() -> Result<LocalSigner<SigningKey>> {
    // Decrypt the keystore file for L1 dev mode with a blank password
    let keystore_folder = "../l1_datadir/keystore";
    let keystore_path = fs::read_dir(keystore_folder).await?.next_entry().await?;
    let wallet = LocalSigner::decrypt_keystore(keystore_path.expect("No keys").path(), "")?;
    Ok(wallet)
}

/// Pause the main process for an optional duration or indefinitely.
fn pause(how_long: Option<Duration>) {
    if let Some(how_long) = how_long {
        Command::new("sleep")
            .arg(how_long.as_secs_f32().to_string())
            .output()
            .context("Pause timeout failed")
            .unwrap();
    } else {
        // Read a single byte to keep the main process hanging
        let mut stdin = std::io::stdin();
        let _ = stdin.read(&mut [0u8]).unwrap();
    }
}
