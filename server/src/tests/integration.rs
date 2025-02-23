use {
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        contract::CallBuilder,
        dyn_abi::EventExt,
        network::{EthereumWallet, TransactionBuilder, TxSignerSync},
        primitives::{address, utils::parse_ether, Address, TxKind, B256, U256},
        providers::{Provider, ProviderBuilder, WalletProvider},
        rlp::Encodable,
        rpc::types::{eth::TransactionRequest, TransactionReceipt},
        signers::{
            k256::ecdsa::SigningKey,
            local::{LocalSigner, PrivateKeySigner},
        },
        sol_types::{SolCall, SolEventInterface},
        transports::http::reqwest::Url,
    },
    anyhow::{Context, Result},
    aptos_types::{
        transaction::{EntryFunction, Module},
        PeerId,
    },
    move_binary_format::CompiledModule,
    move_core_types::{ident_str, language_storage::ModuleId, value::MoveValue},
    moved_execution::transaction::{ScriptOrModule, TransactionData},
    moved_shared::primitives::ToMoveAddress,
    openssl::rand::rand_bytes,
    serde_json::Value,
    std::{
        env::{set_var, var},
        fs::File,
        io::{prelude::*, Read},
        process::{Child, Command, Output},
        str::FromStr,
        time::{Duration, Instant},
        u64,
    },
    tokio::{fs, runtime::Runtime},
};

const GETH_START_IN_SECS: u64 = 1; // 1 seconds to kick off L1 geth in dev mode
const L2_RPC_URL: &str = "http://localhost:8545";
const OP_BRIDGE_IN_SECS: u64 = 2 * 60; // Allow up to two minutes for bridging
const OP_BRIDGE_POLL_IN_SECS: u64 = 5;
const OP_START_IN_SECS: u64 = 20;
const TXN_RECEIPT_WAIT_IN_MILLIS: u64 = 100;

const OPTIMISM_MINTABLE_ERC20_CREATED: B256 = B256::new(alloy::hex!(
    "52fe89dd5930f343d25650b62fd367bae47088bcddffd2a88350a6ecdd620cdb"
));

mod heartbeat;
mod withdrawal;

alloy::sol!(
    #[sol(rpc)]
    ERC20,
    "src/tests/res/erc20/gold_sol_Gold.abi"
);

alloy::sol!(
    #[sol(rpc)]
    L1StandardBridge,
    "src/tests/res/erc20/L1StandardBridge.json"
);

alloy::sol!(
    #[sol(rpc)]
    OptimismMintable,
    "src/tests/res/erc20/OptimismMintableERC20Factory.json"
);

#[tokio::test]
async fn test_on_ethereum() -> Result<()> {
    // Steps to run a testnet on OP
    // 1. Check the accounts in env vars and Optimism binaries
    check_env_vars();
    check_programs();
    cleanup_files();
    let geth = start_geth().await?;

    // 2. Fund all the OP and deployer accounts, then deploy the factory deployer contract
    fund_accounts().await?;
    check_factory_deployer().await?;

    // 3. Execute config.sh to generate a quickstart configuration file
    generate_config();

    // 4. Execute forge script to deploy the L1 contracts onto Ethereum
    deploy_l1_contracts();

    // 5. Execute L2Genesis forge script to generate state dumps
    state_dump();

    // 6. Generate genesis and jwt files and copy under deployments
    generate_genesis();
    generate_jwt()?;

    // Background task to send transactions to L1 at regular intervals.
    // This ensures the L1 will consistently be producing blocks which
    // is something `op-proposer` expects.
    let hb = heartbeat::HeartbeatTask::new();

    // 7. Init op-geth to start accepting requests
    let op_geth = init_and_start_op_geth()?;

    // 8. Start op-move to accept requests from the sequencer
    let op_move_runtime = Runtime::new()?;
    op_move_runtime.spawn(crate::run());

    // 9. In separate threads run op-node, op-batcher, op-proposer
    let (op_node, op_batcher, op_proposer) = run_op()?;

    // 10. Test out the OP bridge
    // use_optimism_bridge().await?;

    // 11. Test out a simple Move contract
    // deploy_move_counter().await?;

    deploy_erc20().await?;
    pause(Some(Duration::from_secs(OP_BRIDGE_POLL_IN_SECS)));

    // 12. Cleanup generated files and folders
    hb.shutdown().await;
    cleanup_files();
    op_move_runtime.shutdown_background();
    cleanup_processes(vec![geth, op_geth, op_node, op_batcher, op_proposer])
}

fn check_env_vars() {
    // Make sure accounts, chain id and RPC endpoint are registered with `direnv allow`
    dotenvy::dotenv().expect(".env file not found");
    assert!(var("ADMIN_ADDRESS").is_ok());
    assert!(var("ADMIN_PRIVATE_KEY").is_ok());
    assert!(var("BATCHER_ADDRESS").is_ok());
    assert!(var("BATCHER_PRIVATE_KEY").is_ok());
    assert!(var("PROPOSER_ADDRESS").is_ok());
    assert!(var("PROPOSER_PRIVATE_KEY").is_ok());
    assert!(var("SEQUENCER_ADDRESS").is_ok());
    assert!(var("SEQUENCER_PRIVATE_KEY").is_ok());
    assert!(var("L1_RPC_URL").is_ok());
}

fn check_programs() {
    assert!(is_program_in_path("geth"));
    assert!(is_program_in_path("op-geth"));
    assert!(is_program_in_path("op-node"));
    assert!(is_program_in_path("op-batcher"));
    assert!(is_program_in_path("op-proposer"));
    assert!(is_program_in_path("cast"));
}

async fn fund_accounts() -> Result<()> {
    let from_wallet = get_prefunded_wallet().await?;
    // Normally we just fund these accounts once, but for some reason to generate a genesis file
    // we need at least 98 transactions on geth. So we repeat the transactions just to catch up.
    for _ in 0..10 {
        l1_send_ethers(&from_wallet, var("ADMIN_ADDRESS")?.parse()?, "10", true).await?;
        l1_send_ethers(&from_wallet, var("BATCHER_ADDRESS")?.parse()?, "10", true).await?;
        l1_send_ethers(&from_wallet, var("PROPOSER_ADDRESS")?.parse()?, "10", true).await?;
        let factory_deployer_address = address!("3fAB184622Dc19b6109349B94811493BF2a45362");
        l1_send_ethers(&from_wallet, factory_deployer_address, "1", true).await?;
        l1_send_ethers(&from_wallet, heartbeat::ADDRESS, "10", true).await?;
    }
    Ok(())
}

async fn check_factory_deployer() -> Result<()> {
    let factory_address = address!("4e59b44847b379578588920cA78FbF26c0B4956C");
    let code_size = get_code_size(factory_address).await?;
    // Factory deployer contract doesn't exist on L1. This is Ok as long as we can deploy it.
    if code_size != 69 {
        let output = Command::new("cast")
            .args([
                "publish",
                "--rpc-url",
                &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
                // Signed transaction of the contract code
                "0xf8a58085174876e800830186a08080b853604580600e600039806000f350fe7ff\
                fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe036016\
                00081602082378035828234f58015156039578182fd5b8082525050506014600cf31\
                ba02222222222222222222222222222222222222222222222222222222222222222a\
                02222222222222222222222222222222222222222222222222222222222222222",
            ])
            .output()
            .context("Call to foundry cast failed")
            .unwrap();
        check_output(output);
    }
    assert_eq!(get_code_size(factory_address).await?, 69);
    Ok(())
}

fn generate_config() {
    let genesis_file = "optimism/packages/contracts-bedrock/deploy-config/moved.json";
    let output = Command::new("./config.sh")
        .current_dir("src/tests")
        .env("DEPLOY_CONFIG_PATH", genesis_file)
        .output()
        .context("Call to config failed")
        .unwrap();
    check_output(output);
}

fn deploy_l1_contracts() {
    let mut salt = [0; 32]; // 32 byte, u256 random salt for CREATE2 contract deployments
    rand_bytes(&mut salt).unwrap();
    let child_process = Command::new("forge")
        .current_dir("src/tests/optimism/packages/contracts-bedrock")
        .env("DEPLOYMENT_CONTEXT", "moved")
        .env("DEPLOY_CONFIG_PATH", "deploy-config/moved.json")
        .env("IMPL_SALT", hex::encode(salt))
        .args([
            "script",
            "scripts/Deploy.s.sol:Deploy",
            "--private-key",
            &var("ADMIN_PRIVATE_KEY").expect("Missing admin private key"),
            "--broadcast",
            "--rpc-url",
            &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
            "--slow",
            "--legacy",
            "--non-interactive",
        ])
        .spawn()
        .unwrap();
    check_output(child_process.wait_with_output().unwrap());
}

fn state_dump() {
    let output = Command::new("forge")
        .current_dir("src/tests/optimism/packages/contracts-bedrock")
        // Include contract address path in env var only for the genesis script.
        // Globally setting this will make the L1 contracts deployment fail.
        .env("CONTRACT_ADDRESSES_PATH", "deployments/1337-deploy.json")
        .env("DEPLOY_CONFIG_PATH", "deploy-config/moved.json")
        .args([
            "script",
            "scripts/L2Genesis.s.sol:L2Genesis",
            "--sig",
            "runWithAllUpgrades()",
        ])
        .output()
        .context("Call to state dump failed")
        .unwrap();
    check_output(output);
}

fn generate_genesis() {
    let output = Command::new("op-node")
        .current_dir("src/tests/optimism/packages/contracts-bedrock")
        .args([
            "genesis",
            "l2",
            "--deploy-config",
            "deploy-config/moved.json",
            "--l1-deployments",
            "deployments/1337-deploy.json",
            "--l2-allocs",
            "state-dump-42069.json",
            "--outfile.l2",
            "deployments/genesis.json",
            "--outfile.rollup",
            "deployments/rollup.json",
            "--l1-rpc",
            &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
        ])
        .output()
        .context("Call to state dump failed")
        .unwrap();
    check_output(output);
}

fn generate_jwt() -> Result<()> {
    let mut jwt = [0; 32]; // 32 byte, u256 random authentication key
    rand_bytes(&mut jwt).unwrap();
    let mut f = File::create("src/tests/optimism/packages/contracts-bedrock/deployments/jwt.txt")?;
    f.write_all(hex::encode(jwt).as_bytes())?;
    // Set the env var to read the same secret key from the op-move main method
    set_var("JWT_SECRET", hex::encode(jwt));
    Ok(())
}

async fn start_geth() -> Result<Child> {
    let geth_logs = File::create("geth.log").unwrap();
    let geth_process = Command::new("geth")
        .current_dir("src/tests/optimism/")
        .args([
            // Ephemeral proof-of-authority network with a pre-funded developer account,
            // with automatic mining when there are pending transactions.
            "--dev",
            "--datadir",
            "./l1_datadir",
            "--rpc.allow-unprotected-txs",
            "--http",
            "--http.addr",
            "0.0.0.0",
            "--http.port",
            "58138",
            "--http.corsdomain",
            "*",
            "--http.api",
            "web3,debug,eth,txpool,net,engine",
        ])
        .stderr(geth_logs)
        .spawn()?;
    // Give a second to settle geth
    pause(Some(Duration::from_secs(GETH_START_IN_SECS)));
    Ok(geth_process)
}

fn init_and_start_op_geth() -> Result<Child> {
    // Initialize the datadir with genesis
    let output = Command::new("op-geth")
        .current_dir("src/tests/optimism/packages/contracts-bedrock")
        .args([
            "init",
            "--datadir",
            "../../datadir",
            "deployments/genesis.json",
        ])
        .output()
        .context("Call to state dump failed")?;
    check_output(output);
    let op_geth_logs = File::create("op_geth.log").unwrap();
    // Run geth as a child process, so we can continue with the test
    let op_geth_process = Command::new("op-geth")
        // Geth fails to start IPC when the directory name is too long, so simply keeping it short
        .current_dir("src/tests/optimism/packages/contracts-bedrock")
        .args([
            "--datadir",
            "../../datadir",
            "--http",
            "--http.addr",
            "0.0.0.0",
            "--http.port",
            "9545",
            "--http.corsdomain",
            "*",
            "--http.vhosts",
            "*",
            "--http.api",
            "web3,debug,eth,txpool,net,engine",
            "--ws",
            "--ws.addr",
            "0.0.0.0",
            "--ws.port",
            "9546",
            "--ws.origins",
            "*",
            "--ws.api",
            "debug,eth,txpool,net,engine",
            "--syncmode",
            "full",
            "--gcmode",
            "archive",
            "--nodiscover",
            "--maxpeers",
            "0",
            "--networkid",
            "42069",
            "--authrpc.vhosts",
            "*",
            "--authrpc.addr",
            "0.0.0.0",
            "--authrpc.port",
            "9551",
            "--authrpc.jwtsecret",
            "deployments/jwt.txt",
            "--rollup.disabletxpoolgossip",
        ])
        .stderr(op_geth_logs)
        .spawn()?;
    // Give some time for op-geth to settle
    pause(Some(Duration::from_secs(GETH_START_IN_SECS)));
    Ok(op_geth_process)
}

fn run_op() -> Result<(Child, Child, Child)> {
    let op_node_logs = File::create("op_node.log").unwrap();
    let op_node_process = Command::new("op-node")
        .current_dir("src/tests/optimism/packages/contracts-bedrock")
        .args([
            "--l1.beacon.ignore",
            "--l2",
            "http://localhost:8551",
            "--l2.jwt-secret",
            "deployments/jwt.txt",
            "--sequencer.enabled",
            "--sequencer.l1-confs",
            "5",
            "--verifier.l1-confs",
            "4",
            "--rollup.config",
            "deployments/rollup.json",
            "--rpc.addr",
            "0.0.0.0",
            "--rpc.port",
            "8547",
            "--p2p.disable",
            "--rpc.enable-admin",
            "--p2p.sequencer.key",
            &var("SEQUENCER_PRIVATE_KEY").expect("Missing sequencer private key"),
            "--l1",
            &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
            "--l1.rpckind",
            "basic",
        ])
        .stdout(op_node_logs)
        .spawn()?;

    let op_batcher_logs = File::create("op_batcher.log").unwrap();
    let op_batcher_process = Command::new("op-batcher")
        .args([
            "--l2-eth-rpc",
            "http://localhost:8545",
            "--rollup-rpc",
            "http://localhost:8547",
            "--poll-interval",
            "1s",
            "--sub-safety-margin",
            "6",
            "--num-confirmations",
            "1",
            "--safe-abort-nonce-too-low-count",
            "3",
            "--resubmission-timeout",
            "30s",
            "--rpc.addr",
            "0.0.0.0",
            "--rpc.port",
            "8548",
            "--rpc.enable-admin",
            "--max-channel-duration",
            "1",
            "--private-key",
            &var("BATCHER_PRIVATE_KEY").expect("Missing batcher private key"),
            "--l1-eth-rpc",
            &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
        ])
        .stdout(op_batcher_logs)
        .spawn()?;

    let op_proposer_logs = File::create("op_proposer.log").unwrap();
    let op_proposer_process = Command::new("op-proposer")
        .args([
            "--poll-interval",
            "12s",
            "--rpc.port",
            "8560",
            "--rollup-rpc",
            "http://localhost:8547",
            "--l2oo-address",
            &get_deployed_address("L2OutputOracleProxy")?,
            "--private-key",
            &var("PROPOSER_PRIVATE_KEY").expect("Missing proposer private key"),
            "--l1-eth-rpc",
            &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
            "--num-confirmations",
            "1",
            "--allow-non-finalized",
            "true",
        ])
        .stdout(op_proposer_logs)
        .spawn()?;
    Ok((op_node_process, op_batcher_process, op_proposer_process))
}

async fn use_optimism_bridge() -> Result<()> {
    pause(Some(Duration::from_secs(OP_START_IN_SECS)));

    deposit_to_l2().await?;
    // withdrawal::withdraw_to_l1().await?;

    Ok(())
}

async fn deposit_to_l2() -> Result<()> {
    let amount = "100";

    let bridge_address = Address::from_str(&get_deployed_address("L1StandardBridgeProxy")?)?;
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
    eprintln!("deposited to l2");
    pause(None);
    Ok(())
}

async fn deploy_move_counter() -> Result<()> {
    let from_wallet = get_prefunded_wallet().await?;
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(L2_RPC_URL)?);

    let bytecode_hex = std::fs::read_to_string("src/tests/res/counter.hex").unwrap();
    let bytecode = hex::decode(bytecode_hex.trim()).unwrap();
    let bytecode = set_module_address(bytecode, from_wallet.address());

    let call = CallBuilder::new_raw_deploy(&provider, bytecode.into());
    let contract_address = call.deploy().await.unwrap();

    let input = TransactionData::EntryFunction(EntryFunction::new(
        ModuleId::new(
            contract_address.to_move_address(),
            ident_str!("counter").into(),
        ),
        ident_str!("publish").into(),
        Vec::new(),
        vec![
            bcs::to_bytes(&MoveValue::Address(from_wallet.address().to_move_address())).unwrap(),
            bcs::to_bytes(&MoveValue::U64(7)).unwrap(),
        ],
    ));
    let pending_tx = CallBuilder::new_raw(&provider, bcs::to_bytes(&input).unwrap().into())
        .to(contract_address)
        .send()
        .await
        .unwrap();
    let receipt = pending_tx.get_receipt().await.unwrap();
    assert!(receipt.status(), "Transaction should succeed");

    Ok(())
}

// async fn deploy_erc20() -> Result<()> {
//     let from_wallet = get_prefunded_wallet().await?;
//     let l1_provider = ProviderBuilder::new()
//         .with_recommended_fillers()
//         .wallet(EthereumWallet::from(from_wallet.clone()))
//         .on_http(Url::parse(&var("L1_RPC_URL")?)?);
//     let l2_provider = ProviderBuilder::new()
//         .with_recommended_fillers()
//         .wallet(EthereumWallet::from(from_wallet.clone()))
//         .on_http(Url::parse(L2_RPC_URL)?);
//
//     // Deploy L1 Gold (EVM)
//     let l1_bytecode =
//         hex::decode(std::fs::read_to_string("src/tests/res/erc20/gold_sol_Gold.bin")?.trim())?;
//     let l1_call = CallBuilder::new_raw_deploy(&l1_provider, l1_bytecode.into());
//     let l1_token = l1_call.deploy().await?;
//     eprintln!("L1 Gold deployed at: {:?}", l1_token);
//
//     // Deploy L2 Gold (Move)
//     // Deploy base module
//     let base_hex = std::fs::read_to_string("src/tests/res/erc20/base.hex")?;
//     let base_bytecode = hex::decode(base_hex.trim())?;
//     let base_bytecode = set_module_address(base_bytecode, from_wallet.address());
//     let base_call = CallBuilder::new_raw_deploy(&l2_provider, base_bytecode.into());
//     let _base_address = base_call.deploy().await?;
//
//     let l2_token = FRAMEWORK_ADDRESS.to_eth_address();
//     eprintln!("L2 Gold deployed at: {:?}", l2_token);
//
//     // Approve and bridge
//     eprintln!("about to approvue stuff");
//     let bridge_address = Address::from_str(&get_deployed_address("L1StandardBridgeProxy")?)?;
//     let bridge_contract = L1StandardBridge::new(bridge_address, &l1_provider);
//     let contract = ERC20::new(l1_token, &l1_provider);
//     let send_amount = parse_ether("100")?;
//     let approve_tx = contract.approve(bridge_address, send_amount).send().await?;
//     approve_tx.watch().await?;
//     let bridge_tx = bridge_contract
//         .depositERC20(l1_token, l2_token, send_amount, 21_000, "".into())
//         .send()
//         .await?;
//     let receipt = bridge_tx.watch().await?;
//     eprintln!("Bridge tx receipt: {:?}", receipt);
//
//     // Poll L2 logs
//     let filter = alloy::rpc::types::Filter::new()
//         .event("ERC20DepositFinalized(address,address,address,address,uint256,bytes)")
//         .address(
//             "0x4200000000000000000000000000000000000010"
//                 .parse::<Address>()
//                 .unwrap(),
//         );
//
//     let now = Instant::now();
//     while now.elapsed().as_secs() < OP_BRIDGE_IN_SECS {
//         let l2_logs = l2_provider.get_logs(&filter).await?;
//         if !l2_logs.is_empty() {
//             eprintln!("L2 logs: {:?}", l2_logs);
//             break;
//         }
//         tokio::time::sleep(Duration::from_secs(OP_BRIDGE_POLL_IN_SECS)).await;
//     }
//
//     // Check L2 balance
//     let balance_input = TransactionData::EntryFunction(EntryFunction::new(
//         ModuleId::new(l2_token.to_move_address(), ident_str!("Gold").into()),
//         ident_str!("get_balance").into(),
//         Vec::new(),
//         vec![bcs::to_bytes(&from_wallet.address().to_move_address())?],
//     ));
//     let balance_tx = CallBuilder::new_raw(&l2_provider, bcs::to_bytes(&balance_input)?.into())
//         .to(l2_token)
//         .call()
//         .await?;
//     eprintln!("L2 Gold balance: {:?}", balance_tx);
//
//     Ok(())
// }

async fn deploy_erc20() -> Result<()> {
    pause(Some(Duration::from_secs(OP_START_IN_SECS)));
    let from_wallet = get_prefunded_wallet().await?;
    let l1_provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(EthereumWallet::from(from_wallet.clone()))
        .on_http(Url::parse(
            &var("L1_RPC_URL").expect("Missing Ethereum L1 RPC URL"),
        )?);
    let l2_provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(EthereumWallet::from(from_wallet.clone()))
        .on_http(Url::parse(L2_RPC_URL)?);

    let bytecode_hex = std::fs::read_to_string("src/tests/res/erc20/gold_sol_Gold.bin").unwrap();
    let bytecode = hex::decode(bytecode_hex.trim()).unwrap();
    let erc20_call = CallBuilder::new_raw_deploy(&l1_provider, bytecode.into());

    let l1_token_address = erc20_call.deploy().await.unwrap();
    let bridge_address = Address::from_str(&get_deployed_address("L1StandardBridgeProxy")?)?;
    dbg!(&l1_token_address);
    dbg!(&bridge_address);

    let factory_address = Address::new(alloy::hex!("4200000000000000000000000000000000000012"));
    dbg!(&factory_address);

    let name = "Gold".to_string();
    let symbol = "AU".to_string();

    // Encode the function call
    let call = OptimismMintable::createOptimismMintableERC20Call {
        _remoteToken: l1_token_address,
        _name: name,
        _symbol: symbol,
    };

    let tx = TransactionRequest::default()
        // EIP1559
        .transaction_type(2)
        .with_call(&call)
        .with_to(factory_address);
    dbg!(&tx);

    let tx = l2_provider.send_transaction(tx).await?;
    let receipt = tx.get_receipt().await?;
    dbg!(receipt.inner.logs());
    let logs = receipt.inner.logs();

    let mut l2_token_addr = None;
    for log in logs {
        if log.topic0() == Some(&OPTIMISM_MINTABLE_ERC20_CREATED) {
            eprintln!(
                "FOUNDDDD! loc: {:?}, rem: {:?}",
                log.topics()[1],
                log.topics()[2],
            );
            l2_token_addr = Some(Address::from_word(log.topics()[1]));
        }
    }
    let l2_token_addr = l2_token_addr.expect("l2 creation event not found");
    // Extract L2 token address from logs
    eprintln!("L2 Gold created at: {:?}", l2_token_addr);

    let bridge_contract = L1StandardBridge::new(bridge_address, &l1_provider);
    eprintln!(
        "deployed code: {:?}",
        &l1_provider.get_code_at(l1_token_address).await.unwrap()
    );
    let erc20_contract = ERC20::new(l1_token_address, &l1_provider);

    let balance = erc20_contract
        .balanceOf(from_wallet.address())
        .call()
        .await?
        ._0;
    eprintln!("balance of from wallet: {:?}", balance);

    let send_amount = parse_ether("100")?;
    let before_allowance = erc20_contract
        .allowance(from_wallet.address(), bridge_address)
        .call()
        .await?
        ._0;
    eprintln!("allowance before increase: {:?}", before_allowance);
    let approve_tx = erc20_contract
        .approve(bridge_address, send_amount)
        .send()
        .await?;
    approve_tx.watch().await?;
    let after_allowance = erc20_contract
        .allowance(from_wallet.address(), bridge_address)
        .call()
        .await?
        ._0;
    eprintln!("allowance after increase: {:?}", after_allowance);

    let bridge_tx = bridge_contract
        .depositERC20(
            l1_token_address,
            // TODO: h
            l2_token_addr,
            send_amount,
            21_000,
            "".into(),
        )
        .send()
        .await?;
    eprintln!("return of deposit: {:?}", bridge_tx);
    let bridge_tx_hash = bridge_tx.watch().await?;
    dbg!(bridge_tx_hash);
    let balance = erc20_contract
        .balanceOf(from_wallet.address())
        .call()
        .await?
        ._0;
    eprintln!("balance of from wallet after: {:?}", balance);
    let receipt = l1_provider
        .get_transaction_receipt(bridge_tx_hash)
        .await?
        .unwrap();
    eprintln!("success: {:?}", receipt.inner.is_success());
    eprintln!("logs: {:?}", receipt.inner.logs());

    Ok(())
}

// Ensure the self-address of the module to deploy matches the given address
fn set_module_address(bytecode: Vec<u8>, address: Address) -> Vec<u8> {
    let module: ScriptOrModule = bcs::from_bytes(&bytecode).unwrap();
    if let ScriptOrModule::Module(module) = module {
        let mut code = module.into_inner();
        let mut compiled_module = CompiledModule::deserialize(&code).unwrap();
        let self_module_index = compiled_module.self_module_handle_idx.0 as usize;
        let self_address_index =
            compiled_module.module_handles[self_module_index].address.0 as usize;
        compiled_module.address_identifiers[self_address_index] = address.to_move_address();
        code.clear();
        compiled_module.serialize(&mut code).unwrap();
        bcs::to_bytes(&ScriptOrModule::Module(Module::new(code))).unwrap()
    } else {
        bytecode
    }
}

fn cleanup_files() {
    let base = "src/tests/optimism/packages/contracts-bedrock";
    // No unwrap() anywhere, so it doesn't fail if the files don't exist
    std::fs::remove_dir_all("src/tests/optimism/l1_datadir").ok();
    std::fs::remove_dir_all(format!("{}/artifacts", base)).ok();
    std::fs::remove_dir_all(format!("{}/broadcast", base)).ok();
    std::fs::remove_dir_all(format!("{}/cache", base)).ok();
    std::fs::remove_dir_all(format!("{}/forge-artifacts", base)).ok();
    std::fs::remove_file(format!("{}/deploy-config/moved.json", base)).ok();
    std::fs::remove_file(format!("{}/deployments/1337-deploy.json", base)).ok();
    std::fs::remove_file(format!("{}/deployments/31337-deploy.json", base)).ok();
    std::fs::remove_file(format!("{}/state-dump-42069.json", base)).ok();
    std::fs::remove_file(format!("{}/state-dump-42069-delta.json", base)).ok();
    std::fs::remove_file(format!("{}/state-dump-42069-ecotone.json", base)).ok();
    std::fs::remove_file(format!("{}/deployments/genesis.json", base)).ok();
    std::fs::remove_file(format!("{}/deployments/jwt.txt", base)).ok();
    std::fs::remove_file(format!("{}/deployments/rollup.json", base)).ok();
    std::fs::remove_dir_all("src/tests/optimism/datadir").ok();
}

fn cleanup_processes(processes: Vec<Child>) -> Result<()> {
    for mut process in processes {
        process.kill()?;
    }
    Ok(())
}

async fn get_code_size(address: Address) -> Result<usize> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(Url::parse(&var("L1_RPC_URL")?)?);
    let bytecode = provider.get_code_at(address).await?;
    Ok(bytecode.len())
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
        .with_recommended_fillers()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(url)?);
    let prev_balance = provider.get_balance(to).await?;
    let receipt = provider.send_transaction(tx).await?;
    pause(Some(Duration::from_millis(TXN_RECEIPT_WAIT_IN_MILLIS)));
    let tx_hash = receipt.watch().await?;
    dbg!(&tx_hash);

    if check_balance {
        let new_balance = provider.get_balance(to).await?;
        assert_eq!(new_balance - prev_balance, parse_ether(how_many_ethers)?);
    }
    Ok(tx_hash)
}

async fn get_op_balance(account: Address) -> Result<U256> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(Url::parse(L2_RPC_URL)?);
    Ok(provider.get_balance(account).await?)
}

fn check_output(output: Output) {
    if !output.status.success() {
        panic!("Call failed {output:?}");
    }
}

fn is_program_in_path(program: &str) -> bool {
    if let Ok(path) = var("PATH") {
        for p in path.split(':') {
            let p_str = format!("{}/{}", p, program);
            if std::fs::metadata(p_str).is_ok() {
                return true;
            }
        }
    }
    false
}

fn get_deployed_address(field: &str) -> Result<String> {
    // Read the oracle address from the list of deployed contract addresses
    let filename = "src/tests/optimism/packages/contracts-bedrock/deployments/1337-deploy.json";
    let mut deploy_file = File::open(filename)?;
    let mut content = String::new();
    deploy_file.read_to_string(&mut content)?;
    let root: Value = serde_json::from_str(&content)?;
    Ok(root.get(field).unwrap().as_str().unwrap().to_string())
}

async fn get_prefunded_wallet() -> Result<LocalSigner<SigningKey>> {
    // Decrypt the keystore file for L1 dev mode with a blank password
    let keystore_folder = "src/tests/optimism/l1_datadir/keystore";
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
