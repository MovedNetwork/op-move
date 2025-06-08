use {
    crate::{
        Application, ApplicationReader, CommandActor, DependenciesThreadSafe, queue::CommandQueue,
    },
    std::fmt::Debug,
    tokio::sync::{broadcast, mpsc},
};

pub fn create<T: DependenciesThreadSafe>(
    app: &mut Application<T>,
    buffer: u32,
) -> (CommandQueue, CommandActor<T>) {
    let (ktx, _) = broadcast::channel(1);
    let (tx, rx) = mpsc::channel(buffer as usize);

    (CommandQueue::new(tx, ktx), CommandActor::new(rx, app))
}

/// Creates and runs the `future`.
pub async fn run_deferred<D: DependenciesThreadSafe, F, Out>(
    reader: impl FnOnce() -> ApplicationReader<D>,
    app: impl FnOnce() -> Application<D>,
    buffer: u32,
    future: impl FnOnce(CommandQueue, ApplicationReader<D>) -> F,
) -> Out
where
    F: Future<Output = Out> + Send,
    Out: Send + Debug,
{
    let (ktx, _) = broadcast::channel(1);
    let (tx, rx) = mpsc::channel(buffer as usize);
    let queue = CommandQueue::new(tx, ktx);
    let reader = reader();
    let handle = future(queue, reader);
    let mut app = app();
    let actor = CommandActor::new(rx, &mut app);

    crate::run(actor, handle)
}
