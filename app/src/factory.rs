//! The application creation interface.
//!
//! This module contains:
//! * [`ApplicationFactory`]: A trait that encapsulates creating [`ApplicationReader`] and
//!   [`Application`].
//! * [`run`]: A function that creates and runs the application concurrently with a provided future.

use {
    crate::{
        Application, ApplicationReader, CommandActor, DependenciesThreadSafe, queue::CommandQueue,
    },
    std::{fmt::Debug, marker::PhantomData},
    tokio::sync::{broadcast, mpsc},
};

pub fn create<'app, 'b, T: DependenciesThreadSafe<'b>>(
    app: &'app mut Application<'b, T>,
    buffer: u32,
) -> (CommandQueue, CommandActor<'app, 'b, T>) {
    let (ktx, _) = broadcast::channel(1);
    let (tx, rx) = mpsc::channel(buffer as usize);

    (CommandQueue::new(tx, ktx), CommandActor::new(rx, app))
}

/// Encapsulates the [`ApplicationReader`] and [`Application`] creation.
///
/// This trait is automatically implemented for any [`FnOnce`] that returns a pair of
/// ([`ApplicationReader`], [`Application`]) or a pair of two [`FnOnce`] that return these values
/// individually. In both cases the closures have no arguments.
///
/// The single closure implementation is used when it's possible to create both immediately, while
/// the second one when the reader is created first but there can be a delay until the [`Application`]
/// is returned.
pub trait ApplicationFactory<
    'app,
    'reader,
    D: DependenciesThreadSafe<'app>,
    RD: DependenciesThreadSafe<'reader>,
    F: ApplicationOnlyFactory<'app, D>,
>
{
    fn create(self) -> ReaderWithFactory<'app, 'reader, D, RD, F>;
}

impl<
    'app,
    'reader,
    D: DependenciesThreadSafe<'app>,
    RD: DependenciesThreadSafe<'reader>,
    F: FnOnce() -> (ApplicationReader<'reader, RD>, Application<'app, D>),
> ApplicationFactory<'app, 'reader, D, RD, Application<'app, D>> for F
{
    fn create(self) -> ReaderWithFactory<'app, 'reader, D, RD, Application<'app, D>> {
        let (reader, app) = self();

        ReaderWithFactory::new(reader, app)
    }
}

impl<
    'app,
    'reader,
    D: DependenciesThreadSafe<'app>,
    RD: DependenciesThreadSafe<'reader>,
    Reader: FnOnce() -> ApplicationReader<'reader, RD>,
    Factory: FnOnce() -> Application<'app, D>,
> ApplicationFactory<'app, 'reader, D, RD, Factory> for (Reader, Factory)
{
    fn create(self) -> ReaderWithFactory<'app, 'reader, D, RD, Factory> {
        let reader = self.0();

        ReaderWithFactory::new(reader, self.1)
    }
}

/// Creates [`Application`] without reader.
///
/// This is the second part of [`ApplicationFactory`] that should be executed after creating the
/// [`ApplicationReader`].
pub trait ApplicationOnlyFactory<'app, D: DependenciesThreadSafe<'app>> {
    fn create(self) -> Application<'app, D>;
}

impl<'app, D: DependenciesThreadSafe<'app>> ApplicationOnlyFactory<'app, D>
    for Application<'app, D>
{
    fn create(self) -> Application<'app, D> {
        self
    }
}

impl<'app, D: DependenciesThreadSafe<'app> + Sized, F: FnOnce() -> Application<'app, D>>
    ApplicationOnlyFactory<'app, D> for F
{
    fn create(self) -> Application<'app, D> {
        self()
    }
}

impl<
    'app,
    'reader,
    D: DependenciesThreadSafe<'app>,
    RD: DependenciesThreadSafe<'reader>,
    F: ApplicationOnlyFactory<'app, D>,
> ReaderWithFactory<'app, 'reader, D, RD, F>
{
    pub fn new(reader: ApplicationReader<'reader, RD>, factory: F) -> Self {
        Self {
            reader,
            factory,
            _marker: PhantomData,
        }
    }
}

/// Carries `reader` and a `factory` for [`Application`].
pub struct ReaderWithFactory<
    'app,
    'reader,
    D: DependenciesThreadSafe<'app>,
    RD: DependenciesThreadSafe<'reader>,
    F: ApplicationOnlyFactory<'app, D>,
> {
    reader: ApplicationReader<'reader, RD>,
    factory: F,
    _marker: PhantomData<&'app D>,
}

/// Creates [`Application`] and runs the `future`.
///
/// Passes [`CommandQueue`] and [`ApplicationReader`] into the `future` which it can use to send
/// commands and run queries. Size of the queue is determined by `buffer`.
///
/// The queries are processed in an actor that runs concurrently with the `future`. The actor owns
/// the [`Application`], which is created after the `future` using the `factory`.
///
/// The provided [`ApplicationFactory`] implementation can expect that the `future` is created after
/// [`ApplicationReader`] and before [`Application`]
pub async fn run<
    'app,
    'reader,
    D: DependenciesThreadSafe<'app>,
    RD: DependenciesThreadSafe<'reader>,
    F,
    Out,
    T: ApplicationOnlyFactory<'app, D>,
>(
    factory: impl ApplicationFactory<'app, 'reader, D, RD, T>,
    buffer: u32,
    future: impl FnOnce(CommandQueue, ApplicationReader<'reader, RD>) -> F,
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
