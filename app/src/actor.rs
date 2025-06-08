use {
    crate::{Application, Dependencies, DependenciesThreadSafe, input::Command},
    move_core_types::effects::ChangeSet,
    std::{fmt::Debug, ops::DerefMut},
    tokio::sync::mpsc::Receiver,
    umi_blockchain::{
        payload::{InMemoryPayloadQueries, PayloadId},
        state::InMemoryStateQueries,
    },
    umi_shared::primitives::B256,
};

/// A function invoked on a completion of new transaction execution batch.
pub type OnTxBatch<S> = dyn Fn(&mut S) + Send + Sync;

/// A function invoked on an execution of a new transaction.
pub type OnTx<S> = dyn Fn(&mut S, ChangeSet) + Send + Sync;

/// A function invoked on an execution of a new payload.
pub type OnPayload<S> = dyn Fn(&mut S, PayloadId, B256) + Send + Sync;

pub struct CommandActor<'a, D: Dependencies> {
    rx: Receiver<Command>,
    app: &'a mut Application<D>,
}

impl<D: DependenciesThreadSafe> CommandActor<'_, D> {
    pub async fn work(mut self) {
        while let Some(msg) = self.rx.recv().await {
            Self::handle_command(&mut *self.app, msg);
        }
    }
}

impl<'a, D: Dependencies> CommandActor<'a, D> {
    pub fn new(rx: Receiver<Command>, app: &'a mut Application<D>) -> Self {
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

impl<D: Dependencies<StateQueries = InMemoryStateQueries>> CommandActor<'_, D> {
    pub fn on_tx_in_memory() -> &'static OnTx<Application<D>> {
        &|_state, _changes| ()
    }

    pub fn on_tx_batch_in_memory() -> &'static OnTxBatch<Application<D>> {
        &|_state| ()
    }
}

impl<D: Dependencies<PayloadQueries = InMemoryPayloadQueries>> CommandActor<'_, D> {
    pub fn on_payload_in_memory() -> &'static OnPayload<Application<D>> {
        &|_state, _payload_id, _block_hash| ()
    }
}

/// Runs the `future` while also running `actor` [`CommandActor::work`] loop concurrently, returning
/// when `future` completes.
///
/// The `actor` receives input from a [`CommandQueue`], which the `future` should use to send
/// [`Command`]s. At the end of the `future`, the queue gets dropped. When the queue gets dropped,
/// the `actor` work loop stops.
///
/// [`CommandQueue`]: crate::CommandQueue
pub async fn run<D: DependenciesThreadSafe, F, Out>(actor: CommandActor<'_, D>, future: F) -> Out
where
    F: Future<Output = Out> + Send,
    Out: Send + Debug,
{
    tokio::join!(actor.work(), future).1
}
