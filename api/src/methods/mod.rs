pub mod block_number;
pub mod call;
pub mod chain_id;
pub mod estimate_gas;
pub mod fee_history;
pub mod forkchoice_updated;
pub mod gas_price;
pub mod get_balance;
pub mod get_block_by_hash;
pub mod get_block_by_number;
pub mod get_nonce;
pub mod get_payload;
pub mod get_proof;
pub mod get_transaction_by_hash;
pub mod get_transaction_receipt;
pub mod new_payload;
pub mod send_raw_transaction;

#[cfg(test)]
pub mod tests {
    use {
        alloy::{
            consensus::{Sealed, SignableTransaction, TxEip1559},
            hex::FromHex,
            network::TxSignerSync,
            primitives::{Bytes, FixedBytes, TxKind, hex, utils::parse_ether},
            rlp::Encodable,
            signers::local::PrivateKeySigner,
        },
        move_core_types::account_address::AccountAddress,
        moved_app::{
            Application, ApplicationReader, Command, CommandActor, DependenciesThreadSafe, Payload,
            TestDependencies,
        },
        moved_blockchain::{
            block::{
                Block, BlockQueries, BlockRepository, BlockResponse, Eip1559GasFee,
                InMemoryBlockQueries, InMemoryBlockRepository, MovedBlockHash,
            },
            in_memory::{SharedMemory, shared_memory},
            payload::InMemoryPayloadQueries,
            receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, ReceiptMemory},
            state::{InMemoryStateQueries, MockStateQueries},
            transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
        },
        moved_evm_ext::state::InMemoryStorageTrieRepository,
        moved_execution::MovedBaseTokenAccounts,
        moved_genesis::config::{CHAIN_ID, GenesisConfig},
        moved_shared::primitives::{Address, B256, U64, U256},
        moved_state::InMemoryState,
        op_alloy::consensus::{OpTxEnvelope, TxDeposit},
        std::{convert::Infallible, sync::Arc},
        tokio::sync::{RwLock, mpsc::Sender},
    };

    /// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
    pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

    pub fn create_app() -> (
        ApplicationReader<TestDependencies>,
        Application<TestDependencies>,
    ) {
        let genesis_config = GenesisConfig::default();

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let (memory_reader, mut memory) = shared_memory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut memory, genesis_block).unwrap();

        let mut state = InMemoryState::new();
        let mut evm_storage = InMemoryStorageTrieRepository::new();
        let (changes, table_changes, evm_storage_changes) = moved_genesis_image::load();
        moved_genesis::apply(
            changes.clone(),
            table_changes,
            evm_storage_changes,
            &genesis_config,
            &mut state,
            &mut evm_storage,
        );
        let initial_state_root = genesis_config.initial_state_root;

        (
            ApplicationReader {
                genesis_config,
                base_token: (),
                block_queries: (),
                payload_queries: (),
                receipt_queries: (),
                receipt_memory: (),
                storage: memory_reader,
                state: (),
                state_queries: (),
                evm_storage: (),
                transaction_queries: (),
            },
            Application {
                mem_pool: Default::default(),
                genesis_config,
                gas_fee: Eip1559GasFee::default(),
                base_token: MovedBaseTokenAccounts::new(AccountAddress::ONE),
                l1_fee: U256::ZERO,
                l2_fee: U256::ZERO,
                block_hash: MovedBlockHash,
                block_queries: InMemoryBlockQueries,
                block_repository: repository,
                on_payload: CommandActor::on_payload_in_memory(),
                on_tx: CommandActor::on_tx_noop(),
                on_tx_batch: CommandActor::on_tx_batch_noop(),
                payload_queries: InMemoryPayloadQueries::new(),
                receipt_queries: InMemoryReceiptQueries::new(),
                receipt_repository: InMemoryReceiptRepository::new(),
                receipt_memory: ReceiptMemory::new(),
                storage: memory,
                state,
                state_queries: InMemoryStateQueries::from_genesis(initial_state_root),
                evm_storage,
                transaction_queries: InMemoryTransactionQueries::new(),
                transaction_repository: InMemoryTransactionRepository::new(),
            },
        )
    }

    pub async fn deposit_eth(to: &str, channel: &Sender<Command>) {
        let to = Address::from_hex(to).unwrap();
        let amount = parse_ether("1").unwrap();
        let tx = OpTxEnvelope::Deposit(Sealed::new(TxDeposit {
            to: TxKind::Call(to),
            value: amount,
            source_hash: FixedBytes::default(),
            from: to,
            mint: Some(amount.try_into().unwrap()),
            gas_limit: u64::MAX,
            is_system_transaction: false,
            input: Vec::new().into(),
        }));

        let mut encoded = Vec::new();
        tx.encode(&mut encoded);
        let payload_attributes = Payload {
            gas_limit: U64::MAX,
            transactions: vec![encoded.into()],
            ..Default::default()
        };

        let msg = Command::StartBlockBuild {
            payload_attributes,
            payload_id: U64::from(0x03421ee50df45cacu64),
        };
        channel.send(msg).await.unwrap();
    }

    pub async fn deploy_contract(contract_bytes: Bytes, channel: &Sender<Command>) {
        let mut tx = TxEip1559 {
            chain_id: CHAIN_ID,
            nonce: 0,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            gas_limit: u64::MAX,
            to: TxKind::Create,
            value: U256::ZERO,
            input: contract_bytes,
            access_list: Default::default(),
        };

        let signer = PrivateKeySigner::from_bytes(&PRIVATE_KEY.into()).unwrap();
        let signature = signer.sign_transaction_sync(&mut tx).unwrap();
        let signed_tx = tx.into_signed(signature);
        let tx = OpTxEnvelope::Eip1559(signed_tx);

        let mut encoded = Vec::new();
        tx.encode(&mut encoded);
        let payload_attributes = Payload {
            gas_limit: U64::MAX,
            transactions: vec![encoded.into()],
            ..Default::default()
        };

        let msg = Command::StartBlockBuild {
            payload_attributes,
            payload_id: U64::from(0x03421ee50df45cacu64),
        };
        channel.send(msg).await.unwrap();
    }

    #[allow(clippy::type_complexity)]
    pub fn create_app_with_mock_state_queries(
        address: AccountAddress,
        height: u64,
    ) -> Arc<RwLock<Application<impl DependenciesThreadSafe<State = InMemoryState>>>> {
        #[derive(Debug)]
        struct StubLatest(u64);

        impl BlockQueries for StubLatest {
            type Err = Infallible;
            type Storage = ();

            fn by_hash(
                &self,
                _: &Self::Storage,
                _: B256,
                _: bool,
            ) -> Result<Option<BlockResponse>, Self::Err> {
                unimplemented!("Unexpected call to `by_hash`")
            }

            fn by_height(
                &self,
                _: &Self::Storage,
                _: u64,
                _: bool,
            ) -> Result<Option<BlockResponse>, Self::Err> {
                unimplemented!("Unexpected call to `by_height`")
            }

            fn latest(&self, _: &Self::Storage) -> Result<Option<u64>, Self::Err> {
                Ok(Some(self.0))
            }
        }

        Arc::new(RwLock::new(Application::<
            TestDependencies<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>,
        > {
            genesis_config: GenesisConfig::default(),
            mem_pool: Default::default(),
            gas_fee: Eip1559GasFee::default(),
            base_token: MovedBaseTokenAccounts::new(AccountAddress::ONE),
            l1_fee: U256::ZERO,
            l2_fee: U256::ZERO,
            block_hash: MovedBlockHash,
            block_queries: StubLatest(height),
            block_repository: (),
            on_payload: CommandActor::on_payload_noop(),
            on_tx: CommandActor::on_tx_noop(),
            on_tx_batch: CommandActor::on_tx_batch_noop(),
            payload_queries: (),
            receipt_queries: (),
            receipt_repository: (),
            receipt_memory: (),
            storage: (),
            state: InMemoryState::new(),
            state_queries: MockStateQueries(address, height),
            evm_storage: (),
            transaction_queries: (),
            transaction_repository: (),
        }))
    }
}
