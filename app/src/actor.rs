use {
    crate::{Application, Dependencies, DependenciesThreadSafe, input::Command},
    move_core_types::effects::ChangeSet,
    moved_blockchain::{
        payload::{InMemoryPayloadQueries, PayloadId},
        state::InMemoryStateQueries,
    },
    moved_shared::primitives::B256,
    std::ops::DerefMut,
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

/// A function invoked on a completion of new transaction execution batch.
pub type OnTxBatch<S> = dyn Fn(&mut S) + Send + Sync;

/// A function invoked on an execution of a new transaction.
pub type OnTx<S> = dyn Fn(&mut S, ChangeSet) + Send + Sync;

/// A function invoked on an execution of a new payload.
pub type OnPayload<S> = dyn Fn(&mut S, PayloadId, B256) + Send + Sync;

pub struct CommandActor<D: Dependencies> {
    rx: Receiver<Command>,
    app: Box<Application<D>>,
}

impl<D: DependenciesThreadSafe> CommandActor<D> {
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = self.rx.recv().await {
                Self::handle_command(&mut *self.app, msg);
            }
        })
    }
}

impl<D: Dependencies> CommandActor<D> {
    pub fn new(rx: Receiver<Command>, app: Box<Application<D>>) -> Self {
        Self { rx, app }
    }

    pub fn handle_command(mut app: impl DerefMut<Target = Application<D>>, msg: Command) {
        match msg {
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

impl<D: Dependencies<StateQueries = InMemoryStateQueries>> CommandActor<D> {
    pub fn on_tx_in_memory() -> &'static OnTx<Application<D>> {
        &|_state, _changes| ()
    }

    pub fn on_tx_batch_in_memory() -> &'static OnTxBatch<Application<D>> {
        &|_state| ()
    }
}

impl<D: Dependencies<PayloadQueries = InMemoryPayloadQueries>> CommandActor<D> {
    pub fn on_payload_in_memory() -> &'static OnPayload<Application<D>> {
        &|state, payload_id, block_hash| {
            state.payload_queries.add_block_hash(payload_id, block_hash)
        }
    }
}
