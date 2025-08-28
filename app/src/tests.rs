use {
    super::*,
    crate::{
        Payload, PayloadForExecution, TestDependencies,
        query::{MAX_SUGGESTED_PRIORITY_FEE, MIN_SUGGESTED_PRIORITY_FEE},
    },
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        eips::BlockNumberOrTag::{self, *},
        hex,
        network::TxSignerSync,
        primitives::{TxKind, address},
        rpc::types::FeeHistory,
        signers::local::PrivateKeySigner,
    },
    move_core_types::{account_address::AccountAddress, effects::ChangeSet},
    move_vm_runtime::{
        AsUnsyncCodeStorage,
        module_traversal::{TraversalContext, TraversalStorage},
    },
    move_vm_types::gas::UnmeteredGasMeter,
    std::sync::Arc,
    test_case::test_case,
    umi_blockchain::{
        block::{
            Block, BlockHash, BlockRepository, Eip1559GasFee, Header, InMemoryBlockQueries,
            InMemoryBlockRepository, UmiBlockHash,
        },
        in_memory::shared_memory,
        payload::{InMemoryPayloadQueries, InProgressPayloads, MaybePayloadResponse},
        receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, receipt_memory},
        state::{BlockHeight, InMemoryStateQueries, MockStateQueries, StateQueries},
        transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
    },
    umi_evm_ext::state::{BlockHashWriter, InMemoryStorageTrieRepository, StorageTrieRepository},
    umi_execution::{
        UmiBaseTokenAccounts, create_vm_session,
        session_id::SessionId,
        transaction::{NormalizedEthTransaction, UmiTxEnvelope},
    },
    umi_genesis::{
        CreateMoveVm, UmiVm,
        config::{CHAIN_ID, GenesisConfig},
    },
    umi_shared::{
        error::{Error, UserError},
        primitives::{Address, B256, ToMoveAddress, U64, U256},
    },
    umi_state::{Changes, InMemoryState, InMemoryTrieDb, ResolverBasedModuleBytesStorage, State},
};

/// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

pub const EVM_ADDRESS: Address = address!("8fd379246834eac74b8419ffda202cf8051f7a03");

#[derive(Debug)]
pub struct Signer {
    pub inner: PrivateKeySigner,
    pub nonce: u64,
}

impl Signer {
    pub fn new(key_bytes: &[u8; 32]) -> Self {
        Self {
            inner: PrivateKeySigner::from_bytes(&key_bytes.into()).unwrap(),
            nonce: 0,
        }
    }
}

fn create_app_with_given_queries<SQ: StateQueries + Clone + Send + Sync + 'static>(
    height: u64,
    state_queries: SQ,
) -> (
    ApplicationReader<'static, TestDependencies<SQ>>,
    Application<'static, TestDependencies<SQ>>,
) {
    let genesis_config = GenesisConfig::default();

    let head_hash = B256::new(hex!(
        "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
    ));
    let genesis_block = Block::default()
        .into_extended_with_hash(head_hash)
        .with_value(U256::ZERO);

    let (memory_reader, mut memory) = shared_memory::new();
    let mut block_hash_cache =
        HybridBlockHashCache::new(memory_reader.clone(), InMemoryBlockQueries);
    let mut repository = InMemoryBlockRepository::new();

    for i in 0..=height {
        let mut block = genesis_block.clone();
        block.block.header.number = i;
        block.hash = block.block.header.hash_slow();
        repository.add(&mut memory, block).unwrap();
        block_hash_cache.push(i, head_hash);
    }

    let mut state = InMemoryState::default();
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let (changes, evm_storage_changes) = umi_genesis_image::load();
    umi_genesis::apply(
        changes,
        evm_storage_changes,
        &genesis_config,
        &mut state,
        &mut evm_storage,
    );

    let (receipt_memory_reader, receipt_memory) = receipt_memory::new();
    let in_progress_payloads = InProgressPayloads::default();

    (
        ApplicationReader {
            genesis_config: genesis_config.clone(),
            base_token: UmiBaseTokenAccounts::new(AccountAddress::ONE),
            block_queries: InMemoryBlockQueries,
            block_hash_lookup: block_hash_cache.clone(),
            payload_queries: InMemoryPayloadQueries::new(in_progress_payloads.clone()),
            receipt_queries: InMemoryReceiptQueries::new(),
            receipt_memory: receipt_memory_reader.clone(),
            storage: memory_reader.clone(),
            state_queries: state_queries.clone(),
            evm_storage: evm_storage.clone(),
            transaction_queries: InMemoryTransactionQueries::new(),
        },
        Application {
            mem_pool: Default::default(),
            genesis_config,
            base_token: UmiBaseTokenAccounts::new(AccountAddress::ONE),
            block_hash_lookup: block_hash_cache.clone(),
            block_hash_writer: block_hash_cache,
            block_hash: UmiBlockHash,
            block_queries: InMemoryBlockQueries,
            block_repository: repository,
            on_payload: CommandActor::on_payload_noop(),
            on_tx: CommandActor::on_tx_noop(),
            on_tx_batch: CommandActor::on_tx_batch_noop(),
            payload_queries: InMemoryPayloadQueries::new(in_progress_payloads.clone()),
            receipt_queries: InMemoryReceiptQueries::new(),
            receipt_repository: InMemoryReceiptRepository::new(),
            receipt_memory,
            receipt_memory_reader,
            storage: memory,
            storage_reader: memory_reader,
            state,
            state_queries,
            evm_storage,
            transaction_queries: InMemoryTransactionQueries::new(),
            transaction_repository: InMemoryTransactionRepository::new(),
            gas_fee: Eip1559GasFee::default(),
            l1_fee: U256::ZERO,
            l2_fee: U256::ZERO,
            resolver_cache: Default::default(),
        },
    )
}

fn mint_eth(
    state: &impl State,
    evm_storage: &impl StorageTrieRepository,
    addr: AccountAddress,
    amount: U256,
) -> ChangeSet {
    let umi_vm = UmiVm::new(&Default::default());
    let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state.resolver());
    let code_storage = module_bytes_storage.as_unsync_code_storage(&umi_vm);
    let vm = umi_vm.create_move_vm().unwrap();
    let mut session = create_vm_session(
        &vm,
        state.resolver(),
        SessionId::default(),
        evm_storage,
        &(),
        &(),
    );
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = UnmeteredGasMeter;

    umi_execution::mint_eth(
        &addr,
        amount,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
        &code_storage,
    )
    .unwrap();

    session.finish(&code_storage).unwrap()
}

fn create_app_with_fake_queries(
    addr: AccountAddress,
    initial_balance: U256,
    base_fee: u64,
    height: u64,
) -> (
    ApplicationReader<'static, TestDependencies>,
    Application<'static, TestDependencies>,
) {
    let genesis_config = GenesisConfig::default();

    let head_hash = B256::new(hex!(
        "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
    ));
    let mut genesis_block = Block::default()
        .into_extended_with_hash(head_hash)
        .with_value(U256::ZERO);
    genesis_block.block.header.base_fee_per_gas = Some(base_fee);

    let (memory_reader, mut memory) = shared_memory::new();
    let mut block_hash_cache =
        HybridBlockHashCache::new(memory_reader.clone(), InMemoryBlockQueries);
    let mut repository = InMemoryBlockRepository::new();
    repository.add(&mut memory, genesis_block.clone()).unwrap();
    block_hash_cache.push(0, head_hash);

    for i in 1..=height {
        let mut block = genesis_block.clone();
        block.block.header.number = i;
        block.hash = block.block.header.hash_slow();
        block_hash_cache.push(i, block.hash);
        repository.add(&mut memory, block).unwrap();
    }

    let evm_storage = InMemoryStorageTrieRepository::new();
    let trie_db = Arc::new(InMemoryTrieDb::empty());
    let mut state = InMemoryState::empty(trie_db.clone());
    let (genesis_changes, evm_storage_changes) = umi_genesis_image::load();

    state.apply(genesis_changes).unwrap();
    evm_storage.apply(evm_storage_changes).unwrap();
    let changes_addition = mint_eth(&state, &evm_storage, addr, initial_balance);
    state
        .apply(Changes::without_tables(changes_addition))
        .unwrap();

    let (receipt_reader, receipt_memory) = receipt_memory::new();

    let state_queries = InMemoryStateQueries::new(
        memory_reader.clone(),
        trie_db,
        genesis_config.initial_state_root,
    );
    let in_progress_payloads = InProgressPayloads::default();

    (
        ApplicationReader {
            genesis_config: genesis_config.clone(),
            base_token: UmiBaseTokenAccounts::new(AccountAddress::ONE),
            block_hash_lookup: block_hash_cache.clone(),
            block_queries: InMemoryBlockQueries,
            payload_queries: InMemoryPayloadQueries::new(in_progress_payloads.clone()),
            receipt_queries: InMemoryReceiptQueries::new(),
            receipt_memory: receipt_reader.clone(),
            storage: memory_reader.clone(),
            state_queries: state_queries.clone(),
            evm_storage: evm_storage.clone(),
            transaction_queries: InMemoryTransactionQueries::new(),
        },
        Application::<TestDependencies> {
            mem_pool: Default::default(),
            genesis_config,
            base_token: UmiBaseTokenAccounts::new(AccountAddress::ONE),
            block_hash: UmiBlockHash,
            block_hash_lookup: block_hash_cache.clone(),
            block_hash_writer: block_hash_cache,
            block_queries: InMemoryBlockQueries,
            block_repository: repository,
            on_payload: CommandActor::on_payload_in_memory(),
            on_tx: CommandActor::on_tx_in_memory(),
            on_tx_batch: CommandActor::on_tx_batch_in_memory(),
            payload_queries: InMemoryPayloadQueries::new(in_progress_payloads.clone()),
            receipt_queries: InMemoryReceiptQueries::new(),
            receipt_repository: InMemoryReceiptRepository::new(),
            receipt_memory,
            storage: memory,
            receipt_memory_reader: receipt_reader,
            storage_reader: memory_reader,
            state,
            state_queries,
            evm_storage,
            transaction_queries: InMemoryTransactionQueries::new(),
            transaction_repository: InMemoryTransactionRepository::new(),
            gas_fee: Eip1559GasFee::default(),
            l1_fee: U256::ZERO,
            l2_fee: U256::ZERO,
            resolver_cache: Default::default(),
        },
    )
}

#[test]
fn test_build_block_hash() {
    use alloy::{hex, primitives::address};

    let payload_attributes = Payload {
        timestamp: U64::from(0x6759e370_u64),
        prev_randao: B256::new(hex!(
            "ade920edae8d7bb10146e7baae162b5d5d8902c5a2a4e9309d0bf197e7fdf9b6"
        )),
        suggested_fee_recipient: address!("4200000000000000000000000000000000000011"),
        withdrawals: Vec::new(),
        parent_beacon_block_root: Default::default(),
        transactions: Vec::new(),
        gas_limit: U64::from(0x1c9c380),
        eip1559_params: Some(U64::from_be_slice(&hex!("0x000000fa00000006"))),
        no_tx_pool: None,
    };

    let execution_outcome = ExecutionOutcome {
        receipts_root: B256::new(hex!(
            "3c55e3bccc48ee3ee637d8fc6936e4825d1489cbebf6057ce8025d63755ebf54"
        )),
        state_root: B256::new(hex!(
            "5affa0c563587bc4668feaea28e997d29961e864be20b0082d123bcb2fbbaf55"
        )),
        logs_bloom: Default::default(),
        gas_used: U64::from(0x272a2),
        total_tip: Default::default(),
    };

    let header = Header {
        parent_hash: B256::new(hex!(
            "966c80cc0cbf7dbf7a2b2579002b95c8756f388c3fbf4a77c4d94d3719880c6e"
        )),
        number: 1,
        transactions_root: B256::new(hex!(
            "c355179c91ebb544d6662d6ad580c45eb3f155e1626b693b3afa4fdca677c450"
        )),
        base_fee_per_gas: Some(0x3b5dc100),
        blob_gas_used: Some(0),
        excess_blob_gas: Some(0),
        withdrawals_root: Some(B256::new(hex!(
            "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
        ))),
        ..Default::default()
    }
    .with_payload_attributes(payload_attributes.try_into().unwrap())
    .with_execution_outcome(execution_outcome);

    let hash = UmiBlockHash.block_hash(&header);
    assert_eq!(
        hash,
        B256::new(hex!(
            "c9f7a6ef5311bf49b8322a92f3d75bd5c505ee613323fb58c7166c3511a62bcf"
        ))
    );
}

#[test_case(Latest, 4, 4; "Latest")]
#[test_case(Finalized, 4, 4; "Finalized")]
#[test_case(Safe, 4, 4; "Safe")]
#[test_case(Earliest, 4, 0; "Earliest")]
#[test_case(Pending, 4, 4; "Pending")]
#[test_case(Number(2), 4, 2; "Number")]
fn test_nonce_is_fetched_by_height_successfully(
    height: BlockNumberOrTag,
    head_height: BlockHeight,
    expected_height: BlockHeight,
) {
    let address = Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
    let (reader, _app) = create_app_with_given_queries(
        head_height,
        MockStateQueries(address.to_move_address(), expected_height),
    );

    let actual_nonce = reader.nonce_by_height(address, height).unwrap();
    let expected_nonce = 3;

    assert_eq!(actual_nonce, expected_nonce);
}

#[test_case(Latest, 2, 2; "Latest")]
#[test_case(Finalized, 2, 2; "Finalized")]
#[test_case(Safe, 2, 2; "Safe")]
#[test_case(Earliest, 2, 0; "Earliest")]
#[test_case(Pending, 2, 2; "Pending")]
#[test_case(Number(1), 2, 1; "Number")]
fn test_balance_is_fetched_by_height_successfully(
    height: BlockNumberOrTag,
    head_height: BlockHeight,
    expected_height: BlockHeight,
) {
    let address = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let (reader, _app) = create_app_with_given_queries(
        head_height,
        MockStateQueries(address.to_move_address(), expected_height),
    );

    let actual_balance = reader.balance_by_height(address, height).unwrap();
    let expected_balance = U256::from(5);

    assert_eq!(actual_balance, expected_balance);
}

fn create_transaction(nonce: u64) -> NormalizedEthTransaction {
    let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let amount = U256::from(4);
    let signer = Signer::new(&PRIVATE_KEY);
    let mut tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce: signer.nonce + nonce,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Call(to),
        value: amount,
        access_list: Default::default(),
        input: Default::default(),
    };
    let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();

    let tx_envelope = TxEnvelope::Eip1559(tx.into_signed(signature));
    let umi_tx: UmiTxEnvelope = tx_envelope.try_into().unwrap();
    umi_tx.try_into().unwrap()
}

fn create_transaction_with_max_fee_and_gas_limit(
    nonce: u64,
    max_fee: u128,
    gas_limit: u64,
) -> NormalizedEthTransaction {
    let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let amount = U256::from(4);
    let signer = Signer::new(&PRIVATE_KEY);
    let mut tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce: signer.nonce + nonce,
        gas_limit,
        max_fee_per_gas: max_fee,
        max_priority_fee_per_gas: max_fee / 2,
        to: TxKind::Call(to),
        value: amount,
        access_list: Default::default(),
        input: Default::default(),
    };
    let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();

    let tx_envelope = TxEnvelope::Eip1559(tx.into_signed(signature));
    let umi_tx: UmiTxEnvelope = tx_envelope.try_into().unwrap();
    umi_tx.try_into().unwrap()
}

#[test]
fn test_fetched_balances_are_updated_after_transfer_of_funds() {
    let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let initial_balance = U256::from(5);
    let amount = U256::from(4);
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance, 0, 0);

    let tx = create_transaction(0);

    app.add_transaction(tx);
    app.start_block_build(
        PayloadForExecution::default(),
        U64::from(0x03421ee50df45cacu64),
    )
    .unwrap();

    let actual_recipient_balance = reader.balance_by_height(to, Latest).unwrap();
    let expected_recipient_balance = amount;

    assert_eq!(actual_recipient_balance, expected_recipient_balance);

    let actual_sender_balance = reader.balance_by_height(EVM_ADDRESS, Latest).unwrap();
    let expected_sender_balance = initial_balance - amount;

    assert_eq!(actual_sender_balance, expected_sender_balance);
}

#[test]
fn test_fetched_nonces_are_updated_after_executing_transaction() {
    let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let initial_balance = U256::from(5);
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance, 0, 0);

    let tx = create_transaction(0);

    app.add_transaction(tx);
    app.start_block_build(
        PayloadForExecution::default(),
        U64::from(0x03421ee50df45cacu64),
    )
    .unwrap();

    let actual_recipient_balance = reader.nonce_by_height(to, Latest).unwrap();
    let expected_recipient_balance = 0;

    assert_eq!(actual_recipient_balance, expected_recipient_balance);

    let actual_sender_balance = reader.nonce_by_height(EVM_ADDRESS, Latest).unwrap();
    let expected_sender_balance = 1;

    assert_eq!(actual_sender_balance, expected_sender_balance);
}

#[test]
fn test_one_payload_can_be_fetched_repeatedly() {
    let initial_balance = U256::from(5);
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance, 0, 0);

    let tx = create_transaction(0);

    app.add_transaction(tx);

    let payload_id = U64::from(0x03421ee50df45cacu64);

    app.start_block_build(PayloadForExecution::default(), payload_id)
        .unwrap();

    let expected_payload = reader.payload(payload_id).unwrap().unwrap();
    let actual_payload = reader.payload(payload_id).unwrap().unwrap();

    assert_eq!(expected_payload, actual_payload);
}

#[test]
fn test_older_payload_can_be_fetched_again_successfully() {
    let initial_balance = U256::from(15);
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance, 0, 0);

    let tx = create_transaction(0);

    app.add_transaction(tx);

    let payload_id = U64::from(0x03421ee50df45cacu64);

    app.start_block_build(
        Payload {
            gas_limit: U64::MAX,
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        payload_id,
    )
    .unwrap();

    let expected_payload = reader.payload(payload_id).unwrap().unwrap();

    let tx = create_transaction(1);

    app.add_transaction(tx);

    let payload_2_id = U64::from(0x03421ee50df45dadu64);

    app.start_block_build(
        Payload {
            timestamp: U64::from(1u64),
            gas_limit: U64::MAX,
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        payload_2_id,
    )
    .unwrap();

    // make sure the newer payload is fetchable
    let _ = reader.payload(payload_2_id).unwrap();

    let older_payload = reader.payload(payload_id).unwrap().unwrap();

    assert_eq!(expected_payload, older_payload);
}

#[test]
fn test_txs_from_one_account_have_proper_nonce_ordering() {
    let initial_balance = U256::from(1000);
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance, 0, 0);

    let mut tx_hashes: Vec<B256> = Vec::with_capacity(10);

    for i in 0..10 {
        let tx = create_transaction(i);
        tx_hashes.push(tx.tx_hash.0.into());
        app.add_transaction(tx);
    }

    let payload_id = U64::from(0x03421ee50df45cacu64);

    app.start_block_build(PayloadForExecution::default(), payload_id)
        .unwrap();

    for (i, tx_hash) in tx_hashes.iter().enumerate() {
        // Get receipt for this transaction
        let receipt = reader.transaction_receipt(*tx_hash);

        let receipt = receipt.expect("Database should work").unwrap_or_else(|| {
            panic!(
                "Transaction with nonce {} and hash {:?} has no receipt",
                i, tx_hash
            )
        });

        assert!(
            receipt.inner.inner.status(),
            "Transaction with nonce {} and hash {:?} failed",
            i,
            tx_hash
        );

        assert!(
            receipt
                .inner
                .transaction_index
                .is_some_and(|idx| idx == i as u64),
            "Transaction with nonce {} has incorrect index {:?}",
            i,
            receipt.inner.transaction_index
        );

        assert_eq!(
            receipt.inner.from, EVM_ADDRESS,
            "Transaction with nonce {} has unexpected sender",
            i
        );
    }

    let payload = reader.payload(payload_id).unwrap().unwrap();
    assert!(
        payload.execution_payload.transactions.len() == 10,
        "Expected 10 transactions in block, but found {:?}",
        payload.execution_payload.transactions.len()
    );
}

#[test_case(0, None => matches Err(Error::User(UserError::InvalidBlockCount(0))); "zero block count")]
#[test_case(5, None => matches Ok(_); "block count too long")]
#[test_case(1, Some(vec![0.0; 101]) => matches Err(Error::User(UserError::RewardPercentilesTooLong{max: 100, given: 101})); "too many percentiles")]
#[test_case(1, Some(vec![50.0, 101.0]) => matches Err(Error::User(UserError::InvalidRewardPercentiles(_))); "percentile out of range")]
#[test_case(1, Some(vec![-5.0]) => matches Err(Error::User(UserError::InvalidRewardPercentiles(_))); "negative percentile")]
#[test_case(1, Some(vec![75.0, 25.0, 50.0]) => matches Err(Error::User(UserError::InvalidRewardPercentiles(_))); "unsorted percentiles")]
#[test_case(1, Some(vec![25.0, 50.0, 75.0]) => matches Ok(_); "valid percentiles")]
#[test_case(1, None => matches Ok(_); "no percentiles")]
fn test_fee_history_validation(
    block_count: u64,
    percentiles: Option<Vec<f64>>,
) -> Result<FeeHistory, Error> {
    let (reader, _app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10), 0, 1);

    reader.fee_history(block_count, Latest, percentiles)
}

#[test_case(1, Latest, 5; "single block latest")]
#[test_case(2, Latest, 4; "two blocks latest")]
#[test_case(100, Latest, 0; "block count exceeds available")]
#[test_case(2, Earliest, 0; "earliest block")]
#[test_case(2, Number(3), 2; "specific block number")]
#[test_case(1, Number(0), 0; "genesis block")]
fn test_fee_history_block_ranges(
    block_count: u64,
    block_tag: BlockNumberOrTag,
    expected_oldest: u64,
) {
    let (reader, _app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10), 0, 5);

    let result = reader.fee_history(block_count, block_tag, None);
    assert!(result.is_ok());

    let fee_history = result.unwrap();
    assert_eq!(fee_history.oldest_block, expected_oldest);
}

#[test_case(None, 1; "no percentiles")]
#[test_case(Some(vec![50.0]), 1; "single percentile")]
#[test_case(Some(vec![25.0, 50.0, 75.0]), 3; "triple percentiles")]
#[test_case(Some(vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0]), 9; "many percentiles")]
fn test_fee_history_reward_lengths(percentiles: Option<Vec<f64>>, expected_reward_length: usize) {
    let (reader, _app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10), 0, 1);

    let result = reader.fee_history(1, Latest, percentiles);
    assert!(result.is_ok());

    let fee_history = result.unwrap();

    match &fee_history.reward {
        Some(rewards) => {
            assert_eq!(rewards.len(), 1);
            assert_eq!(rewards[0].len(), expected_reward_length);
        }
        None => assert_eq!(expected_reward_length, 1),
    }
}

#[test]
fn test_fee_history_eip1559_fields() {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10), 0, 1);

    app.start_block_build(PayloadForExecution::default(), U64::from(1))
        .unwrap();

    let result = reader.fee_history(1, Latest, None);
    assert!(result.is_ok());

    let fee_history = result.unwrap();

    // Verify EIP-1559 fields
    assert_eq!(fee_history.base_fee_per_gas.len(), 2); // Current + next block
    assert_eq!(fee_history.gas_used_ratio.len(), 1);

    // Verify EIP-4844 fields are zero (not supported)
    assert_eq!(fee_history.base_fee_per_blob_gas.len(), 2);
    assert!(fee_history.base_fee_per_blob_gas.iter().all(|&x| x == 0));
    assert_eq!(fee_history.blob_gas_used_ratio.len(), 1);
    assert!(fee_history.blob_gas_used_ratio.iter().all(|&x| x == 0.0));
}

#[test_case(0, true; "empty blocks have zero gas ratio")]
#[test_case(5, false; "blocks with transactions have non-zero gas ratio")]
fn test_fee_history_empty_vs_full_blocks(num_txs: usize, expect_zero_ratio: bool) {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(1000), 0, 0);

    for i in 0..num_txs {
        let tx = create_transaction(i as u64);
        app.add_transaction(tx);
    }

    let payload = Payload {
        gas_limit: U64::from(1_000_000),
        ..Default::default()
    };
    app.start_block_build(payload.try_into().unwrap(), U64::from(1))
        .unwrap();

    let result = reader.fee_history(1, Latest, Some(vec![50.0]));
    assert!(result.is_ok());

    let fee_history = result.unwrap();

    if expect_zero_ratio {
        assert_eq!(fee_history.gas_used_ratio[0], 0.0);
    } else {
        assert!(fee_history.gas_used_ratio[0] > 0.0);
    }
}

#[test_case(vec![5_000_000_000, 4_000_000_000, 3_000_000_000, 2_000_000_000, 1_000_000_000], vec![21000; 5], vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0], vec![500_000_000, 500_000_000, 1_000_000_000, 1_500_000_000, 2_000_000_000, 2_500_000_000]; "equal gas usage descending prices")]
#[test_case(vec![1_000_000_000, 2_000_000_000, 3_000_000_000], vec![33333, 33333, 33334], vec![25.0, 50.0, 75.0], vec![500_000_000, 1_000_000_000, 1_500_000_000]; "three equal gas transactions")]
#[test_case(vec![2_500_000_000], vec![21000], vec![0.0, 25.0, 50.0, 75.0, 100.0], vec![1_250_000_000; 5]; "single transaction all percentiles")]
fn test_fee_history_percentile_calculations(
    gas_prices: Vec<u128>,
    gas_limits: Vec<u64>,
    percentiles: Vec<f64>,
    expected_rewards: Vec<u128>,
) {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10000000), 0, 0);

    for (i, (&gas_price, &gas_limit)) in gas_prices.iter().zip(gas_limits.iter()).enumerate() {
        let tx = create_transaction_with_max_fee_and_gas_limit(i as u64, gas_price, gas_limit);
        app.add_transaction(tx);
    }

    app.start_block_build(
        Payload {
            gas_limit: U64::from(1_000_000),
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        U64::from(1),
    )
    .unwrap();

    let result = reader.fee_history(1, Latest, Some(percentiles));
    assert!(result.is_ok());

    let fee_history = result.unwrap();
    if let Some(rewards) = &fee_history.reward {
        assert_eq!(rewards[0].len(), expected_rewards.len());
        for (actual, expected) in rewards[0].iter().zip(expected_rewards.iter()) {
            assert_eq!(*actual, *expected);
        }
    }
}

#[test_case(vec![1, 2, 3], true; "increasing transactions")]
#[test_case(vec![3, 2, 1], false; "decreasing transactions")]
#[test_case(vec![2, 2, 2], false; "constant transactions")]
fn test_fee_history_gas_ratio_progression(tx_counts: Vec<usize>, expect_increasing: bool) {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10000000), 0, 0);

    let mut nonce = 0;
    // Reasonable amount to allow a tx succeed
    let gas_limit = 30_000;
    for (block_num, &tx_count) in tx_counts.iter().enumerate() {
        for _ in 0..tx_count {
            let tx = create_transaction_with_max_fee_and_gas_limit(nonce, 1_000_000_000, gas_limit);
            app.add_transaction(tx);
            nonce += 1;
        }

        app.start_block_build(
            Payload {
                timestamp: U64::from(block_num as u64 + 1),
                gas_limit: U64::from(1_000_000),
                ..Default::default()
            }
            .try_into()
            .unwrap(),
            U64::from(block_num as u64 + 1),
        )
        .unwrap();
    }

    let result = reader.fee_history(tx_counts.len() as u64, Latest, None);
    assert!(result.is_ok());

    let fee_history = result.unwrap();
    assert_eq!(fee_history.gas_used_ratio.len(), tx_counts.len());

    if expect_increasing {
        for i in 1..fee_history.gas_used_ratio.len() {
            assert!(fee_history.gas_used_ratio[i - 1] < fee_history.gas_used_ratio[i]);
        }
    }

    for (i, &tx_count) in tx_counts.iter().enumerate() {
        let expected_ratio = tx_count as f64 * gas_limit as f64 / 1_000_000.0;
        assert!((fee_history.gas_used_ratio[i] - expected_ratio).abs() < 0.01);
    }
}

#[test]
fn test_fee_history_boundary_percentiles() {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10000000), 0, 0);

    // Create exactly 4 transactions with equal gas usage but different prices for boundary testing
    let gas_prices = [1_000_000_000, 2_000_000_000, 3_000_000_000, 4_000_000_000];

    for (i, &gas_price) in gas_prices.iter().enumerate() {
        let tx = create_transaction_with_max_fee_and_gas_limit(i as u64, gas_price, 25_000);
        app.add_transaction(tx);
    }

    app.start_block_build(
        Payload {
            gas_limit: U64::from(1_000_000),
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        U64::from(1),
    )
    .unwrap();

    let percentiles = vec![0.0, 33.33, 66.66, 100.0];
    let result = reader.fee_history(1, Latest, Some(percentiles));

    assert!(result.is_ok());
    let fee_history = result.unwrap();

    if let Some(rewards) = &fee_history.reward {
        let block_rewards = &rewards[0];
        // With 4 equal-gas transactions:
        // 0% -> first tx, 33.33% -> second tx, 66.66% -> third tx, 100% -> fourth tx
        assert_eq!(block_rewards[0], 500_000_000); // 0th percentile
        assert_eq!(block_rewards[1], 1_000_000_000); // 33rd percentile  
        assert_eq!(block_rewards[2], 1_500_000_000); // 66th percentile
        assert_eq!(block_rewards[3], 2_000_000_000); // 100th percentile
    }
}

#[test]
fn test_max_priority_fee_low_congestion() {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10000000), 1, 0);

    // Most likely to actually consume less than this preset tx gas limit
    for i in 0..10 {
        let tx = create_transaction_with_max_fee_and_gas_limit(i, 2_000_000_000, 40_000);
        app.add_transaction(tx);
    }

    app.start_block_build(
        Payload {
            gas_limit: U64::from(1_000_000), // still well within the block limits
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        U64::from(1),
    )
    .unwrap();

    let max_priority_fee = reader.max_priority_fee_per_gas().unwrap();

    // In low congestion, should return minimum priority fee
    assert_eq!(max_priority_fee, MIN_SUGGESTED_PRIORITY_FEE);
}

#[test]
fn test_max_priority_fee_high_congestion() {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10000000), 1, 0);

    // Low preset gas limit actually makes txs run out of gas, but they still get receipts and
    // block inclusion, and allows us to control actual gas consumed.
    for i in 0..5 {
        let tx = create_transaction_with_max_fee_and_gas_limit(
            i,
            2_000_000_000 + (i * 100_000_000) as u128,
            20_000,
        );
        app.add_transaction(tx);
    }

    app.start_block_build(
        Payload {
            gas_limit: U64::from(100_000),
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        U64::from(1),
    )
    .unwrap();

    let max_priority_fee = reader.max_priority_fee_per_gas().unwrap();

    assert!(max_priority_fee > MIN_SUGGESTED_PRIORITY_FEE);

    // With 5 transactions, median is the 3rd. Expected tip for 3rd transaction:
    // (2_000_000_000 + 2 * 100_000_000) / 2 = 1_100_000_000 (Division due to max
    // priority fee set to half max fee at init). That value gets bumped by 10% additionally.
    let expected_priority_fee = 1_210_000_000;
    assert_eq!(max_priority_fee, expected_priority_fee);
}

#[test]
fn test_gas_price_high_max_fee() {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10000000), 1, 0);

    // Same setup as before, but with very high constant max fees instead of gradually increasing
    // moderate ones
    for i in 0..5 {
        let tx = create_transaction_with_max_fee_and_gas_limit(i, 1_000_000_000_000, 21_000);
        app.add_transaction(tx);
    }

    app.start_block_build(
        Payload {
            gas_limit: U64::from(100_000),
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        U64::from(1),
    )
    .unwrap();

    let max_priority_fee = reader.max_priority_fee_per_gas().unwrap();

    // Should be clamped to maximum as 1e12 > 5e11
    assert_eq!(max_priority_fee, MAX_SUGGESTED_PRIORITY_FEE);
}

#[test]
fn test_gas_price_vs_max_priority_fee_difference() {
    let (reader, mut app) =
        create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(10000000), 1, 0);

    for i in 0..10 {
        let tx = create_transaction_with_max_fee_and_gas_limit(i, 2_000_000_000, 40_000);
        app.add_transaction(tx);
    }

    app.start_block_build(
        Payload {
            gas_limit: U64::from(1_000_000),
            ..Default::default()
        }
        .try_into()
        .unwrap(),
        U64::from(1),
    )
    .unwrap();

    let gas_price = reader.gas_price().unwrap();
    let max_priority_fee = reader.max_priority_fee_per_gas().unwrap();

    // `eth_gasPrice` and `eth_maxPriorityFeePerGas` differ exactly by base fee
    let difference = gas_price - max_priority_fee;
    assert!(difference > 0);

    let block = reader.block_by_height(Latest, true).unwrap();
    let actual_base_fee = block.0.header.inner.base_fee_per_gas.unwrap_or_default();

    assert_eq!(difference, actual_base_fee as u128);
}

#[test]
fn test_no_tx_pool_does_not_affect_mempool() {
    let payload_signer = Signer::new(&[0xbb; 32]);
    let initial_balance = U256::from(1_000_000_000);

    let (reader, mut app) = create_app_with_fake_queries(
        payload_signer.inner.address().to_move_address(),
        initial_balance,
        0,
        0,
    );

    let mempool_tx = create_transaction(0); // Uses the default signer (0xaa...)
    app.add_transaction(mempool_tx.clone());
    assert_eq!(
        app.mem_pool.len(),
        1,
        "Mempool should have one transaction before block build"
    );

    let mut payload_tx_raw = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce: 0,
        gas_limit: 21000,
        max_fee_per_gas: 1_000_000_000,
        max_priority_fee_per_gas: 1_000_000_000,
        to: TxKind::Call(Address::random()),
        value: U256::from(1),
        access_list: Default::default(),
        input: Default::default(),
    };
    let signature = payload_signer
        .inner
        .sign_transaction_sync(&mut payload_tx_raw)
        .unwrap();
    let payload_tx_envelope = TxEnvelope::Eip1559(payload_tx_raw.into_signed(signature));
    let payload_tx: NormalizedEthTransaction = (UmiTxEnvelope::try_from(payload_tx_envelope)
        .unwrap())
    .try_into()
    .unwrap();

    let payload_attributes = PayloadForExecution {
        no_tx_pool: Some(true),
        transactions: vec![payload_tx.clone().into()],
        ..Default::default()
    };
    let payload_id = U64::from(1);

    app.start_block_build(payload_attributes, payload_id)
        .unwrap();

    assert_eq!(
        app.mem_pool.len(),
        1,
        "Mempool should still have one transaction after block build"
    );
    let remaining_mempool_tx = app.mem_pool.iter().next().unwrap();
    assert_eq!(
        remaining_mempool_tx.tx_hash, mempool_tx.tx_hash,
        "The transaction in the mempool should be the original one"
    );

    let payload_response =
        if let MaybePayloadResponse::Some(payload) = reader.payload(payload_id).unwrap() {
            payload
        } else {
            unreachable!("Payload should have been applied in block build")
        };
    assert_eq!(
        payload_response.execution_payload.transactions.len(),
        1,
        "Built block should contain exactly one transaction"
    );

    let built_tx_bytes = &payload_response.execution_payload.transactions[0];
    let built_tx_hash = alloy::primitives::keccak256(built_tx_bytes.as_ref());
    assert_eq!(
        built_tx_hash, payload_tx.tx_hash,
        "The transaction in the block should be the one from payload_attributes"
    );
    assert_ne!(
        built_tx_hash, mempool_tx.tx_hash,
        "The transaction in the block should NOT be the one from the mempool"
    );
}
