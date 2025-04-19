use {
    self::erc20_factory::OptimismMintableERC20Factory::OptimismMintableERC20Created,
    super::*,
    alloy::sol_types::SolEvent,
    move_vm_runtime::session::SerializedReturnValues,
    moved_evm_ext::{extract_evm_result, EVM_NATIVE_ADDRESS, EVM_NATIVE_OUTCOME_LAYOUT},
    moved_shared::primitives::{ToEthAddress, ToMoveU256},
};

alloy::sol!(
    #[sol(rpc)]
    Erc20,
    "src/tests/res/ERC20.json"
);
mod erc20_factory {
    alloy::sol!(
        #[sol(rpc)]
        OptimismMintableERC20Factory,
        "src/tests/res/OptimismMintableERC20Factory.json"
    );
}

mod bridge_l1 {
    alloy::sol!(
        #[sol(rpc)]
        L1StandardBridge,
        "src/tests/res/L1StandardBridge.json"
    );
}

mod bridge_l2 {
    alloy::sol!(
        #[sol(rpc)]
        L2StandardBridge,
        "src/tests/res/L2StandardBridge.json"
    );
}

const NAME: &str = "Gold";
const SYMBOL: &str = "AU";
// We didn't spend much time thinking about this. This value is probably too high.
// But it shouldn't really matter because it is used as part of a deposit-type transaction
// which has lots of gas to work with.
const L2_MINT_ERC20_GAS_LIMIT: u32 = 100_000;

const L2_STANDARD_BRIDGE_ADDRESS: Address = address!("4200000000000000000000000000000000000010");

pub struct Erc20AddressPair {
    pub l1_address: Address,
    pub l2_address: Address,
}

/// Create a new ERC-20 token on the L1 chain, returning its address.
/// For convenience, this function also calls `approve` on the new
/// ERC-20 token allowing the `L1StandardBridgeProxy` to spend the newly
/// created tokens.
pub async fn deploy_l1_token(from_wallet: &PrivateKeySigner, rpc_url: &str) -> Result<Address> {
    let from_address = from_wallet.address();
    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(rpc_url)?);

    let contract = Erc20::deploy(
        provider,
        NAME.into(),
        SYMBOL.into(),
        from_address,
        U256::MAX,
    )
    .await?;

    let bridge_address = Address::from_str(&get_deployed_address("L1StandardBridgeProxy")?)?;
    contract
        .approve(bridge_address, U256::MAX)
        .send()
        .await?
        .watch()
        .await?;

    Ok(*contract.address())
}

/// Use the `OptimismMintableERC20Factory` to create a new ERC-20 token on the L2.
/// This token is used for bridging the L1 token with the given address to the L2.
pub async fn deploy_l2_token(
    from_wallet: &PrivateKeySigner,
    l1_address: Address,
    rpc_url: &str,
) -> Result<Address> {
    let factory_address = alloy::primitives::address!("4200000000000000000000000000000000000012");
    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(rpc_url)?);

    let contract = erc20_factory::OptimismMintableERC20Factory::new(factory_address, provider);
    let receipt = contract
        .createOptimismMintableERC20(l1_address, NAME.into(), SYMBOL.into())
        .send()
        .await?
        .get_receipt()
        .await?;
    let event_signature = OptimismMintableERC20Created::SIGNATURE_HASH;
    let event = receipt
        .inner
        .logs()
        .iter()
        .find(|log| {
            log.topic0()
                .map(|topic| topic == &event_signature)
                .unwrap_or(false)
        })
        .expect("OptimismMintableERC20Factory emits log");
    let event = event
        .log_decode::<OptimismMintableERC20Created>()
        .expect("Event is type OptimismMintableERC20Created");
    let l2_token_address = event.data().localToken;
    Ok(l2_token_address)
}

/// Submits a transaction to the `L1StandardBridgeProxy` to deposit L1 ERC-20 tokens (`l1_address`)
/// into the L2, where the address of the token on the L2 is also specified (`l2_address`).
pub async fn deposit_l1_token(
    from_wallet: &PrivateKeySigner,
    l1_address: Address,
    l2_address: Address,
    amount: U256,
    rpc_url: &str,
) -> Result<()> {
    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(rpc_url)?);

    let bridge_address = Address::from_str(&get_deployed_address("L1StandardBridgeProxy")?)?;
    let bridge_contract = bridge_l1::L1StandardBridge::new(bridge_address, provider);
    let receipt = bridge_contract
        .depositERC20(
            l1_address,
            l2_address,
            amount,
            L2_MINT_ERC20_GAS_LIMIT,
            Default::default(),
        )
        .send()
        .await?
        .get_receipt()
        .await?;
    assert!(receipt.inner.is_success(), "ERC-20 deposit should succeed");
    Ok(())
}

pub async fn withdraw_erc20_token_from_l2_to_l1(
    wallet: &PrivateKeySigner,
    l1_address: Address,
    l2_address: Address,
    amount: U256,
    l1_rpc_url: &str,
    l2_rpc_url: &str,
) -> Result<()> {
    let owner_address = wallet.address();

    // Approve bridge to spend tokens
    let spender = L2_STANDARD_BRIDGE_ADDRESS;
    let initial_allowance = l2_erc20_allowance(l2_address, owner_address, spender, l2_rpc_url)
        .await
        .unwrap();
    assert_eq!(initial_allowance, U256::ZERO);

    erc20::l2_erc20_approve(wallet, l2_address, spender, amount, l2_rpc_url)
        .await
        .unwrap();
    let allowance = erc20::l2_erc20_allowance(l2_address, owner_address, spender, l2_rpc_url)
        .await
        .unwrap();
    assert_eq!(allowance, amount);

    // Initiate bridging
    let l2_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(wallet.to_owned()))
        .on_http(Url::parse(l2_rpc_url)?);
    let bridge_contract = bridge_l2::L2StandardBridge::new(L2_STANDARD_BRIDGE_ADDRESS, l2_provider);
    let receipt = bridge_contract
        .bridgeERC20(l2_address, l1_address, amount, 100_000, Default::default())
        .send()
        .await?
        .get_receipt()
        .await?;
    assert!(
        receipt.inner.is_success(),
        "ERC-20 L2 deposit should succeed"
    );

    // Get initial balance
    let l1_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(wallet.to_owned()))
        .on_http(Url::parse(l1_rpc_url)?);
    let l1_token = Erc20::new(l1_address, l1_provider);
    let initial_balance = l1_token.balanceOf(owner_address).call().await?._0;

    // Prove withdraw on L1
    let withdraw_tx_hash = receipt.transaction_hash;
    super::withdrawal::withdraw_to_l1(withdraw_tx_hash, wallet.clone()).await?;

    // Check final balance
    let final_balance = l1_token.balanceOf(owner_address).call().await?._0;
    assert_eq!(
        initial_balance + amount,
        final_balance,
        "L1 balance should increase"
    );

    Ok(())
}

pub async fn l2_erc20_balance_of(token: Address, account: Address, rpc_url: &str) -> Result<U256> {
    let provider = ProviderBuilder::new().on_http(Url::parse(rpc_url)?);

    let args = vec![
        // The caller here does not matter because it is a view call.
        MoveValue::Signer(EVM_NATIVE_ADDRESS)
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(token.to_move_address())
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(account.to_move_address())
            .simple_serialize()
            .unwrap(),
    ];
    let function_call = EntryFunction::new(
        ModuleId::new(EVM_NATIVE_ADDRESS, ident_str!("erc20").into()),
        ident_str!("balance_of").into(),
        Vec::new(),
        args,
    );
    let tx_data = TransactionData::EntryFunction(function_call);
    let data = bcs::to_bytes(&tx_data)?;
    let eth_call_result = CallBuilder::<(), _, _, _>::new_raw(provider, data.into())
        .to(EVM_NATIVE_ADDRESS.to_eth_address())
        .call()
        .await?;

    // Extract the first result and combine with EVM result layout for deserialization
    let output: Vec<Vec<u8>> = bcs::from_bytes(&eth_call_result)?;
    let return_values = SerializedReturnValues {
        mutable_reference_outputs: Vec::new(),
        return_values: Vec::from([(
            output.first().unwrap().to_owned(),
            EVM_NATIVE_OUTCOME_LAYOUT.clone(),
        )]),
    };
    let evm_result = extract_evm_result(return_values);

    Ok(U256::from_be_slice(&evm_result.output))
}

pub async fn l2_erc20_allowance(
    token: Address,
    owner: Address,
    spender: Address,
    rpc_url: &str,
) -> Result<U256> {
    let provider = ProviderBuilder::new().on_http(Url::parse(rpc_url)?);

    let args = vec![
        // The caller here does not matter because it is a view call.
        MoveValue::Signer(EVM_NATIVE_ADDRESS)
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(token.to_move_address())
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(owner.to_move_address())
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(spender.to_move_address())
            .simple_serialize()
            .unwrap(),
    ];
    let function_call = EntryFunction::new(
        ModuleId::new(EVM_NATIVE_ADDRESS, ident_str!("erc20").into()),
        ident_str!("allowance").into(),
        Vec::new(),
        args,
    );
    let tx_data = TransactionData::EntryFunction(function_call);
    let data = bcs::to_bytes(&tx_data)?;
    let eth_call_result = CallBuilder::<(), _, _, _>::new_raw(provider, data.into())
        .to(EVM_NATIVE_ADDRESS.to_eth_address())
        .call()
        .await?;

    let output: Vec<Vec<u8>> = bcs::from_bytes(&eth_call_result)?;
    let return_values = SerializedReturnValues {
        mutable_reference_outputs: Vec::new(),
        return_values: Vec::from([(
            output.first().unwrap().to_owned(),
            EVM_NATIVE_OUTCOME_LAYOUT.clone(),
        )]),
    };
    let evm_result = extract_evm_result(return_values);

    Ok(U256::from_be_slice(&evm_result.output))
}

pub async fn l2_erc20_approve(
    from_wallet: &PrivateKeySigner,
    token: Address,
    spender: Address,
    amount: U256,
    rpc_url: &str,
) -> Result<()> {
    let from_address = from_wallet.address();
    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(rpc_url)?);

    let args = vec![
        MoveValue::Signer(from_address.to_move_address())
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(token.to_move_address())
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(spender.to_move_address())
            .simple_serialize()
            .unwrap(),
        MoveValue::U256(amount.to_move_u256())
            .simple_serialize()
            .unwrap(),
    ];
    let function_call = EntryFunction::new(
        ModuleId::new(EVM_NATIVE_ADDRESS, ident_str!("erc20").into()),
        ident_str!("approve_entry").into(),
        Vec::new(),
        args,
    );
    let tx_data = TransactionData::EntryFunction(function_call);
    let data = bcs::to_bytes(&tx_data)?;
    let receipt = CallBuilder::<(), _, _, _>::new_raw(provider, data.into())
        .to(EVM_NATIVE_ADDRESS.to_eth_address())
        .send()
        .await?
        .get_receipt()
        .await?;

    assert!(receipt.inner.is_success(), "ERC-20 approve should succeed");
    Ok(())
}
