pub mod block_number;
pub mod call;
pub mod chain_id;
pub mod estimate_gas;
pub mod fee_history;
pub mod forkchoice_updated;
pub mod get_balance;
pub mod get_block_by_hash;
pub mod get_block_by_number;
pub mod get_nonce;
pub mod get_payload;
pub mod get_transaction_receipt;
pub mod new_payload;
pub mod send_raw_transaction;

#[cfg(test)]
pub mod tests {
    use {
        crate::json_utils::access_state_error,
        alloy::{
            hex::FromHex,
            primitives::{hex, utils::parse_ether, FixedBytes},
            rlp::Encodable,
        },
        move_core_types::account_address::AccountAddress,
        moved::{
            block::{
                Block, BlockHash, BlockMemory, BlockQueries, BlockRepository, Eip1559GasFee,
                GasFee, InMemoryBlockQueries, InMemoryBlockRepository, MovedBlockHash,
            },
            genesis::{self, config::GenesisConfig},
            move_execution::{BaseTokenAccounts, CreateL1GasFee, MovedBaseTokenAccounts},
            primitives::{Address, B256, U256, U64},
            state_actor::{
                InMemoryStateQueries, MockStateQueries, NewPayloadId, StateActor, StateQueries,
            },
            storage::InMemoryState,
            types::{
                state::{Command, Payload, StateMessage},
                transactions::{DepositedTx, ExtendedTxEnvelope},
            },
        },
        tokio::sync::{
            mpsc::{self, Sender},
            oneshot,
        },
    };

    pub fn create_state_actor() -> (moved::state_actor::InMemStateActor, Sender<StateMessage>) {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut block_memory = BlockMemory::new();
        let mut repository = InMemoryBlockRepository::new();
        repository.add(&mut block_memory, genesis_block);

        let mut state = InMemoryState::new();
        let (changes, table_changes) = genesis::init(&genesis_config, &state);
        genesis::apply(changes.clone(), table_changes, &genesis_config, &mut state);

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
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            InMemoryBlockQueries,
            block_memory,
            InMemoryStateQueries::from_genesis(changes),
            StateActor::on_tx_noop(),
            StateActor::on_tx_batch_noop(),
        );
        (state, state_channel)
    }

    pub async fn deposit_eth(to: &str, channel: &Sender<StateMessage>) {
        let (sender, receiver) = oneshot::channel();
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

        let msg = Command::StartBlockBuild {
            payload_attributes,
            response_channel: sender,
        }
        .into();
        channel.send(msg).await.map_err(access_state_error).unwrap();
        receiver.await.map_err(access_state_error).unwrap();
    }

    pub fn create_state_actor_with_mock_state_queries(
        height: u64,
        address: AccountAddress,
    ) -> (
        StateActor<
            InMemoryState,
            impl NewPayloadId,
            impl BlockHash,
            impl BlockRepository<Storage = ()>,
            impl GasFee,
            impl CreateL1GasFee,
            impl BaseTokenAccounts,
            impl BlockQueries<Storage = ()>,
            (),
            impl StateQueries,
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
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            (),
            (),
            MockStateQueries(height, address),
            StateActor::on_tx_noop(),
            StateActor::on_tx_batch_noop(),
        );
        (state, state_channel)
    }
}
