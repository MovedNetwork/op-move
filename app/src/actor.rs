use {
    crate::{Application, Dependencies, DependenciesThreadSafe, input::Command},
    move_core_types::effects::ChangeSet,
    std::{fmt::Debug, ops::DerefMut},
    tokio::sync::mpsc::Receiver,
    umi_blockchain::{
        payload::{InMemoryPayloadQueries, PayloadId},
        state::InMemoryStateQueries,
    },
    umi_shared::{error::Error, primitives::B256},
};

/// A function invoked on a completion of new transaction execution batch.
pub type OnTxBatch<S> = dyn Fn(&mut S) -> Result<(), Error> + Send + Sync;

/// A function invoked on an execution of a new transaction.
pub type OnTx<S> = dyn Fn(&mut S, ChangeSet) -> Result<(), Error> + Send + Sync;

/// A function invoked on an execution of a new payload.
pub type OnPayload<S> = dyn Fn(&mut S, PayloadId, B256) -> Result<(), Error> + Send + Sync;

pub struct CommandActor<'actor, 'app, D: Dependencies<'app>> {
    rx: Receiver<Command>,
    app: &'actor mut Application<'app, D>,
}

// Marker struct for errors where the app has no choice but to
// stop receiving commands and drop.
#[derive(Debug)]
pub(crate) struct UnrecoverableAppFailure;

impl<'app, D: DependenciesThreadSafe<'app>> CommandActor<'_, 'app, D> {
    pub async fn work(mut self) {
        while let Some(msg) = self.rx.recv().await {
            if Self::handle_command(&mut *self.app, msg).is_err() {
                break;
            }
        }
    }
}

impl<'actor, 'app, D: Dependencies<'app>> CommandActor<'actor, 'app, D> {
    pub fn new(rx: Receiver<Command>, app: &'actor mut Application<'app, D>) -> Self {
        Self { rx, app }
    }

    fn handle_command(
        mut app: impl DerefMut<Target = Application<'app, D>>,
        msg: Command,
    ) -> Result<(), UnrecoverableAppFailure> {
        match msg {
            Command::StartBlockBuild {
                payload_attributes,
                payload_id,
            } => app.start_block_build(payload_attributes, payload_id),
            Command::AddTransaction { tx } => {
                app.add_transaction(tx);
                Ok(())
            }
        }
    }

    pub fn on_tx_batch_noop() -> &'actor OnTxBatch<Application<'app, D>> {
        &|_| Ok(())
    }

    pub fn on_tx_noop() -> &'actor OnTx<Application<'app, D>> {
        &|_, _| Ok(())
    }

    pub fn on_payload_noop() -> &'actor OnPayload<Application<'app, D>> {
        &|_, _, _| Ok(())
    }
}

impl<'app, D: Dependencies<'app, StateQueries = InMemoryStateQueries>> CommandActor<'app, 'app, D> {
    pub fn on_tx_in_memory() -> &'app OnTx<Application<'app, D>> {
        &|_state, _changes| Ok(())
    }

    pub fn on_tx_batch_in_memory() -> &'app OnTxBatch<Application<'app, D>> {
        &|_state| Ok(())
    }
}

impl<'actor, 'app, D: Dependencies<'app, PayloadQueries = InMemoryPayloadQueries>>
    CommandActor<'actor, 'app, D>
{
    pub fn on_payload_in_memory() -> &'actor OnPayload<Application<'app, D>> {
        &|_state, _payload_id, _block_hash| Ok(())
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
pub async fn run_with_actor<'actor, 'app, D: DependenciesThreadSafe<'app>, F, Out>(
    actor: CommandActor<'actor, 'app, D>,
    future: F,
) -> Out
where
    F: Future<Output = Out> + Send,
    Out: Send + Debug,
{
    tokio::join!(actor.work(), future).1
}
