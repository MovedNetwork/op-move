#[tokio::main]
async fn main() {
    // TODO: think about channel size bound
    let max_buffered_commands = 1_000;

    umi_server::run(max_buffered_commands).await;
}
