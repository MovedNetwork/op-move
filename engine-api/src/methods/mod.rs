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
            consensus::{SignableTransaction, TxEip1559, TxEnvelope},
            hex::FromHex,
            network::TxSignerSync,
            primitives::{hex, utils::parse_ether, Bytes, FixedBytes, TxKind},
            rlp::Encodable,
            signers::local::PrivateKeySigner,
        },
        move_core_types::account_address::AccountAddress,
        moved::{
            block::{
                Block, BlockMemory, BlockRepository, Eip1559GasFee, InMemoryBlockQueries,
                InMemoryBlockRepository, MovedBlockHash,
            },
            genesis::{
                config::{GenesisConfig, CHAIN_ID},
                init_state,
            },
            move_execution::MovedBaseTokenAccounts,
            primitives::{Address, B256, U256, U64},
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

    /// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
    pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

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
        init_state(&genesis_config, &mut state);

        let state = moved::state_actor::StateActor::new(
            rx,
            state,
            head_hash,
            genesis_config,
            0x03421ee50df45cacu64,
            MovedBlockHash,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            MovedBaseTokenAccounts::new(AccountAddress::ONE),
            InMemoryBlockQueries,
            block_memory,
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
}
