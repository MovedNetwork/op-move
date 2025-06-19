use {
    crate::queue::input,
    criterion::{
        criterion_group, measurement::WallTime, BatchSize, BenchmarkGroup, BenchmarkId, Criterion,
        Throughput,
    },
    std::process::Termination,
    tokio::runtime::Runtime,
    umi_app::{Application, DependenciesThreadSafe},
    umi_genesis::config::GenesisConfig,
    umi_server::initialize_app,
};

fn build_1000_blocks<'app>(
    runtime: &Runtime,
    bencher: &mut BenchmarkGroup<WallTime>,
    app: &mut Application<'app, impl DependenciesThreadSafe<'app>>,
    buffer_size: u32,
) {
    bencher.throughput(Throughput::Elements(*input::BLOCKS_1000_LEN));
    bencher.sample_size(100);
    bencher.bench_with_input(BenchmarkId::from_parameter(buffer_size), &buffer_size, {
        |b, _size| {
            b.iter_batched(
                input::blocks_1000,
                |input| {
                    let (queue, actor) = umi_app::create(app, buffer_size);

                    runtime.block_on(umi_app::run_with_actor(actor, async {
                        for msg in input {
                            queue.send(msg).await;
                        }
                        drop(queue)
                    }))
                },
                BatchSize::PerIteration,
            );
        }
    });
}

fn bench_build_1000_blocks_with_queue_size(bencher: &mut Criterion) -> impl Termination {
    let current = tokio::runtime::Builder::new_multi_thread().build().unwrap();
    let mut group = bencher.benchmark_group("Build 1000 blocks with queue size");

    for buffer_size in [1000000, 10000, 6000, 5000, 1000, 500, 200, 100, 1]
        .into_iter()
        .rev()
    {
        let (mut app, _app_reader) = initialize_app(&GenesisConfig::default());

        app.genesis_update(input::GENESIS);

        build_1000_blocks(&current, &mut group, &mut app, buffer_size);
    }
}

criterion_group!(benches, bench_build_1000_blocks_with_queue_size);
