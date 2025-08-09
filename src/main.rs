use clap::Parser;
use roy::{run, Args};
use log::LevelFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut builder = env_logger::Builder::new();
    let filter = args.verbosity.log_level_filter();

    if filter == LevelFilter::Error && std::env::var("RUST_LOG").is_err() {
        builder.filter_level(LevelFilter::Info);
    } else {
        builder.filter_level(filter);
    }

    builder.init();

    run(args).await
}
