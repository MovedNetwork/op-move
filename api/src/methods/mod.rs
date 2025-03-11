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
        crate::json_utils::access_state_error,
        alloy::{
            consensus::{SignableTransaction, TxEip1559, TxEnvelope},
            hex::FromHex,
            network::TxSignerSync,
            primitives::{hex, utils::parse_ether, Bytes, FixedBytes, TxKind},
            rlp::Encodable,
            signers::local::PrivateKeySigner,
        },
        move_core_types::account_address::AccountAddress,
        moved_app::{Command, Payload, StateActor, StateMessage},
        moved_blockchain::{
            block::{
                BaseGasFee, Block, BlockHash, BlockQueries, BlockRepository, Eip1559GasFee,
                InMemoryBlockQueries, InMemoryBlockRepository, MovedBlockHash,
            },
            in_memory::SharedMemory,
            payload::{InMemoryPayloadQueries, NewPayloadId, PayloadQueries},
            receipt::{
                InMemoryReceiptQueries, InMemoryReceiptRepository, ReceiptMemory, ReceiptQueries,
                ReceiptRepository,
            },
            state::{InMemoryStateQueries, MockStateQueries, StateQueries},
            transaction::{
                InMemoryTransactionQueries, InMemoryTransactionRepository, TransactionQueries,
                TransactionRepository,
            },
        },
        moved_evm_ext::storage::{InMemoryStorageTrieRepository, StorageTrieRepository},
        moved_execution::{
            transaction::{DepositedTx, ExtendedTxEnvelope},
            BaseTokenAccounts, CreateL1GasFee, CreateL2GasFee, MovedBaseTokenAccounts,
        },
        moved_genesis::config::{GenesisConfig, CHAIN_ID},
        moved_shared::primitives::{Address, B256, U256, U64},
        moved_state::InMemoryState,
        tokio::sync::{
            mpsc::{self, Sender},
            oneshot,
        },
    };

    /// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
    pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

    pub fn create_state_actor() -> (moved_app::InMemStateActor, Sender<StateMessage>) {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut memory = SharedMemory::new();
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

        let state = StateActor::new(
            rx,
            state,
            head_hash,
            0,
            genesis_config,
            0x03421ee50df45cacu64,
            MovedBlockHash,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            U256::ZERO,
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            InMemoryBlockQueries,
            memory,
            InMemoryStateQueries::from_genesis(initial_state_root),
            InMemoryTransactionRepository::new(),
            InMemoryTransactionQueries::new(),
            ReceiptMemory::new(),
            InMemoryReceiptRepository::new(),
            InMemoryReceiptQueries::new(),
            InMemoryPayloadQueries::new(),
            evm_storage,
            StateActor::on_tx_noop(),
            StateActor::on_tx_batch_noop(),
            StateActor::on_payload_in_memory(),
        );
        (state, state_channel)
    }

    pub async fn deposit_eth(to: &str, channel: &Sender<StateMessage>) {
        let to = Address::from_hex(to).unwrap();
        let tx = ExtendedTxEnvelope::DepositedTx(DepositedTx {
            to,
            value: parse_ether("1").unwrap(),
            source_hash: FixedBytes::default(),
            from: to,
            mint: U256::ZERO,
            gas: U64::from(u64::MAX),
            is_system_tx: false,
            data: Vec::new().into(),
        });

        let mut encoded = Vec::new();
        tx.encode(&mut encoded);
        let mut payload_attributes = Payload::default();
        payload_attributes.transactions.push(encoded.into());

        let (sender, receiver) = oneshot::channel();
        let msg = Command::StartBlockBuild {
            payload_attributes,
            response_channel: sender,
        }
        .into();
        channel.send(msg).await.map_err(access_state_error).unwrap();
        receiver.await.map_err(access_state_error).unwrap();
    }

    pub async fn deploy_contract(contract_bytes: Bytes, channel: &Sender<StateMessage>) {
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
        let signed_tx = TxEnvelope::Eip1559(tx.into_signed(signature));
        let tx = ExtendedTxEnvelope::Canonical(signed_tx);

        let mut encoded = Vec::new();
        tx.encode(&mut encoded);
        let mut payload_attributes = Payload::default();
        payload_attributes.transactions.push(encoded.into());

        let (sender, receiver) = oneshot::channel();
        let msg = Command::StartBlockBuild {
            payload_attributes,
            response_channel: sender,
        }
        .into();
        channel.send(msg).await.map_err(access_state_error).unwrap();
        receiver.await.map_err(access_state_error).unwrap();
    }

    pub fn create_state_actor_with_mock_state_queries(
        address: AccountAddress,
        height: u64,
    ) -> (
        StateActor<
            InMemoryState,
            impl NewPayloadId,
            impl BlockHash,
            impl BlockRepository<Storage = ()>,
            impl BaseGasFee,
            impl CreateL1GasFee,
            impl CreateL2GasFee,
            impl BaseTokenAccounts,
            impl BlockQueries<Storage = ()>,
            (),
            impl StateQueries,
            impl TransactionRepository<Storage = ()>,
            impl TransactionQueries<Storage = ()>,
            (),
            impl ReceiptRepository<Storage = ()>,
            impl ReceiptQueries<Storage = ()>,
            impl PayloadQueries<Storage = ()>,
            impl StorageTrieRepository,
        >,
        Sender<StateMessage>,
    ) {
        let (state_channel, rx) = mpsc::channel(10);
        let state = StateActor::new(
            rx,
            InMemoryState::new(),
            B256::new(hex!(
                "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
            )),
            height,
            GenesisConfig::default(),
            0x03421ee50df45cacu64,
            MovedBlockHash,
            (),
            Eip1559GasFee::default(),
            U256::ZERO,
            U256::ZERO,
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            (),
            (),
            MockStateQueries(address, height),
            (),
            (),
            (),
            (),
            (),
            (),
            (),
            StateActor::on_tx_noop(),
            StateActor::on_tx_batch_noop(),
            StateActor::on_payload_noop(),
        );
        (state, state_channel)
    }
}
