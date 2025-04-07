use {
    crate::{
        Application, Dependencies, DependenciesThreadSafe,
        input::{Command, Query, StateMessage},
    },
    move_core_types::effects::ChangeSet,
    moved_blockchain::{
        payload::{InMemoryPayloadQueries, PayloadId},
        state::InMemoryStateQueries,
    },
    moved_shared::primitives::B256,
    moved_state::State,
    std::{
        ops::{Deref, DerefMut},
        sync::Arc,
    },
    tokio::{
        sync::{RwLock, mpsc::Receiver},
        task::JoinHandle,
    },
};

/// A function invoked on a completion of new transaction execution batch.
pub type OnTxBatch<S> = dyn Fn(&mut S) + Send + Sync;

/// A function invoked on an execution of a new transaction.
pub type OnTx<S> = dyn Fn(&mut S, ChangeSet) + Send + Sync;

/// A function invoked on an execution of a new payload.
pub type OnPayload<S> = dyn Fn(&mut S, PayloadId, B256) + Send + Sync;

pub struct StateActor<D: Dependencies> {
    rx: Receiver<StateMessage>,
    app: Arc<RwLock<Application<D>>>,
}

impl<D: DependenciesThreadSafe> StateActor<D> {
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = self.rx.recv().await {
                match msg {
                    StateMessage::Command(msg) => Self::handle_command(self.app.write().await, msg),
                    StateMessage::Query(msg) => Self::handle_query(self.app.read().await, msg),
                };
            }
        })
    }
}

impl<D: Dependencies> StateActor<D> {
    pub fn new(rx: Receiver<StateMessage>, app: Arc<RwLock<Application<D>>>) -> Self {
        Self { rx, app }
    }

    pub fn handle_query(app: impl Deref<Target = Application<D>>, msg: Query) {
        match msg {
            Query::ChainId { response_channel } => response_channel.send(app.chain_id()).ok(),
            Query::BalanceByHeight {
                address,
                response_channel,
                height,
            } => response_channel
                .send(app.balance_by_height(address, height))
                .ok(),
            Query::NonceByHeight {
                address,
                response_channel,
                height,
            } => response_channel
                .send(app.nonce_by_height(address, height))
                .ok(),
            Query::BlockByHash {
                hash,
                response_channel,
                include_transactions,
            } => response_channel
                .send(app.block_by_hash(hash, include_transactions))
                .ok(),
            Query::BlockByHeight {
                height,
                response_channel,
                include_transactions,
            } => response_channel
                .send(app.block_by_height(height, include_transactions))
                .ok(),
            Query::BlockNumber { response_channel } => {
                response_channel.send(app.block_number()).ok()
            }
            Query::FeeHistory {
                response_channel,
                block_count,
                block_number,
                reward_percentiles,
            } => response_channel
                .send(app.fee_history(block_count, block_number, reward_percentiles))
                .ok(),
            Query::EstimateGas {
                transaction,
                block_number,
                response_channel,
            } => response_channel
                .send(app.estimate_gas(transaction, block_number))
                .ok(),
            Query::Call {
                transaction,
                response_channel,
                block_number,
            } => response_channel
                .send(app.call(transaction, block_number))
                .ok(),
            Query::TransactionReceipt {
                tx_hash,
                response_channel,
            } => response_channel.send(app.transaction_receipt(tx_hash)).ok(),
            Query::TransactionByHash {
                tx_hash,
                response_channel,
            } => response_channel.send(app.transaction_by_hash(tx_hash)).ok(),
            Query::GetProof {
                address,
                storage_slots,
                height,
                response_channel,
            } => response_channel
                .send(app.proof(address, storage_slots, height))
                .ok(),
            Query::GetPayload {
                id: payload_id,
                response_channel,
            } => response_channel.send(app.payload(payload_id)).ok(),
            Query::GetPayloadByBlockHash {
                block_hash,
                response_channel,
            } => response_channel
                .send(app.payload_by_block_hash(block_hash))
                .ok(),
        };
    }

    pub fn handle_command(mut app: impl DerefMut<Target = Application<D>>, msg: Command) {
        match msg {
            Command::UpdateHead { block_hash } => app.update_head(block_hash),
            Command::StartBlockBuild {
                payload_attributes,
                payload_id,
            } => app.start_block_build(payload_attributes, payload_id),
            Command::AddTransaction { tx } => app.add_transaction(tx),
            Command::GenesisUpdate { block } => app.genesis_update(block),
        }
    }

    pub fn on_tx_batch_noop() -> &'static OnTxBatch<Application<D>> {
        &|_| {}
    }

    pub fn on_tx_noop() -> &'static OnTx<Application<D>> {
        &|_, _| {}
    }

    pub fn on_payload_noop() -> &'static OnPayload<Application<D>> {
        &|_, _, _| {}
    }
}

impl<D: Dependencies<StateQueries = InMemoryStateQueries>> StateActor<D> {
    pub fn on_tx_in_memory() -> &'static OnTx<Application<D>> {
        &|_state, _changes| ()
    }

    pub fn on_tx_batch_in_memory() -> &'static OnTxBatch<Application<D>> {
        &|state| {
            state
                .state_queries
                .push_state_root(state.state.state_root())
        }
    }
}

impl<D: Dependencies<PayloadQueries = InMemoryPayloadQueries>> StateActor<D> {
    pub fn on_payload_in_memory() -> &'static OnPayload<Application<D>> {
        &|state, payload_id, block_hash| {
            state.payload_queries.add_block_hash(payload_id, block_hash)
        }
    }
}
