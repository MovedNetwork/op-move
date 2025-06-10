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

pub enum CreationMethod<D: DependenciesThreadSafe, F: ApplicationOnlyFactory<D>> {
    Simultaneous(ApplicationReader<D>, Application<D>),
    Deferred(ApplicationReader<D>, F),
}

pub trait ApplicationFactory<D: DependenciesThreadSafe, F: ApplicationOnlyFactory<D>> {
    fn create(self) -> CreationMethod<D, F>;
}

pub trait ApplicationOnlyFactory<D: DependenciesThreadSafe> {
    fn create(self) -> Application<D>;
}

impl<D: DependenciesThreadSafe> ApplicationOnlyFactory<D> for () {
    fn create(self) -> Application<D> {
        unimplemented!("Unexpected call to create")
    }
}

impl<D: DependenciesThreadSafe, F: FnOnce() -> (ApplicationReader<D>, Application<D>)>
    ApplicationFactory<D, ()> for F
{
    fn create(self) -> CreationMethod<D, ()> {
        let (app, reader) = self();

        CreationMethod::Simultaneous(app, reader)
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
    fn create(self) -> CreationMethod<D, F2> {
        let reader = self.0();

        CreationMethod::Deferred(reader, self.1)
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

    match factory.create() {
        CreationMethod::Simultaneous(reader, mut app) => {
            let handle = future(queue, reader);
            let actor = CommandActor::new(rx, &mut app);

            crate::run_with_actor(actor, handle).await
        }
        CreationMethod::Deferred(reader, factory) => {
            let handle = future(queue, reader);
            let mut app = factory.create();
            let actor = CommandActor::new(rx, &mut app);

            crate::run_with_actor(actor, handle).await
        }
    }
}
