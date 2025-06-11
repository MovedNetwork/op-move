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

pub struct ReaderWithFactory<D: DependenciesThreadSafe, F: ApplicationOnlyFactory<D>> {
    reader: ApplicationReader<D>,
    factory: F,
}

pub trait ApplicationFactory<D: DependenciesThreadSafe, F: ApplicationOnlyFactory<D>> {
    fn create(self) -> ReaderWithFactory<D, F>;
}

pub trait ApplicationOnlyFactory<D: DependenciesThreadSafe> {
    fn create(self) -> Application<D>;
}

impl<D: DependenciesThreadSafe> ApplicationOnlyFactory<D> for Application<D> {
    fn create(self) -> Application<D> {
        self
    }
}

impl<D: DependenciesThreadSafe, F: FnOnce() -> (ApplicationReader<D>, Application<D>)>
    ApplicationFactory<D, Application<D>> for F
{
    fn create(self) -> ReaderWithFactory<D, Application<D>> {
        let (reader, app) = self();

        ReaderWithFactory::new(reader, app)
    }
}

impl<D: DependenciesThreadSafe + Sized, F: FnOnce() -> Application<D>> ApplicationOnlyFactory<D>
    for F
{
    fn create(self) -> Application<D> {
        self()
    }
}

impl<
    D: DependenciesThreadSafe,
    F1: FnOnce() -> ApplicationReader<D>,
    F2: FnOnce() -> Application<D>,
> ApplicationFactory<D, F2> for (F1, F2)
{
    fn create(self) -> ReaderWithFactory<D, F2> {
        let reader = self.0();

        ReaderWithFactory::new(reader, self.1)
    }
}

impl<D: DependenciesThreadSafe, F: ApplicationOnlyFactory<D>> ReaderWithFactory<D, F> {
    pub fn new(reader: ApplicationReader<D>, factory: F) -> Self {
        Self { reader, factory }
    }
}

/// Creates and runs the `future`.
pub async fn run<D: DependenciesThreadSafe, F, Out, T: ApplicationOnlyFactory<D>>(
    factory: impl ApplicationFactory<D, T>,
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
    let reader_with_factory = factory.create();
    let handle = future(queue, reader_with_factory.reader);
    let mut app = reader_with_factory.factory.create();
    let actor = CommandActor::new(rx, &mut app);

    crate::run_with_actor(actor, handle).await
}
