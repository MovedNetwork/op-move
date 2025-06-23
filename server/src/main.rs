use {
    umi_server::defaults,
    umi_server_args::{CliLayer, ConfigBuilder, EnvLayer, FileLayer},
};

#[tokio::main]
async fn main() {
    let args = ConfigBuilder::new()
        .layer(defaults())
        .layer(FileLayer::toml())
        .layer(EnvLayer::new())
        .layer(CliLayer::new())
        .try_build()
        .unwrap();

    umi_server::run(args).await;
}
