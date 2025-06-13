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

pub fn create<T: DependenciesThreadSafe>(
    app: &mut Application<T>,
    buffer: u32,
) -> (CommandQueue, CommandActor<T>) {
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
    D: DependenciesThreadSafe,
    RD: DependenciesThreadSafe,
    F: ApplicationOnlyFactory<D>,
>
{
    fn create(self) -> ReaderWithFactory<D, RD, F>;
}

impl<
    D: DependenciesThreadSafe,
    RD: DependenciesThreadSafe,
    F: FnOnce() -> (ApplicationReader<RD>, Application<D>),
> ApplicationFactory<D, RD, Application<D>> for F
{
    fn create(self) -> ReaderWithFactory<D, RD, Application<D>> {
        let (reader, app) = self();

        ReaderWithFactory::new(reader, app)
    }
}

impl<
    D: DependenciesThreadSafe,
    RD: DependenciesThreadSafe,
    Reader: FnOnce() -> ApplicationReader<RD>,
    Factory: FnOnce() -> Application<D>,
> ApplicationFactory<D, RD, Factory> for (Reader, Factory)
{
    fn create(self) -> ReaderWithFactory<D, RD, Factory> {
        let reader = self.0();

        ReaderWithFactory::new(reader, self.1)
    }
}

/// Creates [`Application`] without reader.
///
/// This is the second part of [`ApplicationFactory`] that should be executed after creating the
/// [`ApplicationReader`].
pub trait ApplicationOnlyFactory<D: DependenciesThreadSafe> {
    fn create(self) -> Application<D>;
}

impl<D: DependenciesThreadSafe> ApplicationOnlyFactory<D> for Application<D> {
    fn create(self) -> Application<D> {
        self
    }
}

impl<D: DependenciesThreadSafe + Sized, F: FnOnce() -> Application<D>> ApplicationOnlyFactory<D>
    for F
{
    fn create(self) -> Application<D> {
        self()
    }
}

impl<D: DependenciesThreadSafe, RD: DependenciesThreadSafe, F: ApplicationOnlyFactory<D>>
    ReaderWithFactory<D, RD, F>
{
    pub fn new(reader: ApplicationReader<RD>, factory: F) -> Self {
        Self {
            reader,
            factory,
            _marker: PhantomData,
        }
    }
}

/// Carries `reader` and a `factory` for [`Application`].
pub struct ReaderWithFactory<
    D: DependenciesThreadSafe,
    RD: DependenciesThreadSafe,
    F: ApplicationOnlyFactory<D>,
> {
    reader: ApplicationReader<RD>,
    factory: F,
    _marker: PhantomData<D>,
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
    D: DependenciesThreadSafe,
    RD: DependenciesThreadSafe,
    F,
    Out,
    T: ApplicationOnlyFactory<D>,
>(
    factory: impl ApplicationFactory<D, RD, T>,
    buffer: u32,
    future: impl FnOnce(CommandQueue, ApplicationReader<RD>) -> F,
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
