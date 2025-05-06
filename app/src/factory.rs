use {
    crate::{Application, CommandActor, DependenciesThreadSafe, queue::CommandQueue},
    tokio::sync::{broadcast, mpsc},
};

pub fn create<T: DependenciesThreadSafe>(
    app: Box<Application<T>>,
    buffer: u32,
) -> (CommandQueue, CommandActor<T>) {
    let (ktx, _) = broadcast::channel(1);
    let (tx, rx) = mpsc::channel(buffer as usize);

    (CommandQueue::new(tx, ktx), CommandActor::new(rx, app))
}
