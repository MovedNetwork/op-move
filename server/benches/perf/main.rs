use criterion::criterion_main;

// Safety: this is benchmark code only, not used in production.
#[allow(clippy::unwrap_used)]
mod queue;

criterion_main!(queue::benches);
